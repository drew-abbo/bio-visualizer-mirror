//! Defines the [Version] type and the things related to it.

use std::fmt::{self, Display, Formatter};

use thiserror::Error;

/// A macro for creating a [Version] at compile-time. The input string should
/// look like `{MAJOR}.{MINOR}.{INCREMENT}`.
///
/// # Example
///
/// ```
/// use util::version::Version;
/// assert_eq!(util::version_const!("0.1.2"), Version(0, 1, 2));
/// ```
#[macro_export]
macro_rules! version_const {
    ($s: literal) => {{
        match $crate::version::Version::try_from_str($s) {
            Ok(v) => v,
            Err(_) => panic!("Invalid version string format. Expected something like `0.1.2`."),
        }
    }};
}

/// A version (e.g. `0.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// The major version (e.g. the `0` in `0.1.2`).
    pub const fn major(&self) -> u32 {
        self.0
    }

    /// The minor version (e.g. the `1` in `0.1.2`).
    pub const fn minor(&self) -> u32 {
        self.1
    }

    /// The version increment (e.g. the `2` in `0.1.2`).
    pub const fn increment(&self) -> u32 {
        self.2
    }

    /// The same as `try_from::<&str>`, just `const`. Also see the
    /// [const_version] macro.
    pub const fn try_from_str(version_str: &str) -> Result<Self, VersionStrError> {
        let mut result: [Option<u32>; 3] = [None, None, None];
        let mut curr_result_section: usize = 0;

        let mut i: usize = 0;
        let version_chars = version_str.as_bytes();
        while i < version_chars.len() {
            match version_chars[i] {
                b'.' => {
                    if curr_result_section == 2 {
                        return Err(VersionStrError);
                    }
                    curr_result_section += 1;
                }

                digit if digit >= b'0' && digit <= b'9' => {
                    let result_section = &mut result[curr_result_section];
                    *result_section = match *result_section {
                        Some(val) => {
                            if val == 0 {
                                return Err(VersionStrError);
                            }

                            let Some(val) = val.checked_mul(10) else {
                                return Err(VersionStrError);
                            };
                            let Some(val) = val.checked_add((digit - b'0') as u32) else {
                                return Err(VersionStrError);
                            };
                            Some(val)
                        }

                        None => Some((digit - b'0') as u32),
                    };
                }

                _ => return Err(VersionStrError),
            }

            i += 1;
        }

        let Some(major) = result[0] else {
            return Err(VersionStrError);
        };
        let Some(minor) = result[1] else {
            return Err(VersionStrError);
        };
        let Some(increment) = result[2] else {
            return Err(VersionStrError);
        };
        Ok(Self(major, minor, increment))
    }
}

impl TryFrom<&str> for Version {
    type Error = VersionStrError;

    fn try_from(version_str: &str) -> Result<Self, Self::Error> {
        Self::try_from_str(version_str.as_ref())
    }
}

impl From<(u32, u32, u32)> for Version {
    fn from(version: (u32, u32, u32)) -> Self {
        Self(version.0, version.1, version.2)
    }
}

impl Into<(u32, u32, u32)> for Version {
    fn into(self) -> (u32, u32, u32) {
        (self.0, self.1, self.2)
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Indicates that a string could not be converted to a [Version].
#[derive(Error, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("The string could not be converted to a version.")]
pub struct VersionStrError;
