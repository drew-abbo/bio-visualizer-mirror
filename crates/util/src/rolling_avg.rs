//! Contains [RollingAvgF64] and [RollingAvgF32]. Types that store the rolling
//! averages of values added to them.

use std::ops::Deref;

// We're using a macro so we don't have to do all of this for both float types.
macro_rules! def_and_impl {
    (
        $primary_type:ident, $primary_float_type:ident,
        $secondary_type:ident, $secondary_float_type:ident
    ) => {
        /// A type that stores and continually computes a rolling average in
        /// `O(1)` time and `O(1)` space.
        #[derive(Debug, Clone, Copy)]
        pub struct $primary_type {
            mean: $primary_float_type,
            count: usize,
        }

        impl $primary_type {
            /// Create a new rolling average object. Unless you *need* the
            /// `const`-ness of this function, use [Self::default].
            #[inline]
            pub const fn new() -> Self {
                Self {
                    mean: 0.0,
                    count: 0,
                }
            }

            /// Adds `val` to the rolling average and returns the new average.
            #[inline]
            pub const fn add(&mut self, val: $primary_float_type) -> $primary_float_type {
                self.count += 1;
                self.mean += (val - self.mean) / self.count as $primary_float_type;
                self.mean
            }

            /// The number of values added to this rolling average.
            #[inline(always)]
            pub const fn count(&self) -> usize {
                self.count
            }

            /// Get the average or [None] if no values have been added.
            ///
            /// Also see [Self::get_or_0].
            #[inline]
            pub const fn get(&self) -> Option<$primary_float_type> {
                if self.count == 0 {
                    return None;
                }
                Some(self.mean)
            }

            /// Get the average or `0.0` if no values have been added. You can
            /// also [dereference](Self::deref) this object to get this value
            /// (`*` operator).
            ///
            /// Also see [Self::get].
            #[inline(always)]
            pub const fn get_or_0(&self) -> $primary_float_type {
                self.mean
            }

            /// Adds all values from `iter` to the rolling average. See
            /// [Self::add].
            #[inline]
            pub fn extend<I: IntoIterator<Item = $primary_float_type>>(&mut self, iter: I) {
                for val in iter {
                    self.add(val);
                }
            }
        }

        impl Default for $primary_type {
            #[inline(always)]
            fn default() -> Self {
                Self::new()
            }
        }

        impl Deref for $primary_type {
            type Target = $primary_float_type;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                &self.mean
            }
        }

        impl AsRef<$primary_float_type> for $primary_type {
            #[inline(always)]
            fn as_ref(&self) -> &$primary_float_type {
                &self.mean
            }
        }

        impl From<$primary_type> for $primary_float_type {
            #[inline(always)]
            fn from(rolling_avg: $primary_type) -> Self {
                rolling_avg.mean
            }
        }

        impl From<$primary_type> for $secondary_float_type {
            #[inline(always)]
            fn from(rolling_avg: $primary_type) -> Self {
                rolling_avg.mean as _
            }
        }

        impl From<$primary_type> for $secondary_type {
            #[inline]
            fn from(rolling_avg: $primary_type) -> Self {
                let $primary_type { mean, count } = rolling_avg;
                let mean = mean as _;
                Self { mean, count }
            }
        }

        impl FromIterator<$primary_float_type> for $primary_type {
            #[inline]
            fn from_iter<I: IntoIterator<Item = $primary_float_type>>(iter: I) -> Self {
                let mut rolling_avg = Self::default();
                rolling_avg.extend(iter);
                rolling_avg
            }
        }
    };
}

def_and_impl!(RollingAvgF64, f64, RollingAvgF32, f32);
def_and_impl!(RollingAvgF32, f32, RollingAvgF64, f64);
