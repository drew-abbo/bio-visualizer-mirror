//! Functions for finding the greatest common denominator between 2 numbers.

// Implementation
macro_rules! impl_gcd {
    ($name:ident, $type:ident) => {
        /// Returns the greatest common denominator between `a` and `b`.
        ///
        /// If either `a` or `b` are non-positive, the result is undefined.
        pub const fn $name(mut a: $type, mut b: $type) -> $type {
            while b != 0 {
                (b, a) = (a % b, b)
            }
            a
        }
    };
}

impl_gcd!(gcd_i8, i8);
impl_gcd!(gcd_i16, i16);
impl_gcd!(gcd_i32, i32);
impl_gcd!(gcd_i64, i64);
impl_gcd!(gcd_i128, i128);
impl_gcd!(gcd_u8, u8);
impl_gcd!(gcd_u16, u16);
impl_gcd!(gcd_u32, u32);
impl_gcd!(gcd_u64, u64);
impl_gcd!(gcd_u128, u128);

#[cfg(test)]
mod tests {
    use super::*;

    // --- loop execution ---

    #[test]
    fn gcd_runs_loop_once() {
        // 10 % 5 == 0 → one iteration
        assert_eq!(gcd_i32(10, 5), 5);
    }

    #[test]
    fn gcd_runs_loop_multiple_times() {
        // Classic Euclidean example
        assert_eq!(gcd_i32(48, 18), 6);
    }

    // --- equal numbers ---

    #[test]
    fn gcd_of_equal_numbers_is_number() {
        assert_eq!(gcd_i32(7, 7), 7);
        assert_eq!(gcd_u32(7, 7), 7);
    }

    // --- coprime numbers ---

    #[test]
    fn gcd_of_coprime_numbers_is_one() {
        assert_eq!(gcd_i32(17, 13), 1);
        assert_eq!(gcd_u32(17, 13), 1);
    }

    // --- edge cases ---

    #[test]
    fn gcd_of_one_and_number() {
        assert_eq!(gcd_i32(1, 999), 1);
        assert_eq!(gcd_u32(1, 999), 1);
    }

    #[test]
    fn gcd_large_numbers() {
        assert_eq!(gcd_u64(1_000_000_000, 2), 2);
    }
}
