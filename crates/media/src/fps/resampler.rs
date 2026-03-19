//! Exports [Fps] resampling utilities.

use std::num::NonZeroU64;

use util::gcd::gcd_u64;

use super::Fps;

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

        let gcd = gcd_u64(src_per_dest_num, src_per_dest_den);
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::fps::consts;

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
