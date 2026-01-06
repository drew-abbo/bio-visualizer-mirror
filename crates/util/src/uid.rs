//! Contains [Uid], useful for generating unique IDs.

use std::fmt::{self, Display, Formatter};
use std::process;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use thiserror::Error;

/// A unique ID.
///
/// No 2 calls to [Self::default] will generate the same [Uid], regardless of
/// what process generates them (so long as the system time doesn't roll back,
/// even then it's highly unlikely).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(into = "String", try_from = "String")]
pub struct Uid {
    time: u128,
    pid: u32,
    idx: u32,
}

impl Default for Uid {
    fn default() -> Self {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let pid = process::id();

        static NEXT_INNER_PROCESS_IDX: AtomicU32 = AtomicU32::new(0);
        let idx = NEXT_INNER_PROCESS_IDX.fetch_add(1, Ordering::Relaxed);

        Self { time, pid, idx }
    }
}

impl Display for Uid {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}-{:x}-{:x}", self.time, self.pid, self.idx)
    }
}

impl From<Uid> for String {
    fn from(val: Uid) -> Self {
        val.to_string()
    }
}

impl TryFrom<&str> for Uid {
    type Error = UidFromStrError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut parts = s.split("-");

        let time = parts
            .next()
            .and_then(|part| u128::from_str_radix(part, 16).ok())
            .ok_or(UidFromStrError)?;

        let pid = parts
            .next()
            .and_then(|part| u32::from_str_radix(part, 16).ok())
            .ok_or(UidFromStrError)?;

        let idx = parts
            .next()
            .and_then(|part| u32::from_str_radix(part, 16).ok())
            .ok_or(UidFromStrError)?;

        if parts.next().is_some() {
            return Err(UidFromStrError);
        }

        Ok(Self { time, pid, idx })
    }
}

impl TryFrom<&mut str> for Uid {
    type Error = UidFromStrError;

    fn try_from(s: &mut str) -> Result<Self, Self::Error> {
        <Self as TryFrom<&str>>::try_from(s)
    }
}

impl TryFrom<String> for Uid {
    type Error = UidFromStringError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str()).map_err(|_| UidFromStringError(s))
    }
}

/// Indicates that a [str] reference couldn't be converted to a [Uid].
#[derive(Error, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("Invalid UID string.")]
pub struct UidFromStrError;

impl From<UidFromStringError> for UidFromStrError {
    fn from(_e: UidFromStringError) -> Self {
        Self
    }
}

/// Indicates that a [String] couldn't be converted to a [Uid].
#[derive(Error, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("Invalid UID string `{0}`")]
pub struct UidFromStringError(pub String);
