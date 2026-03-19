//! Functions for finding the greatest common denominator between 2 numbers.

// Implementation
macro_rules! impl_gcd {
    ($name:ident, $type:ident) => {
        /// Returns the greatest common denominator between `a` and `b`.
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
