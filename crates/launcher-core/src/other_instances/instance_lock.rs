//! Defines [InstanceLock], a lock that, when held, indicates that this is the
//! main instance.

use std::fs::{File, TryLockError};
use std::io;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use thiserror::Error;

use util::local_data;
use util::saved_file::{self, SavedFile, SavedFileError};
use util::version;

/// While held, no other instance can be the main instance. The instance lock
/// will be unlocked when this is dropped.
#[derive(Debug)]
pub struct InstanceLock<T: SavedFile> {
    data: InstanceLockData<T>,
    lock_file: File,
}

impl<T: SavedFile> InstanceLock<T> {
    /// Open the instance lock, calling `f` to generate the file's data if it
    /// doesn't already exist. [InstanceLockError::Locked] is returned if the
    /// lock is already being held.
    pub fn new<F>(f: F) -> Result<Self, InstanceLockError>
    where
        F: FnOnce() -> T,
    {
        let lock_file_path = local_data::root_path().join(LOCK_FILE_NAME);

        let (lock_file, lock_file_created) =
            saved_file::open_file_with_create_info(&lock_file_path)?;

        if let Err(e) = lock_file.try_lock() {
            match e {
                TryLockError::Error(e) => return Err(e.into()),
                TryLockError::WouldBlock => {
                    if lock_file_created {
                        util::debug_log_error!("Lock file was created but couldn't be locked.");
                    }
                    return Err(InstanceLockError::Locked);
                }
            }
        }

        let data = if !lock_file_created {
            let mut data = InstanceLockData::<T>::read_from_file(&lock_file).inspect_err(|e| {
                util::debug_log_error!("Failed to read from file: {e}");
            })?;

            if data.app_version != version::APP_VERSION {
                util::debug_log_warning!("Converting lock file to new version.");
                data.app_version = version::APP_VERSION.into();
            }

            data
        } else {
            let data = InstanceLockData::new(f());
            data.save_to_file(&lock_file).inspect_err(|e| {
                util::debug_log_error!("Failed to save to file: {e}");
            })?;
            data
        };

        Ok(Self { data, lock_file })
    }

    /// The same as [Self::new] but [T::default](Default::default) is used
    /// instead of a callback function.
    pub fn from_default() -> Result<Self, InstanceLockError>
    where
        T: Default,
    {
        Self::new(T::default)
    }

    /// Access the saved data.
    pub fn data(&self) -> &T {
        &self.data.data
    }

    /// Access the saved data *mutably*, saving the data after.
    pub fn with_data<F>(&mut self, f: F) -> Result<(), SavedFileError>
    where
        F: FnOnce(&mut T),
    {
        f(&mut self.data.data);
        self.data.save_to_file(&self.lock_file)
    }
}

impl<T: SavedFile> Drop for InstanceLock<T> {
    fn drop(&mut self) {
        _ = self.lock_file.unlock().inspect_err(|e| {
            util::debug_log_error!("Failed to unlock instance lock file in `Drop` (ignoring): {e}");
        });
    }
}

/// Indicates that something went wrong trying to acquire an [InstanceLock].
#[derive(Error, Debug)]
pub enum InstanceLockError {
    #[error("The instance lock is already locked.")]
    Locked,
    #[error("Something went wrong with the instance lock file: {0}")]
    SavedFileError(#[from] SavedFileError),
}

impl From<io::Error> for InstanceLockError {
    fn from(e: io::Error) -> Self {
        SavedFileError::from(e).into()
    }
}

const LOCK_FILE_NAME: &str = "launcher.json";

#[derive(Serialize, Deserialize, Debug)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
struct InstanceLockData<T> {
    app_version: String,
    data: T,
}

impl<T> InstanceLockData<T> {
    /// Create an instance that uses the app's current version.
    pub fn new(data: T) -> Self {
        Self {
            app_version: version::APP_VERSION.into(),
            data,
        }
    }
}
