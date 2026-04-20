//! [StrN], a [CStr](std::ffi::CStr)-like string stored on the stack with a max
//! size of `N`.

use std::cmp::Ordering;
use std::fmt::{self, Debug, Display};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::Utf8Error;

/// A [CStr](std::ffi::CStr)-like string stored on the stack with a max size of
/// `N`.
///
/// Because this type is [Copy], it's advised you don't construct with a large
/// `N` value.
///
/// This type will always have the same size and alignment as a `[u8; N]`.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct StrN<const N: usize>([u8; N]);

impl<const N: usize> StrN<N> {
    /// The maximum length of this string.
    pub const N: usize = N;

    /// Create a new [StrN].
    ///
    /// If the length of `s` isn't less than or equal to `N` or if `s` contains
    /// a null terminator `b'\0'` [an error](StrNError) will be returned.
    ///
    /// For compile-time construction see [Self::from_str].
    #[inline]
    pub fn new(s: impl AsRef<str>) -> Result<Self, StrNError> {
        Self::from_str(s.as_ref())
    }

    /// Create a new [StrN] from a [str].
    ///
    /// If the length of `s` isn't less than or equal to `N` or if `s` contains
    /// a null terminator `b'\0'` [an error](StrNError) will be returned.
    pub const fn from_str(s: &str) -> Result<Self, StrNError> {
        if s.len() > N {
            return Err(StrNError::TooLong { n: N });
        }
        let s = s.as_bytes();

        let mut ret = [b'\0'; N];

        let mut i = 0;
        while i < s.len() {
            if s[i] == b'\0' {
                return Err(StrNError::NullByte { n: N, null_idx: i });
            }

            ret[i] = s[i];
            i += 1;
        }

        Ok(Self(ret))
    }

    /// Like [Self::from_str] but the result is effectively
    /// [unwrapped](Result::unwrap). Use this method to construct a [StrN] at
    /// compile-time.
    ///
    /// # Panics
    ///
    /// If the length of `s` isn't less than or equal to `N` or if `s` contains
    /// a null terminator `b'\0'` this function will panic.
    #[inline]
    pub const fn from_str_unwrapped(s: &str) -> Self {
        match Self::from_str(s) {
            Ok(s) => s,
            Err(_) => panic!("Failed to construct StrN"),
        }
    }

    /// Create a new [StrN] from a raw byte array.
    ///
    /// If the length of `s` isn't less than or equal to `N`, if `s` isn't
    /// UTF-8, or if `s` contains a null terminator `b'\0'`
    /// [an error](StrNError) will be returned.
    pub const fn from_bytes(s: &[u8]) -> Result<Self, StrNError> {
        if s.len() > N {
            return Err(StrNError::TooLong { n: N });
        }

        if let Err(utf8_error) = str::from_utf8(s) {
            return Err(StrNError::Utf8Error { n: N, utf8_error });
        }

        let mut ret = [b'\0'; N];

        let mut i = 0;
        while i < s.len() {
            if s[i] == b'\0' {
                return Err(StrNError::NullByte { n: N, null_idx: i });
            }

            ret[i] = s[i];
            i += 1;
        }

        Ok(Self(ret))
    }

    /// Creates a [`StrN<N>`] from a [`StrN<M>`], returning an error if
    /// `s.len()` is greater than `N`.
    ///
    /// When `M <= N`, this function will *never* return an [Err].
    pub const fn from_strn<const M: usize>(s: StrN<M>) -> Result<Self, StrNError> {
        if const { M <= N } || s.len() <= N {
            let mut ret = [b'\0'; N];
            let mut i = 0;
            while i < s.len() {
                ret[i] = s.as_bytes()[i];
                i += 1;
            }
            Ok(Self(ret))
        } else {
            Err(StrNError::TooLong { n: N })
        }
    }

    /// The number of bytes in the string before a null terminator `b'\0'` (or
    /// `N`).
    ///
    /// This function has an `O(N)` time complexity.
    #[inline]
    pub const fn len(&self) -> usize {
        let mut i = 0;
        while i < N && self.0[i] != b'\0' {
            i += 1;
        }
        i
    }

    /// Whether the [length](Self::len) of this string is 0.
    ///
    /// This function has an `O(N)` time complexity.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The first [Self::len] bytes of the internal buffer.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        self.0.split_at(self.len()).0
    }

    /// View this [StrN] as a [str].
    #[inline]
    pub const fn as_str(&self) -> &str {
        // SAFETY: The buffer is UTF-8.
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// A reference to the internal `N`-byte buffer.
    ///
    /// The buffer is *not* guaranteed to be null-terminated (if [Self::len] is
    /// `N`).
    #[inline(always)]
    pub const fn as_buffer(&self) -> &[u8; N] {
        &self.0
    }

    /// A *mutable* reference to the internal `N`-byte buffer.
    ///
    /// The buffer is *not* guaranteed to be null-terminated (if [Self::len] is
    /// `N`).
    ///
    /// # Safety
    ///
    /// The internal buffer should remain [valid UTF-8](str::from_utf8) and all
    /// bytes after a null byte `b'\0'` should also be null bytes.
    #[inline(always)]
    pub const unsafe fn as_buffer_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl<const N: usize> Default for StrN<N> {
    #[inline(always)]
    fn default() -> Self {
        const { Self::from_str_unwrapped("") }
    }
}

impl<const N: usize> Debug for StrN<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StrN").field(&self.as_str()).finish()
    }
}

impl<const N: usize> Display for StrN<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const N: usize> AsRef<str> for StrN<N> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> Deref for StrN<N> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const N: usize> From<StrN<N>> for String {
    #[inline]
    fn from(s: StrN<N>) -> Self {
        String::from(s.as_str())
    }
}

impl<const N: usize> TryFrom<&str> for StrN<N> {
    type Error = StrNError;

    #[inline(always)]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::from_str(s)
    }
}

impl<const N: usize, S: AsRef<str>> PartialEq<S> for StrN<N> {
    #[inline]
    fn eq(&self, other: &S) -> bool {
        self.as_str() == other.as_ref()
    }
}
impl<const N: usize> Eq for StrN<N> {}

impl<const N: usize, S: AsRef<str>> PartialOrd<S> for StrN<N> {
    #[inline]
    fn partial_cmp(&self, other: &S) -> Option<Ordering> {
        Some(self.as_str().cmp(other.as_ref()))
    }
}
impl<const N: usize> Ord for StrN<N> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl<const N: usize> Hash for StrN<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl StrN<1> {
    /// The internal buffer reinterpreted as a [u8].
    #[inline(always)]
    pub const fn as_u8(&self) -> u8 {
        self.0[0]
    }
}
impl StrN<2> {
    /// The internal buffer reinterpreted as a [u16].
    #[inline(always)]
    pub const fn as_u16(&self) -> u16 {
        u16::from_ne_bytes(self.0)
    }
}
impl StrN<4> {
    /// The internal buffer reinterpreted as a [u32].
    #[inline(always)]
    pub const fn as_u32(&self) -> u32 {
        u32::from_ne_bytes(self.0)
    }
}
impl StrN<8> {
    /// The internal buffer reinterpreted as a [u64].
    #[inline(always)]
    pub const fn as_u64(&self) -> u64 {
        u64::from_ne_bytes(self.0)
    }
}
impl StrN<16> {
    /// The internal buffer reinterpreted as a [u128].
    #[inline(always)]
    pub const fn as_u128(&self) -> u128 {
        u128::from_ne_bytes(self.0)
    }
}

/// An error invovling the construction of a [StrN].
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrNError {
    #[error("The source string was longer than {n} bytes.")]
    TooLong { n: usize },
    #[error("The source string contained a null-byte at index {null_idx}")]
    NullByte { n: usize, null_idx: usize },
    #[error("{utf8_error}")]
    Utf8Error { n: usize, utf8_error: Utf8Error },
}

impl StrNError {
    /// The `N` value of the [StrN] that caused this error.
    #[inline]
    pub fn n(&self) -> usize {
        match *self {
            StrNError::TooLong { n } => n,
            StrNError::NullByte { n, .. } => n,
            StrNError::Utf8Error { n, .. } => n,
        }
    }
}
