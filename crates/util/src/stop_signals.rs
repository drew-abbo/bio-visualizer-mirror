//! Tools for handling stop signals (e.g. `SIGINT`). See [passthrough] and
//! [polling].
//!
//! Note that enabling the handlers in either sub-module ([passthrough] and
//! [polling]) will disable the default stop-signal handler.

pub mod passthrough;
pub mod polling;

use std::mem::MaybeUninit;

use crate::cast_slice;

const THREAD_EXPECT_MSG: &str = "The other thread shouldn't panic.";

/// Like [std::array::from_fn] except it handles the fact that `f` can fail.
fn try_array_from_fn<const N: usize, T, E, F>(mut f: F) -> Result<[T; N], E>
where
    F: FnMut(usize) -> Result<T, E>,
{
    let mut ret: [MaybeUninit<T>; N] = [const { MaybeUninit::uninit() }; N];

    for i in 0..N {
        match f(i) {
            Ok(item) => ret[i] = MaybeUninit::new(item),
            Err(e) => {
                for item in ret.iter_mut().take(i) {
                    // SAFETY: We're only dropping items we've initialized.
                    unsafe { item.assume_init_drop() };
                }
                return Err(e);
            }
        }
    }

    // SAFETY: We've filled the entire array by this point so it's safe to cast
    // from an array of `MaybeUninit<T>` to an array of `T`.
    Ok(unsafe { cast_slice::cast_array(ret) })
}
