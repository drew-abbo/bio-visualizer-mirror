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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cast_slice_basic() {
        // No decisions — just ensure it works for a valid case
        let data: [u8; 4] = [1, 2, 3, 4];

        let result = unsafe { cast_slice::<u8, u16>(&data) };

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_cast_slice_mut_basic() {
        // No decisions — just ensure mutation works through cast
        let mut data: [u8; 4] = [1, 2, 3, 4];

        let result = unsafe { cast_slice_mut::<u8, u16>(&mut data) };

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_cast_array_valid_sizes() {
        // D1 = true (sizes match)
        let data: [u8; 4] = [1, 2, 3, 4];

        let result: [u16; 2] = unsafe { cast_array(data) };

        assert_eq!(result.len(), 2);
    }

    // The false branch cannot be tested for this
    // This is because the only decision in cast_array
    // a compile time assertion and the false branch cannot
    // be executed at runtime
}
