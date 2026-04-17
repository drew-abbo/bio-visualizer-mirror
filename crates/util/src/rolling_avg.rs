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

#[cfg(test)]
mod tests {
    use super::*;

    // Macro to generate identical tests for both types, avoiding duplication.
    macro_rules! rolling_avg_tests {
        ($mod_name:ident, $type:ident, $float:ident, $other_type:ident, $other_float:ident) => {
            mod $mod_name {
                use super::*;

                // --- new / default ---
                // Decision: always initializes to count=0, mean=0.0 (single path)

                #[test]
                fn new_starts_with_count_zero() {
                    let r = $type::new();
                    assert_eq!(r.count(), 0);
                }

                #[test]
                fn new_starts_with_mean_zero() {
                    let r = $type::new();
                    assert_eq!(*r, 0.0 as $float);
                }

                #[test]
                fn default_equals_new() {
                    let a = $type::new();
                    let b = $type::default();
                    assert_eq!(a.count(), b.count());
                    assert_eq!(*a, *b);
                }

                // --- get ---
                // Decision: count == 0 => None | count > 0 => Some

                #[test]
                fn get_returns_none_when_empty() {
                    let r = $type::new();
                    assert_eq!(r.get(), None);
                }

                #[test]
                fn get_returns_some_after_one_add() {
                    let mut r = $type::new();
                    r.add(4.0);
                    assert_eq!(r.get(), Some(4.0 as $float));
                }

                #[test]
                fn get_returns_some_after_multiple_adds() {
                    let mut r = $type::new();
                    r.add(2.0);
                    r.add(4.0);
                    assert!(r.get().is_some());
                }

                // --- get_or_0 ---
                // Decision: count == 0 => 0.0 | count > 0 => mean
                // (implemented as just returning mean, so 0.0 is the zero-count case)

                #[test]
                fn get_or_0_returns_zero_when_empty() {
                    let r = $type::new();
                    assert_eq!(r.get_or_0(), 0.0 as $float);
                }

                #[test]
                fn get_or_0_returns_mean_when_nonempty() {
                    let mut r = $type::new();
                    r.add(10.0);
                    assert_eq!(r.get_or_0(), 10.0 as $float);
                }

                // --- add ---
                // Decision: count transitions from 0→1 (first add) and N→N+1 (subsequent adds)
                // Also verifies the rolling average formula is correct

                #[test]
                fn add_first_value_sets_mean_to_that_value() {
                    let mut r = $type::new();
                    let result = r.add(7.0);
                    assert_eq!(result, 7.0 as $float);
                    assert_eq!(r.count(), 1);
                }

                #[test]
                fn add_two_equal_values_mean_stays_same() {
                    let mut r = $type::new();
                    r.add(5.0);
                    let result = r.add(5.0);
                    assert!((result - 5.0 as $float).abs() < 1e-6 as $float);
                    assert_eq!(r.count(), 2);
                }

                #[test]
                fn add_returns_correct_rolling_average() {
                    let mut r = $type::new();
                    r.add(0.0);
                    r.add(10.0);
                    // average of 0 and 10 is 5
                    let result = r.add(5.0);
                    // average of 0, 10, 5 is 5
                    assert!((result - 5.0 as $float).abs() < 1e-5 as $float);
                    assert_eq!(r.count(), 3);
                }

                #[test]
                fn add_increments_count_each_time() {
                    let mut r = $type::new();
                    for i in 1..=5 {
                        r.add(i as $float);
                        assert_eq!(r.count(), i);
                    }
                }

                // --- extend ---
                // Decision 1: empty iterator => loop body never executes
                // Decision 2: nonempty iterator => loop body executes for each item

                #[test]
                fn extend_with_empty_iter_changes_nothing() {
                    let mut r = $type::new();
                    r.extend(std::iter::empty());
                    assert_eq!(r.count(), 0);
                    assert_eq!(r.get(), None);
                }

                #[test]
                fn extend_with_single_item() {
                    let mut r = $type::new();
                    r.extend([3.0 as $float]);
                    assert_eq!(r.count(), 1);
                    assert_eq!(r.get(), Some(3.0 as $float));
                }

                #[test]
                fn extend_with_multiple_items() {
                    let mut r = $type::new();
                    r.extend([2.0 as $float, 4.0, 6.0]);
                    assert_eq!(r.count(), 3);
                    // mean of 2, 4, 6 = 4
                    assert!((r.get().unwrap() - 4.0 as $float).abs() < 1e-5 as $float);
                }

                #[test]
                fn extend_on_nonempty_continues_rolling_average() {
                    let mut r = $type::new();
                    r.add(10.0);
                    r.extend([20.0 as $float, 30.0]);
                    assert_eq!(r.count(), 3);
                    // mean of 10, 20, 30 = 20
                    assert!((r.get().unwrap() - 20.0 as $float).abs() < 1e-5 as $float);
                }

                // --- Deref ---
                // Decision: single path, returns &mean

                #[test]
                fn deref_returns_mean_when_empty() {
                    let r = $type::new();
                    assert_eq!(*r, 0.0 as $float);
                }

                #[test]
                fn deref_returns_mean_when_nonempty() {
                    let mut r = $type::new();
                    r.add(8.0);
                    assert_eq!(*r, 8.0 as $float);
                }

                // --- AsRef ---
                // Decision: single path, returns &mean

                #[test]
                fn as_ref_returns_mean_reference() {
                    let mut r = $type::new();
                    r.add(6.0);
                    let mean_ref: &$float = r.as_ref();
                    assert_eq!(*mean_ref, 6.0 as $float);
                }

                // --- From<$type> for $float (same type) ---
                // Decision: single path, extracts mean

                #[test]
                fn into_same_float_empty() {
                    let r = $type::new();
                    let val: $float = r.into();
                    assert_eq!(val, 0.0 as $float);
                }

                #[test]
                fn into_same_float_nonempty() {
                    let mut r = $type::new();
                    r.add(9.0);
                    let val: $float = r.into();
                    assert_eq!(val, 9.0 as $float);
                }

                // --- From<$type> for $other_float (cross-type cast) ---
                // Decision: single path, casts mean

                #[test]
                fn into_other_float_empty() {
                    let r = $type::new();
                    let val: $other_float = r.into();
                    assert_eq!(val, 0.0 as $other_float);
                }

                #[test]
                fn into_other_float_nonempty() {
                    let mut r = $type::new();
                    r.add(5.0);
                    let val: $other_float = r.into();
                    assert!((val - 5.0 as $other_float).abs() < 1e-5 as $other_float);
                }

                // --- From<$type> for $other_type (cross rolling-avg type) ---
                // Decision: single path, converts mean and preserves count

                #[test]
                fn into_other_rolling_avg_empty() {
                    let r = $type::new();
                    let other: $other_type = r.into();
                    assert_eq!(other.count(), 0);
                    assert_eq!(other.get(), None);
                }

                #[test]
                fn into_other_rolling_avg_nonempty_preserves_count_and_mean() {
                    let mut r = $type::new();
                    r.add(3.0);
                    r.add(7.0);
                    let count = r.count();
                    let other: $other_type = r.into();
                    assert_eq!(other.count(), count);
                    assert!(
                        (other.get().unwrap() - 5.0 as $other_float).abs() < 1e-5 as $other_float
                    );
                }

                // --- FromIterator ---
                // Decision 1: empty iterator => count 0, get None
                // Decision 2: nonempty iterator => computes correct average

                #[test]
                fn from_iterator_empty() {
                    let r: $type = std::iter::empty::<$float>().collect();
                    assert_eq!(r.count(), 0);
                    assert_eq!(r.get(), None);
                }

                #[test]
                fn from_iterator_single_value() {
                    let r: $type = [42.0 as $float].into_iter().collect();
                    assert_eq!(r.count(), 1);
                    assert_eq!(r.get(), Some(42.0 as $float));
                }

                #[test]
                fn from_iterator_multiple_values() {
                    let r: $type = [1.0 as $float, 2.0, 3.0, 4.0].into_iter().collect();
                    assert_eq!(r.count(), 4);
                    // mean of 1,2,3,4 = 2.5
                    assert!((r.get().unwrap() - 2.5 as $float).abs() < 1e-5 as $float);
                }
            }
        };
    }

    rolling_avg_tests!(f64_tests, RollingAvgF64, f64, RollingAvgF32, f32);
    rolling_avg_tests!(f32_tests, RollingAvgF32, f32, RollingAvgF64, f64);
}
