//! Defines [Fps], a numeric type used to represent a frame rate, along with
//! some helpers.

use std::cmp::{Ord, Ordering, PartialOrd};
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::{Add, Div, Mul, Sub};

use thiserror::Error;

/// The greatest common denominator between `$a` and `$b`.
macro_rules! gcd {
    ($a:expr, $b:expr) => {{
        let (mut a, mut b) = ($a, $b);
        while b != 0 {
            (b, a) = (a % b, b)
        }
        a
    }};
}

/// [Fps] constants (common frame rates).
pub mod consts {
    use super::Fps;

    macro_rules! const_fps {
        ($fps:literal) => {
            match Fps::from_float($fps as f64) {
                Ok(fps) => fps,
                Err(_) => panic!(),
            }
        };
    }

    /// 1 frame per second.
    pub const FPS_1: Fps = const_fps!(1);
    /// 8 frames per second (animation on 3s).
    pub const FPS_8: Fps = const_fps!(8);
    /// 15 frames per second.
    pub const FPS_15: Fps = const_fps!(15);
    /// 12 frames per second (animation on 2s).
    pub const FPS_12: Fps = const_fps!(12);
    /// 23.976 frames per second (NTSC film).
    pub const FPS_23_976: Fps = const_fps!(23.976);
    pub use FPS_23_976 as NTSC_FILM;
    /// 24 frames per second (film).
    pub const FPS_24: Fps = const_fps!(24);
    pub use FPS_24 as FILM;
    /// 25 frames per second (PAL).
    pub const FPS_25: Fps = const_fps!(25);
    pub use FPS_25 as PAL;
    /// 29.97 frames per second (NTSC).
    pub const FPS_29_97: Fps = const_fps!(29.97);
    pub use FPS_29_97 as NTSC;
    /// 30 frames per second.
    pub const FPS_30: Fps = const_fps!(30);
    /// 48 frames per second (HFR).
    pub const FPS_48: Fps = const_fps!(48);
    pub use FPS_48 as HFR;
    /// 50 frames per second.
    pub const FPS_50: Fps = const_fps!(50);
    /// 59.94 frames per second (NTSC high-frame-rate).
    pub const FPS_59_94: Fps = const_fps!(59.94);
    pub use FPS_59_94 as NTSC_HIGH;
    /// 60 frames per second.
    pub const FPS_60: Fps = const_fps!(60);
    /// 75 frames per second.
    pub const FPS_75: Fps = const_fps!(75);
    /// 90 frames per second.
    pub const FPS_90: Fps = const_fps!(90);
    /// 100 frames per second.
    pub const FPS_100: Fps = const_fps!(100);
    /// 120 frames per second.
    pub const FPS_120: Fps = const_fps!(120);
    /// 144 frames per second.
    pub const FPS_144: Fps = const_fps!(144);
    /// 165 frames per second.
    pub const FPS_165: Fps = const_fps!(165);
    /// 240 frames per second.
    pub const FPS_240: Fps = const_fps!(240);
    /// 280 frames per second.
    pub const FPS_280: Fps = const_fps!(280);
    /// 360 frames per second.
    pub const FPS_360: Fps = const_fps!(360);

    /// All frame rate constants from [crate::fps::consts]. Also see
    /// [common_frame_rate_name].
    pub const COMMON_FRAME_RATES: &[Fps] = &[
        FPS_1, FPS_8, FPS_15, FPS_12, FPS_23_976, FPS_24, FPS_25, FPS_29_97, FPS_30, FPS_48,
        FPS_50, FPS_59_94, FPS_60, FPS_75, FPS_90, FPS_100, FPS_120, FPS_144, FPS_165, FPS_240,
        FPS_280, FPS_360,
    ];

    /// Printable names for all frame rates in [COMMON_FRAME_RATES].
    pub const fn common_frame_rate_name(fps: Fps) -> Option<&'static str> {
        Some(match fps {
            FPS_1 => "1",
            FPS_8 => "8",
            FPS_15 => "15",
            FPS_12 => "12",
            FPS_23_976 => "23.976 (NTSC film)",
            FPS_24 => "24 (film)",
            FPS_25 => "25 (PAL)",
            FPS_29_97 => "29.97 (NTSC)",
            FPS_30 => "30",
            FPS_48 => "48 (HFR)",
            FPS_50 => "50",
            FPS_59_94 => "59.94 (NTSC high-frame-rate)",
            FPS_60 => "60",
            FPS_75 => "60",
            FPS_90 => "90",
            FPS_100 => "100",
            FPS_120 => "120",
            FPS_144 => "144",
            FPS_165 => "165",
            FPS_240 => "240",
            FPS_280 => "280",
            FPS_360 => "360",
            _ => return None,
        })
    }
}

/// A type representing a number of frames per second (i.e. i.e.
/// [Hz](https://en.wikipedia.org/wiki/Hertz)). If you need a common frame rate,
/// consider using a constant from the [consts] module instead of going through
/// a constructor.
///
/// This type stores a simplified fraction internally, not a floating point
/// value. The fraction will always be positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fps {
    num: NonZeroU32,
    den: NonZeroU32,
}

impl Fps {
    /// Constructs an [Fps] object from an integer. Also see [Self::from_frac].
    pub const fn from_int(fps: u32) -> Result<Self, FpsError> {
        let Some(num) = NonZeroU32::new(fps) else {
            return Err(FpsError::NonPositiveNum);
        };
        let den = const { NonZeroU32::new(1).unwrap() };
        Ok(Self { num, den })
    }

    /// Like [Self::from_int] but without checking if `fps` is zero.
    pub const fn from_non_zero_int(fps: NonZeroU32) -> Self {
        let den = const { NonZeroU32::new(1).unwrap() };
        Self { num: fps, den }
    }

    /// Constructs an [Fps] object from a fraction (which gets simplified). Also
    /// see [Self::from_int]
    pub const fn from_frac(num: u32, den: u32) -> Result<Self, FpsError> {
        let Some(num) = NonZeroU32::new(num) else {
            return Err(FpsError::NonPositiveNum);
        };
        let Some(den) = NonZeroU32::new(den) else {
            return Err(FpsError::NonPositiveDen);
        };

        Ok(Self::from_non_zero_frac(num, den))
    }

    /// Like [Self::from_frac] but without checking if `num` or `den` are zero.
    pub const fn from_non_zero_frac(num: NonZeroU32, den: NonZeroU32) -> Self {
        let gcd = gcd!(num.get(), den.get());
        debug_assert!(gcd >= 1);
        Self {
            // SAFETY: Dividing non-zero numbers by a gcd can't result in 0.
            num: unsafe { NonZeroU32::new_unchecked(num.get() / gcd) },
            den: unsafe { NonZeroU32::new_unchecked(den.get() / gcd) },
        }
    }

    /// Like [Self::from_frac] but without simplifying the fraction.
    ///
    /// # Safety
    ///
    /// The greatest common denominator between `num` and `den` must be `1`
    /// (i.e. the fraction must already be simplified).
    pub const unsafe fn from_simplified_frac(num: u32, den: u32) -> Result<Self, FpsError> {
        debug_assert!(gcd!(num, den) == 1);

        let Some(num) = NonZeroU32::new(num) else {
            return Err(FpsError::NonPositiveNum);
        };
        let Some(den) = NonZeroU32::new(den) else {
            return Err(FpsError::NonPositiveDen);
        };

        Ok(Self { num, den })
    }

    /// Like [Self::from_simplified_frac] but without checking if `num` or `den`
    /// are zero.
    ///
    /// # Safety
    ///
    /// The greatest common denominator between `num` and `den` must be `1`
    /// (i.e. the fraction must already be simplified).
    pub const unsafe fn from_simplified_non_zero_frac(num: NonZeroU32, den: NonZeroU32) -> Self {
        Self { num, den }
    }

    /// Try constructing an [Fps] object from a floating point number.
    ///
    /// Note that this function will try to approximate a simple fraction from
    /// `fps`, so some precision may be lost (see [Self::as_frac]). The
    /// resulting fraction will never have a denominator over `2048`.
    ///
    /// This function has special handling for numbers that look like common
    /// decimal approximations of NTSC frame rates:
    /// - If `fps` is near `23.976`, the fraction `24000/1001` will be used.
    /// - If `fps` is near `29.97`, the fraction `30000/1001` will be used.
    /// - If `fps` is near `59.94`, the fraction `60000/1001` will be used.
    ///
    /// Use [Self::from_float_raw] if you don't want the special NTSC handling.
    pub const fn from_float(fps: f64) -> Result<Self, FpsError> {
        // Special handling for NTSC frame rates.
        if is_near(fps, 23.976, Self::TOLERANCE) {
            return const { Fps::from_frac(24000, 1001) };
        } else if is_near(fps, 29.97, Self::TOLERANCE) {
            return const { Fps::from_frac(30000, 1001) };
        } else if is_near(fps, 59.94, Self::TOLERANCE) {
            return const { Fps::from_frac(60000, 1001) };
        }

        Self::from_float_raw(fps)
    }

    /// The same as [Self::from_float] but without the special NTSC handling.
    pub const fn from_float_raw(fps: f64) -> Result<Self, FpsError> {
        if fps < 0.0 {
            return Err(FpsError::NonPositiveFloat);
        }
        if is_near(fps, 0.0, Self::TOLERANCE) {
            return Err(FpsError::NearZeroFloat);
        }
        if fps > u32::MAX as f64 {
            return Err(FpsError::TooLargeFloat);
        }
        if !fps.is_normal() {
            return Err(FpsError::AbnormalFloat);
        }

        let nearest_int = fps.round();
        if is_near(fps, nearest_int, Self::TOLERANCE) {
            // SAFETY: We've already checked `fps` is near a positive integer.
            return Ok(Self::from_non_zero_int(unsafe {
                NonZeroU32::new_unchecked(nearest_int as u32)
            }));
        }

        const MAX_DEN: u32 = 2048;
        let Some((num, den)) = float_to_frac(fps, MAX_DEN, Self::TOLERANCE) else {
            return Err(FpsError::NoFracApproximation);
        };
        debug_assert!(num != 0 && den != 0 && gcd!(num, den) == 1);

        // SAFETY: `float_to_frac()` will return a simplified non-zero fraction.
        Ok(unsafe {
            Self::from_simplified_non_zero_frac(
                NonZeroU32::new_unchecked(num),
                NonZeroU32::new_unchecked(den),
            )
        })
    }

    /// Get the frame rate as a simplified fraction (numerator and denominator
    /// pair).
    pub const fn as_frac(&self) -> (u32, u32) {
        (self.num.get(), self.den.get())
    }

    /// Like [Self::as_frac], but the return values are non-zero.
    pub const fn as_non_zero_frac(&self) -> (NonZeroU32, NonZeroU32) {
        (self.num, self.den)
    }

    /// The numerator from [Self::as_frac].
    pub const fn num(&self) -> u32 {
        self.num.get()
    }

    /// The denominator from [Self::as_frac].
    pub const fn den(&self) -> u32 {
        self.den.get()
    }

    /// Like [Self::num], but the return value is non-zero.
    pub const fn num_non_zero(&self) -> NonZeroU32 {
        self.num
    }

    /// Like [Self::den], but the return value is non-zero.
    pub const fn den_non_zero(&self) -> NonZeroU32 {
        self.den
    }

    /// Get this framerate as a whole number of frames per second *if the frame
    /// rate can be represented as a fraction over 1*. Also see [Self::as_frac].
    pub const fn as_int(&self) -> Result<u32, FpsError> {
        if self.den.get() == 1 {
            Ok(self.num.get())
        } else {
            Err(FpsError::NoIntRepresentation)
        }
    }

    /// Like [Self::as_int], but the return value is non-zero.
    pub const fn as_non_zero_int(&self) -> Result<NonZeroU32, FpsError> {
        if self.den.get() == 1 {
            Ok(self.num)
        } else {
            Err(FpsError::NoIntRepresentation)
        }
    }

    /// Get the frame rate as a float.
    ///
    /// Note that if this [Fps] was constructed with [Self::from_float], the
    /// result of this function may be surprising. See [Self::from_float] for
    /// more information.
    pub const fn as_float(&self) -> f64 {
        self.num.get() as f64 / self.den.get() as f64
    }

    /// Get the inverse of this framerate (e.g. `1/2` becomes `2/1`).
    pub const fn inverse(&self) -> Self {
        Self {
            num: self.den,
            den: self.num,
        }
    }

    const TOLERANCE: f64 = 1e-9;
}

impl PartialOrd for Fps {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Fps {
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = (self.num.get() as u64) * (other.den.get() as u64);
        let rhs = (other.num.get() as u64) * (self.den.get() as u64);
        lhs.cmp(&rhs)
    }
}

const NO_OVERFLOW: &str = "The operation shouldn't overflow or underflow.";

impl Mul<Fps> for Fps {
    type Output = Fps;

    fn mul(self, rhs: Fps) -> Fps {
        let new_num = self.num().checked_mul(rhs.num()).expect(NO_OVERFLOW);
        let new_den = self.den().checked_mul(rhs.den()).expect(NO_OVERFLOW);
        Self::from_non_zero_frac(
            // SAFETY: `n * m != 0` if `n, m > 0`
            unsafe { NonZeroU32::new_unchecked(new_num) },
            unsafe { NonZeroU32::new_unchecked(new_den) },
        )
    }
}

impl Div<Fps> for Fps {
    type Output = Fps;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Fps) -> Fps {
        self * rhs.inverse()
    }
}

impl Add<Fps> for Fps {
    type Output = Fps;

    fn add(self, rhs: Fps) -> Fps {
        let (lhs_num, lhs_den) = (
            self.num().checked_mul(rhs.den()).expect(NO_OVERFLOW),
            self.den().checked_mul(rhs.den()).expect(NO_OVERFLOW),
        );
        let (rhs_num, rhs_den) = (
            rhs.num().checked_mul(self.den()).expect(NO_OVERFLOW),
            rhs.den().checked_mul(self.den()).expect(NO_OVERFLOW),
        );
        let new_num = lhs_num.checked_add(rhs_num).expect(NO_OVERFLOW);
        let new_den = lhs_den.checked_add(rhs_den).expect(NO_OVERFLOW);

        Self::from_non_zero_frac(
            // SAFETY: `n * m != 0` and `n + m != 0` if `n, m > 0`
            unsafe { NonZeroU32::new_unchecked(new_num) },
            unsafe { NonZeroU32::new_unchecked(new_den) },
        )
    }
}

impl Sub<Fps> for Fps {
    type Output = Fps;

    fn sub(self, rhs: Fps) -> Fps {
        assert!(self > rhs, "FPS subtraction result can't be non-positive.");

        let (lhs_num, lhs_den) = (
            self.num().checked_mul(rhs.den()).expect(NO_OVERFLOW),
            self.den().checked_mul(rhs.den()).expect(NO_OVERFLOW),
        );
        let (rhs_num, rhs_den) = (
            rhs.num().checked_mul(self.den()).expect(NO_OVERFLOW),
            rhs.den().checked_mul(self.den()).expect(NO_OVERFLOW),
        );
        let new_num = lhs_num.checked_sub(rhs_num).expect(NO_OVERFLOW);
        let new_den = lhs_den.checked_sub(rhs_den).expect(NO_OVERFLOW);

        Self::from_non_zero_frac(
            // SAFETY: `n * m != 0` and `n - m != 0` if `n, m > 0` and `n > m`
            unsafe { NonZeroU32::new_unchecked(new_num) },
            unsafe { NonZeroU32::new_unchecked(new_den) },
        )
    }
}

impl TryFrom<u32> for Fps {
    type Error = FpsError;

    fn try_from(fps: u32) -> Result<Self, Self::Error> {
        Self::from_int(fps)
    }
}

impl From<NonZeroU32> for Fps {
    fn from(fps: NonZeroU32) -> Self {
        Self::from_non_zero_int(fps)
    }
}

impl TryFrom<(u32, u32)> for Fps {
    type Error = FpsError;

    fn try_from((num, den): (u32, u32)) -> Result<Self, Self::Error> {
        Self::from_frac(num, den)
    }
}

impl From<(NonZeroU32, NonZeroU32)> for Fps {
    fn from((num, den): (NonZeroU32, NonZeroU32)) -> Self {
        Self::from_non_zero_frac(num, den)
    }
}

impl TryFrom<f64> for Fps {
    type Error = FpsError;

    fn try_from(fps: f64) -> Result<Self, Self::Error> {
        Self::from_float(fps)
    }
}

impl TryFrom<f32> for Fps {
    type Error = FpsError;

    fn try_from(fps: f32) -> Result<Self, Self::Error> {
        Self::from_float(fps as f64)
    }
}

impl TryFrom<Fps> for u32 {
    type Error = FpsError;

    fn try_from(fps: Fps) -> Result<Self, Self::Error> {
        fps.as_int()
    }
}

impl TryFrom<Fps> for NonZeroU32 {
    type Error = FpsError;

    fn try_from(fps: Fps) -> Result<Self, Self::Error> {
        fps.as_non_zero_int()
    }
}

impl From<Fps> for (u32, u32) {
    fn from(fps: Fps) -> Self {
        fps.as_frac()
    }
}

impl From<Fps> for (NonZeroU32, NonZeroU32) {
    fn from(fps: Fps) -> Self {
        fps.as_non_zero_frac()
    }
}

impl From<Fps> for f64 {
    fn from(fps: Fps) -> Self {
        fps.as_float()
    }
}

impl From<Fps> for f32 {
    fn from(fps: Fps) -> Self {
        fps.as_float() as f32
    }
}

/// Indicates that an opearation with an [Fps] object failed.
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpsError {
    /// A frame rate cannot be created with a non-positive numerator.
    #[error("A frame rate cannot be created with a non-positive numerator.")]
    NonPositiveNum,

    /// A frame rate cannot be created with a non-positive denominator.
    #[error("A frame rate cannot be created with a non-positive denominator.")]
    NonPositiveDen,

    /// A frame rate cannot be created from an non-positive float.
    #[error("A frame rate cannot be created from an non-positive float.")]
    NonPositiveFloat,

    /// A frame rate cannot be created from an near-zero float.
    #[error("A frame rate cannot be created from an near-zero float.")]
    NearZeroFloat,

    /// A frame rate cannot be created from a float over [u32::MAX].
    #[error("A frame rate cannot be created from an exceptionally large float.")]
    TooLargeFloat,

    /// A frame rate cannot be created from an [abnormal](f64::is_normal) float.
    #[error("A frame rate cannot be created from an abnormal float.")]
    AbnormalFloat,

    /// A frame rate cannot be created from a float with no reasonable fraction
    /// approximation.
    #[error(
        "A frame rate cannot be created from a float with no reasonable fraction approximation."
    )]
    NoFracApproximation,

    /// A frame rate cannot be converted to an integer if it has a denominator
    /// over 1.
    #[error("A frame rate cannot be converted to an integer if it has a denominator over 1.")]
    NoIntRepresentation,
}

/// Given a stream of data `S` that produces data `src` times per second, this
/// can be used to get the index of the piece of data from `S` that should be
/// used for index `dest_idx` in a new stream of data `D`, where `D` produces
/// resampled data from `S` `dest` times per second.
///
/// If you won't be using the same `src` and `dest` values over and over (i.e.
/// you'll only make a few calls to [Self::resample]/[Self::duration] before
/// dropping it's likely more performant to use the freestanding
/// [resample]/[resample_back]/[resample_duration] functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resampler {
    src_per_dest_num: NonZeroU64,
    src_per_dest_den: NonZeroU64,
}

impl Resampler {
    /// Create a [Resampler].
    ///
    /// If you won't be using the same `src` and `dest` values over and over
    /// (i.e. you'll only make a few calls to [Self::resample] before dropping
    /// it's likely more performant to use the freestanding [resample] function.
    pub const fn new(src: Fps, dest: Fps) -> Self {
        let src_per_dest_num = src.num() as u64 * dest.den() as u64;
        let src_per_dest_den = src.den() as u64 * dest.num() as u64;

        let gcd = gcd!(src_per_dest_num, src_per_dest_den);
        debug_assert!(gcd >= 1);

        Self {
            // SAFETY: The numerator and denominator can both never be 0 and
            // dividing by a gcd will never result in 0.
            src_per_dest_num: unsafe { NonZeroU64::new_unchecked(src_per_dest_num / gcd) },
            src_per_dest_den: unsafe { NonZeroU64::new_unchecked(src_per_dest_den / gcd) },
        }
    }

    /// A [Resampler] for when no resampling is needed.
    #[inline]
    pub const fn no_op() -> Self {
        Self {
            src_per_dest_num: NonZeroU64::MIN,
            src_per_dest_den: NonZeroU64::MIN,
        }
    }

    /// This is functionally equivalent to a call to [resample] with the `src`
    /// and `dest` the arguments used with [Self::new].
    #[inline]
    pub const fn resample(&self, dest_idx: usize) -> usize {
        let num = self.src_per_dest_num.get();
        let den = self.src_per_dest_den.get();
        ((dest_idx as u64 * num) / den) as usize
    }

    /// This is functionally equivalent to a call to [resample_back] with the
    /// `src` and `dest` the arguments used with [Self::new].
    #[inline]
    pub const fn resample_back(&self, src_idx: usize) -> usize {
        let num = self.src_per_dest_num.get();
        let den = self.src_per_dest_den.get();
        (src_idx as u64 * den).div_ceil(num) as usize
    }

    /// This is functionally equivalent to a call to [resample_duration] with
    /// the `src` and `dest` the arguments used with [Self::new].
    ///
    /// This function *can* return 0.
    #[inline]
    pub const fn duration(&self, src_duration: usize) -> usize {
        let num = self.src_per_dest_num.get();
        let den = self.src_per_dest_den.get();
        ((src_duration as u64 * den) / num) as usize
    }

    /// Translates a `dest` index from another resampler into a `dest` index for
    /// this resampler.
    #[inline]
    pub const fn translate_old_dest_idx(
        &self,
        old_resampler: Resampler,
        old_dest_idx: usize,
    ) -> usize {
        let old_num = old_resampler.src_per_dest_num.get() as u128;
        let old_den = old_resampler.src_per_dest_den.get() as u128;
        let new_num = self.src_per_dest_num.get() as u128;
        let new_den = self.src_per_dest_den.get() as u128;
        ((old_dest_idx as u128 * old_num * new_den) / (old_den * new_num)) as usize
    }
}

/// Given a stream of data `S` that produces data `src` times per second, this
/// function returns the index of the piece of data from `S` that should be used
/// for index `dest_idx` in a new stream of data `D`, where `D` produces
/// resampled data from `S` `dest` times per second.
///
/// If you'll be calling this function with the same `src` and `dest` value over
/// and over, it's likely more performant to use the [Resampler] struct. Also
/// see [resample_back] and [resample_duration].
#[inline]
pub const fn resample(dest_idx: usize, src: Fps, dest: Fps) -> usize {
    let src_per_dest = (
        src.num() as u128 * dest.den() as u128,
        src.den() as u128 * dest.num() as u128,
    );
    let src_idx = (dest_idx as u128 * src_per_dest.0) / src_per_dest.1;
    src_idx as usize
}

/// The inverse of [resample]. This function returns the first valid index in
/// `D` given an index in `S`.
#[inline]
pub const fn resample_back(src_idx: usize, src: Fps, dest: Fps) -> usize {
    let src_per_dest = (
        src.num() as u128 * dest.den() as u128,
        src.den() as u128 * dest.num() as u128,
    );
    let dest_idx = (src_idx as u128 * src_per_dest.1).div_ceil(src_per_dest.0);
    dest_idx as usize
}

/// Given a resampled stream of data (see [resample]), this function determines
/// the duration of the `dest` stream given the duraton of the `src` stream.
///
/// If you'll be calling this function with the same `src` and `dest` value over
/// and over, it's likely more performant to use the [Resampler] struct.
///
/// This function *can* return 0.
#[inline]
pub const fn resample_duration(src_duration: usize, src: Fps, dest: Fps) -> usize {
    let dest_per_src = (
        dest.num() as u128 * src.den() as u128,
        dest.den() as u128 * src.num() as u128,
    );
    let dest_duration = (src_duration as u128 * dest_per_src.0) / dest_per_src.1;
    dest_duration as usize
}

const fn float_to_frac(x: f64, max_den: u32, tolerance: f64) -> Option<(u32, u32)> {
    // Based loosely on this algorithm: https://stackoverflow.com/a/96035

    debug_assert!(tolerance > 0.0 && tolerance <= 0.1 && tolerance.is_normal());
    debug_assert!(x >= tolerance && x <= u32::MAX as f64 && x.is_normal());
    debug_assert!(max_den >= 1);

    // Current value being expanded into a continued fraction
    let mut remainder = x;

    // Previous convergent: p_{-1}/q_{-1}
    let mut prev_num: u64 = 1;
    let mut prev_den: u64 = 0;

    // Current convergent: p_0/q_0
    let mut curr_num: u64 = remainder.floor() as u64;
    let mut curr_den: u64 = 1;

    let max_den = max_den as u64;

    loop {
        let integer_part = remainder.floor();
        let fractional_part = remainder - integer_part;

        // If exactly representable
        if fractional_part.abs() < tolerance {
            break;
        }

        remainder = 1.0 / fractional_part;
        let next_term = remainder.floor() as u64;

        // Compute next convergent:
        // p_k = a_k * p_{k-1} + p_{k-2}
        // q_k = a_k * q_{k-1} + q_{k-2}
        let next_num = next_term * curr_num + prev_num;
        let next_den = next_term * curr_den + prev_den;

        if next_den > max_den {
            break;
        }

        let approximation = next_num as f64 / next_den as f64;
        if (x - approximation).abs() <= tolerance {
            return Some((next_num as u32, next_den as u32));
        }

        prev_num = curr_num;
        prev_den = curr_den;
        curr_num = next_num;
        curr_den = next_den;
    }

    // Return best found if valid
    if curr_den <= max_den && curr_den != 0 {
        Some((curr_num as u32, curr_den as u32))
    } else {
        None
    }
}

#[inline(always)]
const fn is_near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_to_frac() {
        assert_eq!(
            Fps::from_float(33.3333).unwrap(),
            Fps::from_frac(100, 3).unwrap()
        );

        assert_eq!(
            Fps::from_float(10.5).unwrap(),
            Fps::from_frac(21, 2).unwrap()
        );

        assert_eq!(
            Fps::from_float(24.0).unwrap(),
            Fps::from_frac(24, 1).unwrap()
        );
    }

    #[test]
    fn ntsc_frame_rates() {
        assert_eq!(
            Fps::from_float(23.976).unwrap(),
            Fps::from_frac(24000, 1001).unwrap()
        );

        assert_eq!(
            Fps::from_float(29.97).unwrap(),
            Fps::from_frac(30000, 1001).unwrap()
        );

        assert_eq!(
            Fps::from_float(59.94).unwrap(),
            Fps::from_frac(60000, 1001).unwrap()
        );
    }

    /// The same as [super::resample] except it runs both [super::resample] and
    /// [super::Resampler::resample] and ensures they return the same result
    /// first. We'll use this to test both resample methods at the same time.
    fn resample_both(dest_idx: usize, src: Fps, dest: Fps) -> usize {
        let a = resample(dest_idx, src, dest);
        let b = Resampler::new(src, dest).resample(dest_idx);
        assert_eq!(a, b, "Resample methods not equivalent! ({a} != {b})");
        a
    }

    #[test]
    fn resample_always_starts_at_0() {
        // Even with crazy values dest_idx=0 should always return 0

        // even divide
        let slow = Fps::from_frac(1, 10_000).unwrap();
        let fast = Fps::from_frac(10_000, 1).unwrap();
        assert_eq!(resample_both(0, slow, fast), 0);
        assert_eq!(resample_both(0, fast, slow), 0);

        // uneven divide
        let slow = Fps::from_frac(3, 10_000).unwrap();
        let fast = Fps::from_frac(10_000, 3).unwrap();
        assert_eq!(resample_both(0, slow, fast), 0);
        assert_eq!(resample_both(0, fast, slow), 0);
    }

    #[test]
    fn resample_works_with_easy_fractions() {
        // 60/1 -> 30/1 (60Hz -> 30Hz)
        // Should skip every other frame in src
        let src = consts::FPS_60;
        let dest = consts::FPS_30;
        for dest_idx in 0..1000 {
            let expected_src_idx = dest_idx * 2;
            assert_eq!(resample_both(dest_idx, src, dest), expected_src_idx);
        }

        // 30/1 -> 60/1 (30Hz -> 60Hz)
        // Should duplicate every src frame once
        let src = consts::FPS_30;
        let dest = consts::FPS_60;
        for dest_idx in 0..1000 {
            let expected_src_idx = dest_idx / 2;
            assert_eq!(resample_both(dest_idx, src, dest), expected_src_idx);
        }
    }

    #[test]
    fn resample_works_with_complex_fractions() {
        // 7/3 -> 5/2 (2.333Hz -> 2.5Hz)
        let src = Fps::from_frac(7, 3).unwrap();
        let dest = Fps::from_frac(5, 2).unwrap();
        let sequence = [
            (0, 0),
            (1, 0),
            (2, 1),
            (3, 2),
            (4, 3),
            (5, 4),
            (6, 5),
            (7, 6),
            (8, 7),
            (9, 8),
            (10, 9),
            (11, 10),
            (12, 11),
            (13, 12),
            (14, 13),
            (15, 14),
            (16, 14),
            (17, 15),
            (18, 16),
            (19, 17),
            (20, 18),
            (21, 19),
            (22, 20),
            (23, 21),
            (24, 22),
            (25, 23),
            (26, 24),
            (27, 25),
            (28, 26),
            (29, 27),
        ];
        for (dest_idx, expected_src_idx) in sequence {
            assert_eq!(resample_both(dest_idx, src, dest), expected_src_idx);
        }

        // 11/4 -> 3/5 (2.75Hz -> 0.6Hz)
        let src = Fps::from_frac(11, 4).unwrap();
        let dest = Fps::from_frac(3, 5).unwrap();
        let sequence = [
            (0, 0),
            (1, 4),
            (2, 9),
            (3, 13),
            (4, 18),
            (5, 22),
            (6, 27),
            (7, 32),
            (8, 36),
            (9, 41),
            (10, 45),
            (11, 50),
            (12, 55),
            (13, 59),
            (14, 64),
            (15, 68),
            (16, 73),
            (17, 77),
            (18, 82),
            (19, 87),
            (20, 91),
            (21, 96),
            (22, 100),
            (23, 105),
            (24, 110),
        ];
        for (dest_idx, expected_src_idx) in sequence {
            assert_eq!(resample_both(dest_idx, src, dest), expected_src_idx);
        }

        // 5/7 -> 13/3 (0.714Hz -> 4.333Hz)
        let src = Fps::from_frac(5, 7).unwrap();
        let dest = Fps::from_frac(13, 3).unwrap();
        let sequence = [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (5, 0),
            (6, 0),
            (7, 1),
            (8, 1),
            (9, 1),
            (10, 1),
            (11, 1),
            (12, 1),
            (13, 2),
            (14, 2),
            (15, 2),
            (16, 2),
            (17, 2),
            (18, 2),
            (19, 3),
            (20, 3),
            (21, 3),
            (22, 3),
            (23, 3),
            (24, 3),
            (25, 4),
            (26, 4),
            (27, 4),
            (28, 4),
            (29, 4),
            (30, 4),
            (31, 5),
            (32, 5),
            (33, 5),
            (34, 5),
            (35, 5),
            (36, 5),
            (37, 6),
            (38, 6),
            (39, 6),
            (40, 6),
        ];
        for (dest_idx, expected_src_idx) in sequence {
            assert_eq!(resample_both(dest_idx, src, dest), expected_src_idx);
        }
    }

    #[test]
    fn resample_works_with_large_dest_idx() {
        // 7/3 -> 5/2 (2.333Hz -> 2.5Hz)
        let src = Fps::from_frac(7, 3).unwrap();
        let dest = Fps::from_frac(5, 2).unwrap();
        let sequence = [
            (1000000, 933333),
            (1000001, 933334),
            (1000002, 933335),
            (1000003, 933336),
            (1000004, 933337),
            (1000005, 933338),
            (1000006, 933338),
            (1000007, 933339),
            (1000008, 933340),
            (1000009, 933341),
        ];
        for (dest_idx, expected_src_idx) in sequence {
            assert_eq!(resample_both(dest_idx, src, dest), expected_src_idx);
        }

        // 9/8 -> 8/9 (1.125Hz -> 0.889Hz)
        let src = Fps::from_frac(9, 8).unwrap();
        let dest = Fps::from_frac(8, 9).unwrap();
        let sequence = [
            (1000000, 1265625),
            (1000001, 1265626),
            (1000002, 1265627),
            (1000003, 1265628),
            (1000004, 1265630),
            (1000005, 1265631),
            (1000006, 1265632),
            (1000007, 1265633),
            (1000008, 1265635),
            (1000009, 1265636),
        ];
        for (dest_idx, expected_src_idx) in sequence {
            assert_eq!(resample_both(dest_idx, src, dest), expected_src_idx);
        }
    }
}
