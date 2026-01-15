//! Contains an inter-process communication (IPC) system for instances of the
//! launcher.
//!
//! "OI" is short for "Other Instance".

use std::collections::VecDeque;
use std::fmt::{self, Display, Formatter, Write as FmtWrite};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use thiserror::Error;

use util::local_data;

/// A message from one instance to another.
///
/// "OI" is short for "Other Instance".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OIMsg {
    /// Another instance was launched and is now exiting. Focus the current
    /// instance to indicate this.
    Focus = b'F',

    /// Another instance was launched by the editor to tell the current instance
    /// that a project was saved.
    ProjectUpdated = b'U',

    /// Another instance was launched by the editor to tell the current instance
    /// that a project couldn't be opened.
    ProjectOpenFailed = b'O',

    /// Another instance was launched by the editor to tell the current instance
    /// to close.
    Close = b'C',
}
// IMPORTANT: When adding/changing these variants, make sure to update the
// `TryFrom<u8>` implementation (you won't automatically get an error telling
// you to fix it).

impl TryFrom<u8> for OIMsg {
    type Error = InvalidOIMsgByte;

    fn try_from(char: u8) -> Result<Self, Self::Error> {
        match char {
            b'F' => Ok(Self::Focus),
            b'U' => Ok(Self::ProjectUpdated),
            b'O' => Ok(Self::ProjectOpenFailed),
            b'C' => Ok(Self::Close),
            byte => Err(InvalidOIMsgByte(byte)),
        }
    }
}

impl From<OIMsg> for u8 {
    fn from(msg: OIMsg) -> Self {
        msg as u8
    }
}

impl From<OIMsg> for char {
    fn from(msg: OIMsg) -> Self {
        msg as u8 as char
    }
}

impl Display for OIMsg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_char((*self).into())
    }
}

/// For receiving messages ([OIMsg]s) from [OIMsgSender]s on other instances.
/// This object should *only* exist on the main instance.
///
/// "OI" is short for "Other Instance".
#[derive(Debug)]
pub struct OIMsgReceiver {
    file: Option<File>,
    file_path: PathBuf,
    msg_queue: VecDeque<OIMsg>,
}

impl OIMsgReceiver {
    /// Create a receiver.
    ///
    /// The program will gracefully exit if this fails.
    pub fn new() -> Result<Self, io::Error> {
        let file_path = channel_file_path();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .inspect_err(|e| {
                util::debug_log_error!("Failed to create IPC file file: {e}");
            })?;

        Ok(Self {
            file: Some(file),
            file_path,
            msg_queue: VecDeque::default(),
        })
    }

    /// Receive an available message if there is one.
    ///
    /// The program will gracefully exit if this fails.
    pub fn receive(&mut self) -> Result<Option<OIMsg>, io::Error> {
        if let Some(msg) = self.msg_queue.pop_front() {
            return Ok(Some(msg));
        }

        let mut buf = with_file_locked_mut(self.file_mut(), |file| {
            file.seek(SeekFrom::Start(0))?;

            let mut buf = vec![];
            file.read_to_end(&mut buf)?;
            file.set_len(0)?;

            Ok(buf)
        })
        .inspect_err(|e| {
            util::debug_log_error!("Failed to read from IPC file: {e}");
        })?;

        self.msg_queue.reserve(buf.len());
        for byte in buf.drain(..) {
            let msg = match OIMsg::try_from(byte) {
                Ok(msg) => msg,
                Err(e) => {
                    util::debug_log_warning!("{e}");
                    continue;
                }
            };

            self.msg_queue.push_back(msg);
        }

        Ok(self.msg_queue.pop_front())
    }

    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect(Self::FILE_EXPECT_MSG)
    }

    const FILE_EXPECT_MSG: &str = "The file should be present.";
}

impl Drop for OIMsgReceiver {
    fn drop(&mut self) {
        let file = self.file.take().expect(Self::FILE_EXPECT_MSG);
        drop(file);

        _ = fs::remove_file(&self.file_path).inspect_err(|e| {
            util::debug_log_error!("Failed to remove IPC file in `Drop` (ignoring): {e}");
        });
    }
}

/// For sending messages ([OIMsg]s) to an [OIMsgReceiver] on the main instance.
///
/// "OI" is short for "Other Instance".
pub struct OIMsgSender {
    file: File,
}

impl OIMsgSender {
    /// Create a sender.
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {
            file: OpenOptions::new().append(true).open(channel_file_path())?,
        })
    }

    /// Send a message.
    pub fn send(&mut self, msg: OIMsg) -> Result<(), io::Error> {
        self.file.write_all(&[msg.into()]).inspect_err(|e| {
            util::debug_log_error!("Failed to append to IPC file: {e}");
        })
    }
}

/// Indicates that an invalid message byte was sent.
#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("Invalid message byte `{0}` from other instance.")]
pub struct InvalidOIMsgByte(pub u8);

fn channel_file_path() -> PathBuf {
    local_data::root_path().join("other_instance_msgs.txt")
}

/// Lock the file, call `f` on it, then unlock the file. If unlocking fails,
/// that error will be returned instead of any errors `f` may have produced.
///
/// The file *must* have been opened with read and write permissions.
fn with_file_locked_mut<F, T>(file: &mut File, f: F) -> Result<T, io::Error>
where
    F: FnOnce(&mut File) -> Result<T, io::Error>,
{
    file.lock()?;
    let ret = f(file);
    file.unlock()?;
    ret
}
