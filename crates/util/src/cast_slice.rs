//! Contains functions for casting slices and arrays from one type to another.
//! These are incredibly unsafe functions.

use std::mem::{self, MaybeUninit};
use std::ptr;
use std::slice;

/// Cast one kind of slice to another.
///
/// # Safety
///
/// It's on the caller to ensure this does not invoke undefined behavior.
pub const unsafe fn cast_slice<Src, Dest>(slice: &[Src]) -> &[Dest] {
    // SAFETY: We're casting one slice type to another, taking into account the
    // fact that `Src` may not be be the same size as `Dest`. It's on the caller
    // to ensure this cast is ok.
    unsafe {
        slice::from_raw_parts(
            slice.as_ptr() as *const Dest,
            size_of_val(slice) / size_of::<Dest>(),
        )
    }
}

/// Cast one kind of *mutable* slice to another.
///
/// # Safety
///
/// It's on the caller to ensure this does not invoke undefined behavior.
pub const unsafe fn cast_slice_mut<Src, Dest>(slice: &mut [Src]) -> &mut [Dest] {
    // SAFETY: We're casting one slice type to another, taking into account the
    // fact that `Src` may not be be the same size as `Dest`. It's on the caller
    // to ensure this cast is ok.
    unsafe {
        slice::from_raw_parts_mut(
            slice.as_mut_ptr() as *mut Dest,
            size_of_val(slice) / size_of::<Dest>(),
        )
    }
}

/// Cast one kind of array to another.
///
/// # Safety
///
/// It's on the caller to ensure this does not invoke undefined behavior.
pub const unsafe fn cast_array<const N: usize, const M: usize, Src, Dest>(
    mut array: [Src; N],
) -> [Dest; M] {
    const {
        assert!(
            M * size_of::<Dest>() == N * size_of::<Src>(),
            "Resulting array must have the same size as the input array."
        );
    }

    let mut ret: MaybeUninit<[Dest; M]> = MaybeUninit::uninit();

    // SAFETY: We're copying all of the data from one array to the other, taking
    // into account the fact that `Src` may not be be the same size as `Dest`.
    // It's on the caller to ensure this is ok.
    unsafe {
        ptr::copy_nonoverlapping(
            array.as_mut_ptr() as *mut u8,
            ret.as_mut_ptr() as *mut u8,
            N * size_of::<Src>(),
        )
    };

    mem::forget(array);

    // SAFETY: We've copied all of the data from `array` to `ret` (so it's
    // initialized). We also ensured no destructor will run for the original.
    unsafe { ret.assume_init() }
}
