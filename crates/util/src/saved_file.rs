//! Contains tools for dealing with [serde] (specifically [serde_json])
//! serialization/deserialization to files (mainly by providing a simpler
//! [SavedFile] API).

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Seek, SeekFrom};
use std::path::Path;

use serde::{Serialize, de::DeserializeOwned};

use thiserror::Error;

/// Gives an object a nice API for saving/reading from a file. This trait is
/// blanket implemented for all types that meet the requirements.
pub trait SavedFile: Serialize + DeserializeOwned {
    /// Save data to disk.
    fn save_to_file(&self, mut file: &File) -> Result<(), SavedFileError> {
        crate::debug_log_info!("Writing to saved file.");

        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;

        // We'll pretty print if we're in debug mode.
        if cfg!(debug_assertions) {
            serde_json::to_writer_pretty(file, self)
        } else {
            serde_json::to_writer(file, self)
        }
        .map_err(Into::into)
    }

    /// Read data from disk.
    fn read_from_file(mut file: &File) -> Result<Self, SavedFileError> {
        file.seek(SeekFrom::Start(0))
            .inspect_err(|e| crate::debug_log_error!("Failed to start of file: {e}"))?;
        serde_json::from_reader(BufReader::new(file))
            .inspect_err(|e| crate::debug_log_error!("Failed deserialize file: {e}"))
            .map_err(Into::into)
    }

    /// Read a file from disk, saving the result of `f` to disk if the file does
    /// not exist yet.
    fn read_from_file_path<P, F>(file_path: P, f: F) -> Result<(Self, File), SavedFileError>
    where
        P: AsRef<Path>,
        F: FnOnce() -> Self,
    {
        let (file, created) = open_file_with_create_info(file_path).inspect_err(|e| {
            crate::debug_log_error!("Failed to open file with create info: {e}");
        })?;

        let data = if !created {
            Self::read_from_file(&file).inspect_err(|e| {
                crate::debug_log_error!("Failed to read from file: {e}");
            })?
        } else {
            let data = f();
            data.save_to_file(&file).inspect_err(|e| {
                crate::debug_log_error!("Failed to save to file: {e}");
            })?;
            data
        };

        Ok((data, file))
    }

    /// The same as [SavedFile::read_from_file_path], but [Default::default] is
    /// used in place of a provided callback function.
    fn read_from_file_path_default<P>(file_path: P) -> Result<(Self, File), SavedFileError>
    where
        P: AsRef<Path>,
        Self: Default,
    {
        Self::read_from_file_path(file_path, Self::default)
    }
}

impl<T: Serialize + DeserializeOwned> SavedFile for T {}

/// Indicates that something went wrong trying to serialize or deserialize.
#[derive(Error, Debug)]
pub enum SavedFileError {
    #[error(transparent)]
    BadData(serde_json::Error),
    #[error(transparent)]
    IoError(#[from] io::Error),
}

impl From<serde_json::Error> for SavedFileError {
    fn from(e: serde_json::Error) -> Self {
        if e.is_io() {
            SavedFileError::IoError(e.into())
        } else {
            SavedFileError::BadData(e)
        }
    }
}

/// Opens a file, returning whether or not the file was created.
pub fn open_file_with_create_info<P: AsRef<Path>>(file_path: P) -> Result<(File, bool), io::Error> {
    open_file_with_create_info_impl(file_path.as_ref())
}

fn open_file_with_create_info_impl(file_path: &Path) -> Result<(File, bool), io::Error> {
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
