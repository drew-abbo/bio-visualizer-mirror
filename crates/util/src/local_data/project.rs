//! Tools for dealing with projects.
//!
//! I/O is very fragile, especially I/O with synchronization and locking (file
//! system operations aren't atomic). Try to recover from errors where possible.
//! There's debug logging everywhere in this module to help. Functions here will
//! do their best to clean up any changes to the filesystem when an error
//! occurs, but there's only so much you can do.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::result;
use std::time::SystemTime;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use thiserror::Error;

use time::{OffsetDateTime, macros::format_description};

use crate::local_data;
use crate::saved_file::{SavedFile, SavedFileError};
use crate::uid::Uid;

/// For accessing a project's basic header info ([ProjectInfo]). See [Project]
/// and [OpenProject].
pub trait ProjectHeader: Into<ProjectInfo> {
    /// The path to this project's directory.
    fn dir_path(&self) -> &Path;

    /// Mutate this project's info.
    ///
    /// If an error is returned, the state of the project's info file and the
    /// state of this object is unspecified.
    fn with_info_mut<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ProjectInfo);

    /// Get whatever info about this project is cached, regardless of whether
    /// the cache is stale.
    fn cached_info(&self) -> &ProjectInfo;

    /// Whether or not the cached info is out of date.
    fn info_cache_is_stale(&self) -> Result<bool>;

    /// Refreshses the cache if it's stale, returning whether or not the disk
    /// was read.
    fn refresh(&mut self) -> Result<bool>;

    /// Get up to date info about this project.
    ///
    /// If an error is returned, the object's state will not have changed.
    fn info(&mut self) -> Result<&ProjectInfo> {
        self.refresh()?;
        Ok(self.cached_info())
    }

    /// The timestamp of the last time the data file was edited, or [None] if it
    /// has never been edited.
    fn last_edited(&self) -> Result<Option<SystemTime>> {
        let data_file_path = self.dir_path().join(DATA_FILE_NAME);

        if !data_file_path.exists() {
            return Ok(None);
        }

        data_file_path
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map(Some)
            .map_err(|e| {
                crate::debug_log_error!("Failed to get last edit timestamp from path: {e}");
                e.into()
            })
    }

    /// The timestamp of the last time the data file was edited formatted as a
    /// human readable string (e.g. `1:02 PM 3/4/2025`), or [None] if it has
    /// never been edited.
    fn last_edited_string(&self) -> Result<Option<String>> {
        match self.last_edited() {
            Ok(Some(last_edited)) => Ok(Some(format_datetime(last_edited.into()))),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Like calling [ProjectHeader::last_edited] and
    /// [ProjectHeader::last_edited_string], but without checking the file
    /// system twice.
    fn last_edited_with_string(&self) -> Result<Option<(SystemTime, String)>> {
        let last_edited: OffsetDateTime = match self.last_edited() {
            Ok(Some(last_edited)) => last_edited.into(),
            Ok(None) => return Ok(None),
            Err(e) => return Err(e),
        };

        Ok(Some((last_edited.into(), format_datetime(last_edited))))
    }
}

/// A marker trait for types that can be used as the inner `T` type for
/// [OpenProject]. This trait is blanket implemented for all types that meet the
/// requirements.
///
/// Requirement reasoning:
/// - [SavedFile] is required so that the data can be saved and loaded to disk.
/// - [Default] is required so that some data can be stored when the project is
///   first created.
/// - [Clone] and [Eq] are required so that the last saved data can be cached.
///   This allows expensive serialization and I/O to be skipped when
///   [OpenProject::save] is without anything having changed.
pub trait ProjectData: SavedFile + Default + Clone + Eq {}

impl<T: Default + Clone + Eq + Serialize + DeserializeOwned> ProjectData for T {}

/// A project. The [ProjectHeader] trait can be used to get info about the
/// project.
#[derive(Debug)]
pub struct Project {
    cache: ProjectInfoCache,
    info_file: File,
    dir_path: PathBuf,
}

impl Project {
    /// Loads a new project (failing if a project with the same ID already
    /// exists). See [ProjectInfo::create_project] for a simpler way to
    /// construct.
    ///
    /// If an error is returned, the state of the project with the provided
    /// `project_id` is unspecified.
    pub fn create(info: ProjectInfo) -> Result<Self> {
        let mut dir_path = super::projects_path().join(info.id().as_ref());

        if dir_path.exists() {
            return Err(ProjectError::DuplicateId);
        }

        fs::create_dir(&dir_path).inspect_err(|e| {
            crate::debug_log_error!("Failed to create project directory: {e}");
        })?;

        let (file, cache) = Self::new_info_file_cached(&mut dir_path, info).inspect_err(|e| {
            crate::debug_log_error!("Failed to create project info file: {e}");
            _ = fs::remove_dir_all(&dir_path).inspect_err(|e| {
                crate::debug_log_error!("Project directory cleanup failed (ignoring): {e}");
            });
        })?;

        Ok(Self {
            dir_path,
            cache,
            info_file: file,
        })
    }

    /// Loads an existing project.
    pub fn load(project_id: &ProjectId) -> Result<Self> {
        let mut dir_path = super::projects_path().join(project_id.as_ref());

        // Dir path is now the info file path.
        dir_path.push(INFO_FILE_NAME);
        let info_file_path = &mut dir_path;

        let info_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(info_file_path)
            .inspect_err(|e| {
                crate::debug_log_error!("Failed to open project info file: {e}");
            })?;

        // Restore dir path.
        dir_path.pop();

        let cache = with_file_locked_shared(&info_file, || {
            read_from_file_with_time::<ProjectInfo>(&info_file)
        })?;
        if cache.0.id().as_ref() != project_id.as_ref() {
            return Err(ProjectError::BadSerializedData);
        }

        Ok(Self {
            dir_path,
            cache,
            info_file,
        })
    }

    /// Attempt to clone this object. This function will not refresh the cache
    /// of either object.
    pub fn try_clone(&self) -> Result<Self> {
        let mut dir_path = self.dir_path.clone();

        dir_path.push(INFO_FILE_NAME);
        let info_file_path = &mut dir_path;

        let info_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(info_file_path)
            .inspect_err(|e| {
                crate::debug_log_error!("Failed to open project info file: {e}");
            })?;

        // Restore dir path.
        dir_path.pop();

        Ok(Self {
            cache: self.cache.clone(),
            info_file,
            dir_path,
        })
    }

    /// Returns whether or not this project is already open for editing (even if
    /// it's open in another process). If `true` is returned, a subsequent call
    /// to [Self::open] or [Self::delete] will likely fail.
    pub fn is_open(&self) -> Result<bool> {
        let data_file_path = self.dir_path.join(DATA_FILE_NAME);

        let data_file = match OpenOptions::new()
            .read(true)
            .create(false)
            .open(&data_file_path)
        {
            Ok(data_file) => data_file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => {
                crate::debug_log_error!("Failed to try opening data file: {e}");
                return Err(e.into());
            }
        };

        if let Err(e) = data_file.try_lock() {
            return match e {
                TryLockError::WouldBlock => Ok(true),
                TryLockError::Error(e) => {
                    crate::debug_log_error!("Failed to try locking data file: {e}");
                    Err(e.into())
                }
            };
        }
        data_file.unlock().inspect_err(|e| {
            crate::debug_log_error!("Failed to unlock data file: {e}");
        })?;

        Ok(false)
    }

    /// Open the project for editing, locking its non-header data. This also
    /// write-locks the header-data (can still be read).
    pub fn open<T>(self) -> Result<OpenProject<T>>
    where
        T: ProjectData,
    {
        // Functions like this (where we can't easily use RAII for cleanup) make
        // me miss C-style error handling with `goto`. I think what we're doing
        // here with `try_cleanup` is the best solution, but there really isn't
        // a clean code solution here (at least not one that keeps the function
        // locally understandable).

        // Lock info file.
        self.info_file.lock_shared().inspect_err(|e| {
            crate::debug_log_error!("Failed to acquire shared lock on info file: {e}");
        })?;

        let try_cleanup = || {
            _ = self.info_file.unlock().inspect_err(|e| {
                crate::debug_log_error!("Failed to unlock info file (ignoring): {e}");
            });
        };

        // Create data file.
        let data_file_path = self.dir_path.join(DATA_FILE_NAME);
        let (data_file, data_file_was_created) = match Self::create_or_open_file(&data_file_path) {
            Ok((file, file_was_created)) => (file, file_was_created),

            Err(e) => {
                crate::debug_log_error!("Failed create/open data file.");
                try_cleanup();
                return Err(e.into());
            }
        };

        let try_cleanup = |data_file: File| {
            if data_file_was_created {
                drop(data_file);
                _ = fs::remove_file(&data_file_path).inspect_err(|e| {
                    crate::debug_log_error!("Failed to remove data file (ignoring): {e}");
                });
            }
            try_cleanup();
        };

        // Lock data file.
        if let Err(e) = data_file.try_lock() {
            try_cleanup(data_file);
            return Err(match e {
                TryLockError::WouldBlock => {
                    crate::debug_log_warning!(
                        "Failed to lock project data file `{}` (already locked).",
                        data_file_path.display()
                    );
                    ProjectError::Locked(self)
                }
                TryLockError::Error(e) => {
                    crate::debug_log_error!("Failed to acquire write lock on data file: {e}");
                    e.into()
                }
            });
        }

        let try_cleanup = |data_file: File| {
            _ = data_file.unlock().inspect_err(|e| {
                crate::debug_log_error!("Failed to unlock data file (ignoring): {e}");
            });
            try_cleanup(data_file);
        };

        // Write default data to file or get existing data.
        let data = if data_file_was_created {
            let data = T::default();

            if let Err(e) = data.save_to_file(&data_file) {
                crate::debug_log_error!("Failed to write to data file.");
                try_cleanup(data_file);
                return Err(e.into());
            }

            data
        } else {
            match T::read_from_file(&data_file) {
                Ok(data) => data,

                Err(e) => {
                    crate::debug_log_error!("Failed to read from data file: {e}");
                    try_cleanup(data_file);
                    return Err(e.into());
                }
            }
        };

        Ok(OpenProject {
            last_saved_data: data.clone(),
            data,
            data_file,
            header: Some(self),
        })
    }

    /// Delete this project. No [Project] or [OpenProject] instances should be
    /// pointing to this project (including in other processes).
    ///
    /// In the case of an I/O error, the project is left in an unspecified
    /// state.
    pub fn delete(self) -> Result<()> {
        if self.is_open()? {
            return Err(ProjectError::Locked(self));
        }
        fs::remove_dir_all(self.dir_path)?;
        Ok(())
    }

    /// Open a new info file, returning the file and a cache that store's its
    /// contents and write timestamp.
    ///
    /// Even though `dir_path` is `mut`, its contents will not have changed if
    /// the function returns successfully.
    fn new_info_file_cached(
        dir_path: &mut PathBuf,
        info: ProjectInfo,
    ) -> Result<(File, ProjectInfoCache)> {
        // Dir path is now the info file path.
        dir_path.push(INFO_FILE_NAME);
        let info_file_path = dir_path;

        let info_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&info_file_path)
            .map_err(|e| match e.kind() {
                io::ErrorKind::AlreadyExists => {
                    crate::debug_log_error!(
                        "Failed to create info file, `{}` already exists.",
                        info_file_path.display()
                    );
                    ProjectError::DuplicateId
                }
                _ => {
                    crate::debug_log_error!("Failed to create info file: {e}");
                    e.into()
                }
            })?;

        // Restore dir path.
        let dir_path = info_file_path;
        dir_path.pop();

        let cache = with_file_locked(&info_file, || {
            info.save_to_file(&info_file)?;
            Ok((info, last_edit_timestamp(&info_file)?))
        })?;

        Ok((info_file, cache))
    }

    /// The [bool] indicates whether or not the file was created (didn't already
    /// exist).
    fn create_or_open_file(file_path: &Path) -> result::Result<(File, bool), io::Error> {
        let mut open_options = OpenOptions::new();
        open_options.read(true).write(true);

        open_options.create_new(true);
        match open_options.open(file_path) {
            Ok(file) => return Ok((file, true)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        };

        open_options.create_new(false).create(false);
        open_options
            .open(file_path)
            .map(|file| (file, false))
            .inspect_err(|e| {
                crate::debug_log_error!("Failed to create or open file: {e}");
            })
    }
}

impl TryFrom<ProjectId> for Project {
    type Error = ProjectError;

    fn try_from(project_id: ProjectId) -> Result<Self> {
        Self::load(&project_id)
    }
}

impl TryFrom<&ProjectId> for Project {
    type Error = ProjectError;

    fn try_from(project_id: &ProjectId) -> Result<Self> {
        Self::load(project_id)
    }
}

impl TryFrom<&mut ProjectId> for Project {
    type Error = ProjectError;

    fn try_from(project_id: &mut ProjectId) -> Result<Self> {
        Self::load(project_id)
    }
}

impl From<Project> for ProjectInfo {
    fn from(project: Project) -> Self {
        project.cache.0
    }
}

impl From<Project> for ProjectId {
    fn from(project: Project) -> Self {
        let info: ProjectInfo = project.into();
        info.into()
    }
}

impl ProjectHeader for Project {
    fn dir_path(&self) -> &Path {
        &self.dir_path
    }

    fn with_info_mut<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ProjectInfo),
    {
        f(&mut self.cache.0);

        with_file_locked(&self.info_file, || {
            self.cache.0.save_to_file(&self.info_file)?;
            self.cache.1 = last_edit_timestamp(&self.info_file)?;
            Ok(())
        })
    }

    fn cached_info(&self) -> &ProjectInfo {
        &self.cache.0
    }

    fn info_cache_is_stale(&self) -> Result<bool> {
        Ok(self.cache.1 < last_edit_timestamp(&self.info_file)?)
    }

    fn refresh(&mut self) -> Result<bool> {
        if !self.info_cache_is_stale()? {
            return Ok(false);
        }

        let new_cache = with_file_locked_shared(&self.info_file, || {
            read_from_file_with_time::<ProjectInfo>(&self.info_file)
        })?;

        let expected_id_str = self
            .dir_path
            .file_name()
            .expect("The ID directory should have been added in the constructor.");
        if new_cache.0.id().as_ref() != expected_id_str {
            return Err(ProjectError::BadSerializedData);
        }

        self.cache = new_cache;
        Ok(true)
    }
}

/// An open project. While held, the project's data will only be able to be
/// edited through this object and the project's header data becomes read-only
/// (write-locked). The [ProjectHeader] trait can be used to get info about the
/// project. See [Project] to construct.
///
/// This type is generic over `T` because it doesn't make sense for this crate
/// to define the data format for a project's data. See [ProjectData].
#[derive(Debug)]
pub struct OpenProject<T: ProjectData> {
    data: T,
    last_saved_data: T,
    data_file: File,
    header: Option<Project>,
}

impl<T: ProjectData> OpenProject<T> {
    /// Saves the project, returning whether the data had to actually be written
    /// to disk.
    ///
    /// In the case of an I/O error, the data file is left in an unspecified
    /// state.
    pub fn save(&mut self) -> Result<bool> {
        if self.data == self.last_saved_data {
            return Ok(false);
        }

        self.data.save_to_file(&self.data_file).inspect_err(|e| {
            crate::debug_log_error!("Failed to save project data: {e}");
        })?;
        self.last_saved_data = self.data.clone();

        Ok(true)
    }

    /// Close the project, unlocking the project's non-header data.
    ///
    /// Can fail if unlocking the info file fails.
    pub fn close(mut self) -> Result<Project> {
        let header = self.header.take().expect(HEADER_EXPECT_MSG);
        header.info_file.unlock().inspect_err(|e| {
            crate::debug_log_error!("Failed to unlock info file: {e}");
        })?;
        Ok(header)
    }

    fn header(&self) -> &Project {
        self.header.as_ref().expect(HEADER_EXPECT_MSG)
    }

    fn header_mut(&mut self) -> &mut Project {
        self.header.as_mut().expect(HEADER_EXPECT_MSG)
    }
}

impl<T: ProjectData> Drop for OpenProject<T> {
    fn drop(&mut self) {
        if let Some(header) = &self.header {
            _ = header.info_file.unlock().inspect_err(|e| {
                crate::debug_log_error!("Failed to unlock info file in `Drop` (ignoring): {e}");
            });
        }
        _ = self.data_file.unlock().inspect_err(|e| {
            crate::debug_log_error!("Failed to unlock data file in `Drop` (ignoring): {e}");
        });
    }
}

impl<T: ProjectData> From<OpenProject<T>> for ProjectInfo {
    fn from(mut project: OpenProject<T>) -> Self {
        project.header.take().expect(HEADER_EXPECT_MSG).into()
    }
}

impl<T: ProjectData> From<OpenProject<T>> for ProjectId {
    fn from(project: OpenProject<T>) -> Self {
        let info: ProjectInfo = project.into();
        info.into()
    }
}

impl<T: ProjectData> ProjectHeader for OpenProject<T> {
    fn dir_path(&self) -> &Path {
        self.header().dir_path()
    }

    fn with_info_mut<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ProjectInfo),
    {
        self.header_mut().with_info_mut(f)
    }

    fn refresh(&mut self) -> Result<bool> {
        self.header_mut().refresh()
    }

    fn cached_info(&self) -> &ProjectInfo {
        self.header().cached_info()
    }

    fn info_cache_is_stale(&self) -> Result<bool> {
        self.header().info_cache_is_stale()
    }
}

/// Information about a project that can be accessed without opening
/// (write-locking) it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectInfo {
    id: ProjectId,
    name: String,
    created: OffsetDateTime,
}

impl ProjectInfo {
    /// Create [ProjectInfo].
    pub fn new(name: String) -> Self {
        let created = OffsetDateTime::now_local().unwrap_or_else(|e| {
            crate::debug_log_error!("Failed to get local time (ignoring, using UTC): {e}");
            OffsetDateTime::now_utc()
        });

        Self {
            id: ProjectId::default(),
            name,
            created,
        }
    }

    /// Create a new [Project] from this [ProjectInfo].
    pub fn create_project(self) -> Result<Project> {
        Project::create(self)
    }

    /// This project's [ProjectId].
    pub fn id(&self) -> &ProjectId {
        &self.id
    }

    /// This project's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// A *mutable* reference to this project's name.
    pub fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    /// The time this project info was created.
    pub fn created(&self) -> SystemTime {
        self.created.into()
    }

    /// The time this project info was created formatted as a human readable
    /// string (e.g. `1:02 PM 3/4/2025`).
    pub fn created_string(&self) -> String {
        format_datetime(self.created)
    }

    /// The same as calling [Self::created] and [Self::created_string].
    pub fn created_with_string(&self) -> (SystemTime, String) {
        (self.created(), self.created_string())
    }
}

/// The ID of a project.
///
/// See [Uid] for info on uniqueness.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(into = "String", try_from = "String")]
pub struct ProjectId(OsString);

impl ProjectId {
    /// Tries to load the header for the project ID. This is the equivalent to
    /// calling [Project::load].
    pub fn load_header(&self) -> Result<Project> {
        Project::load(self)
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self(Uid::default().to_string().into())
    }
}

impl From<ProjectInfo> for ProjectId {
    fn from(project_info: ProjectInfo) -> Self {
        project_info.id
    }
}

impl From<ProjectId> for String {
    fn from(id: ProjectId) -> Self {
        id.0.into_string()
            .expect("The inner string should be normal.")
    }
}

impl From<ProjectId> for OsString {
    fn from(id: ProjectId) -> Self {
        id.0
    }
}

impl AsRef<OsStr> for ProjectId {
    fn as_ref(&self) -> &OsStr {
        &self.0
    }
}

impl From<Uid> for ProjectId {
    fn from(uid: Uid) -> Self {
        Self(uid.to_string().into())
    }
}

impl TryFrom<OsString> for ProjectId {
    type Error = ProjectError;

    fn try_from(id_str: OsString) -> Result<Self> {
        if id_str.is_empty() {
            return Err(ProjectError::InvalidIdString);
        }

        for chr in id_str.as_encoded_bytes() {
            if !matches!(chr, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'-') {
                return Err(ProjectError::InvalidIdString);
            }
        }

        Ok(Self(id_str))
    }
}

impl TryFrom<String> for ProjectId {
    type Error = ProjectError;

    fn try_from(id_str: String) -> Result<Self> {
        Uid::try_from(id_str)
            .map_err(|_| ProjectError::InvalidIdString)
            .map(Self::from)
    }
}

/// Indicates something went wrong with the I/O for a project.
#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("This project's data file is locked (currently open).")]
    Locked(Project),
    #[error("The format of a serialized file is invalid.")]
    BadSerializedData,
    #[error("A project can't be created if another one has the same ID.")]
    DuplicateId,
    #[error("Invalid project ID string.")]
    InvalidIdString,
    #[error(transparent)]
    IoError(#[from] io::Error),
}

impl From<SavedFileError> for ProjectError {
    fn from(e: SavedFileError) -> Self {
        match e {
            SavedFileError::BadData(_) => Self::BadSerializedData,
            SavedFileError::IoError(e) => Self::IoError(e),
        }
    }
}

/// A shorthand for [std::result::Result] with an error type of [ProjectError].
pub type Result<T> = result::Result<T, ProjectError>;

/// Iterate over all [ProjectId]s on disk.
pub fn iter_projects() -> Result<impl Iterator<Item = Result<ProjectId>>>
// NOTE: We can't pull the return type's `impl Iterator<...>` out :(
// https://github.com/rust-lang/rust/issues/63063
{
    let dir = fs::read_dir(local_data::projects_path())?;

    let iter = dir
        .map(|entry| entry.map_err(ProjectError::from))
        .filter_map(|entry| -> Option<Result<ProjectId>> {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Some(Err(e)),
            };

            match entry.file_type() {
                Ok(file_type) if !file_type.is_dir() => return None,
                Err(e) => return Some(Err(e.into())),
                _ => {}
            }

            Some(ProjectId::try_from(entry.file_name()))
        });

    Ok(iter)
}

const INFO_FILE_NAME: &str = "info.json";
const DATA_FILE_NAME: &str = "data.json";

const HEADER_EXPECT_MSG: &str = "The header should be present.";

type ProjectInfoCache = (ProjectInfo, SystemTime);

fn read_from_file_with_time<T: SavedFile>(file: &File) -> Result<(T, SystemTime)> {
    let data = T::read_from_file(file)
        .inspect_err(|e| crate::debug_log_error!("Failed to read from saved file: {e}"))?;
    let time = last_edit_timestamp(file)
        .inspect_err(|e| crate::debug_log_error!("Failed to get last edit timestamp: {e}"))?;
    Ok((data, time))
}

/// Locks a file, runs `f`, then tries to unlock the file (regardless of `f`'s
/// return value).
///
/// Lock errors take priority over `f`'s return value, only if `f` hasn't run or
/// if `f` didn't return an error.
///
/// Also see [with_file_locked_shared].
///
/// The file *must* have been opened with read and write permissions.
fn with_file_locked<F, T>(file: &File, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    file.lock().inspect_err(|e| {
        crate::debug_log_error!("Failed to acquire write lock: {e}");
    })?;
    call_and_unlock(file, f)
}

/// The same as [with_file_locked], but it uses [File::lock_shared].
///
/// The file *must* have been opened with read and write permissions.
fn with_file_locked_shared<F, T>(file: &File, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    file.lock_shared().inspect_err(|e| {
        crate::debug_log_error!("Failed to acquire shared lock: {e}");
    })?;
    call_and_unlock(file, f)
}

fn call_and_unlock<F, T>(file: &File, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let ret = f();
    if let Err(ref e) = ret {
        crate::debug_log_error!("Callback failed: {e}");
    }

    let unlock_result = file.unlock();
    if let Err(ref e) = unlock_result {
        crate::debug_log_error!("Failed to unlock file: {e}");
    }

    match unlock_result {
        Ok(_) => ret,
        Err(_) if ret.is_err() => ret,
        Err(e) => Err(e.into()),
    }
}

fn last_edit_timestamp(file: &File) -> result::Result<SystemTime, io::Error> {
    file.metadata()
        .and_then(|metadata| metadata.modified())
        .inspect_err(|e| {
            crate::debug_log_error!("Failed to get last edit timestamp for open file: {e}");
        })
}

fn format_datetime(datetime: OffsetDateTime) -> String {
    datetime
        .format(format_description!(
            "[month padding:none]/[day padding:none]/[year] \
                [hour repr:12 padding:none]:[minute padding:zero] [period case:upper]"
        ))
        .expect("The date shouldn't fail to format.")
}
