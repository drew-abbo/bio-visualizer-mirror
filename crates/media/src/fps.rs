//! Defines [Fps], a numeric type used to represent a frame rate.

use std::fmt::{self, Display};
use std::ops::{Add, AddAssign, Deref, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

use thiserror::Error;

/// A numeric type used to represent frame rates (the amount of frames per
/// second, i.e. [Hz](https://en.wikipedia.org/wiki/Hertz)).
///
/// This type is a wrapper around a positive and [normal](f64::is_normal) [f64].
///
/// This type supports all the normal operators that you'd expect for a numeric
/// floating point type. Note that any arithmetic operation that would result in
/// an invalid inner float (a value that is either non-positive or
/// [abnormal](f64::is_normal)) will result in that function panicking.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Fps(f64);

impl Fps {
    /// Create a new [Fps]. If `fps` is [abnormal](f64::is_normal) or
    /// non-positive, this function will panic.
    ///
    /// See [Self::new_checked] if you'd rather get an error in this case.
    pub const fn new(fps: f64) -> Self {
        match Self::new_checked(fps) {
            Ok(frame_rate) => frame_rate,
            Err(_) => panic!("Invalid float for frame rate."),
        }
    }

    /// Create a new [Fps]. If `fps` is [abnormal](f64::is_normal) or
    /// non-positive, this function will return an error.
    ///
    /// See [Self::new] if you'd rather panic in this case.
    pub const fn new_checked(fps: f64) -> Result<Self, FPSError> {
        if !fps.is_normal() {
            Err(FPSError::Abnormal)
        } else if fps <= 0.0 {
            Err(FPSError::NonPositive)
        } else {
            Ok(Self(fps))
        }
    }

    /// Like [Self::new_checked] but without the input validation.
    ///
    /// # Safety
    ///
    /// Calling this function with an [abnormal](f64::is_normal) or non-positive
    /// `fps` value is undefined behavior.
    pub const unsafe fn new_unchecked(fps: f64) -> Self {
        Self(fps)
    }

    /// Get the inner value as an [f64]. The returned value will *always* be
    /// positive and [normal](f64::is_normal).
    pub const fn as_f64(&self) -> f64 {
        self.assume_self_normal_positive();
        self.0
    }

    /// Asserts to the compiler that the inner value [Self::0] is normal and
    /// positive. This should always be called immediately before directly
    /// accessing [Self::0].
    #[inline(always)]
    const fn assume_self_normal_positive(&self) {
        // SAFETY: The constructors validate that the inner value is positive
        // and normal.
        unsafe { std::hint::assert_unchecked(self.0.is_normal() && self.0 > 0.0) };
    }
}

impl From<Fps> for f64 {
    fn from(fps: Fps) -> Self {
        fps.as_f64()
    }
}

impl From<f64> for Fps {
    fn from(fps: f64) -> Self {
        Self::new(fps)
    }
}

impl Display for Fps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_f64().fmt(f)
    }
}

impl Deref for Fps {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        self.assume_self_normal_positive();
        &self.0
    }
}

// These macros makes it so we have significantly less boilerplate than we would
// doing all of this 4 times (once per operator).
macro_rules! impl_operator {
    (+) => { impl_operator!(@impl +, Add, add, AddAssign, add_assign); };
    (-) => { impl_operator!(@impl -, Sub, sub, SubAssign, sub_assign); };
    (*) => { impl_operator!(@impl *, Mul, mul, MulAssign, mul_assign); };
    (/) => { impl_operator!(@impl /, Div, div, DivAssign, div_assign); };

    (@impl doc) => {
        concat!(
            "If this operation would result in an [abnormal](f64::is_normal)",
            " or non-positive [FPS](Fps), this function will panic."
        )
    };

    (@impl $op:tt,
     $main_trait:ident,
     $main_method:ident,
     $assign_trait:ident,
     $assign_method:ident
    ) => {

        // FPS ? FPS

        impl $main_trait<Fps> for Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: Fps) -> Fps {
                Fps::new(self.as_f64() $op rhs.as_f64())
            }
        }

        impl $main_trait<Fps> for &Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: Fps) -> Fps {
                Fps::new(self.as_f64() $op rhs.as_f64())
            }
        }

        impl $main_trait<&Fps> for Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: &Fps) -> Fps {
                Fps::new(self.as_f64() $op rhs.as_f64())
            }
        }

        impl $main_trait<&Fps> for &Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: &Fps) -> Fps {
                Fps::new(self.as_f64() $op rhs.as_f64())
            }
        }

        // FPS ? f64

        impl $main_trait<f64> for Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: f64) -> Fps {
                Fps::new(self.as_f64() $op rhs)
            }
        }

        impl $main_trait<f64> for &Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: f64) -> Fps {
                Fps::new(self.as_f64() $op rhs)
            }
        }

        impl $main_trait<&f64> for Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: &f64) -> Fps {
                Fps::new(self.as_f64() $op rhs)
            }
        }

        impl $main_trait<&f64> for &Fps {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: &f64) -> Fps {
                Fps::new(self.as_f64() $op rhs)
            }
        }

        // f64 ? FPS

        impl $main_trait<Fps> for f64 {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: Fps) -> Fps {
                Fps::new(self $op rhs.as_f64())
            }
        }

        impl $main_trait<Fps> for &f64 {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: Fps) -> Fps {
                Fps::new(self $op rhs.as_f64())
            }
        }

        impl $main_trait<&Fps> for f64 {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: &Fps) -> Fps {
                Fps::new(self $op rhs.as_f64())
            }
        }

        impl $main_trait<&Fps> for &f64 {
            type Output = Fps;

            #[doc = impl_operator!(@impl doc)]
            fn $main_method(self, rhs: &Fps) -> Fps {
                Fps::new(self $op rhs.as_f64())
            }
        }

        // FPS ?= FPS

        impl $assign_trait<Fps> for Fps {
            #[doc = impl_operator!(@impl doc)]
            fn $assign_method(&mut self, rhs: Fps) {
                *self = Fps::new(self.as_f64() $op rhs.as_f64());
            }
        }

        impl $assign_trait<&Fps> for Fps {
            #[doc = impl_operator!(@impl doc)]
            fn $assign_method(&mut self, rhs: &Fps) {
                *self = Fps::new(self.as_f64() $op rhs.as_f64());
            }
        }

        // FPS ?= f64

        impl $assign_trait<f64> for Fps {
            #[doc = impl_operator!(@impl doc)]
            fn $assign_method(&mut self, rhs: f64) {
                *self = Fps::new(self.as_f64() $op rhs);
            }
        }

        impl $assign_trait<&f64> for Fps {
            #[doc = impl_operator!(@impl doc)]
            fn $assign_method(&mut self, rhs: &f64) {
                *self = Fps::new(self.as_f64() $op rhs);
            }
        }

    };
}

impl_operator!(+);
impl_operator!(-);
impl_operator!(*);
impl_operator!(/);

/// Indicates that something went wrong constructing an [Fps] object.
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FPSError {
    /// A frame rate cannot be an [abnormal](f64::is_normal).
    #[error("A frame rate must be a normal number.")]
    Abnormal,
    /// A frame rate must be a positive number.
    #[error("A frame rate must be a positive number.")]
    NonPositive,
}
