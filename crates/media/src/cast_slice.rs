//! Contains the [cast_slice] and [cast_slice_mut] functions for casting slices
//! from one type to another. These are incredibly unsafe functions.

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
