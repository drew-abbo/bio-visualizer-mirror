//! Exports the [ConnN] type.

use std::array;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Ref-counted heap data intended to "connected to" by up to `N` threads.
///
/// The benefit of this over [std::sync::Arc] is that the ref-count is accessed
/// with [Ordering::SeqCst] (instead of the [Ordering::Relaxed] used in
/// [std::sync::Arc::strong_count]'s implementation). Also, since `T` must be
/// [Sized], we're able to move the ref-count to be after the contained data
/// so the inner pointer points directly to the data without needing to be
/// offset (admittedly a micro-optimization).
#[derive(Debug)]
pub struct ConnN<T>(*const ConnInner<T>);

impl<T> ConnN<T> {
    /// Get `N` connection handles where `N` is known at compile-time.
    #[inline(always)]
    pub fn new<const N: usize>(data: T) -> [Self; N] {
        let ptr = ConnInner::alloc(data, N);
        array::from_fn(|_| Self(ptr))
    }

    /// Get access to the inner `T` data.
    #[inline(always)]
    pub const fn get(&self) -> &T {
        &self.inner().data
    }

    /// If this is the only handle to the shared data (see [Self::count]).
    #[inline(always)]
    pub fn is_only_handle(&self) -> bool {
        self.count() == 1
    }

    /// The number of handles connected to this data.
    ///
    /// `0` will never be returned (see [Self::count_non_zero]).
    #[inline(always)]
    pub fn count(&self) -> usize {
        self.count_non_zero().get()
    }

    /// The number of handles connected to this data.
    #[inline]
    pub fn count_non_zero(&self) -> NonZeroUsize {
        // SAFETY: If this `self` instance exists it means the count is 1+.
        unsafe { NonZeroUsize::new_unchecked(self.inner().count.load(Ordering::SeqCst)) }
    }

    #[inline(always)]
    const fn inner(&self) -> &ConnInner<T> {
        // SAFETY: This pointer is valid while one of these objects exists.
        unsafe { &*self.0 }
    }
}

impl<T> Drop for ConnN<T> {
    #[inline]
    fn drop(&mut self) {
        let new_count = self.inner().count.fetch_sub(1, Ordering::SeqCst) - 1;

        // If nobody else is connected we need to free the shared memory.
        if new_count == 0 {
            // SAFETY: Since nobody else is connected, that means we're the only
            // ones with access to this pointer, so it's safe to be freed.
            unsafe { ConnInner::drop_and_free(self.0) };
        }
    }
}

// SAFETY: This type is `Send` and `Sync`. We have to manually implement this
// because it stores a pointer internally.
unsafe impl<T: Send> Send for ConnN<T> {}
unsafe impl<T: Sync> Sync for ConnN<T> {}

impl<T> Deref for ConnN<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> AsRef<T> for ConnN<T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self.get()
    }
}

#[derive(Debug)]
struct ConnInner<T> {
    data: T,
    count: AtomicUsize,
}

impl<T> ConnInner<T> {
    /// Allocate a new [ConnInner] on the heap and return a pointer to it.
    #[inline(always)]
    pub fn alloc(data: T, n: usize) -> *const Self {
        if n > 0 {
            Box::leak(Box::new(ConnInner {
                data,
                count: AtomicUsize::new(n),
            }))
        } else {
            // If `N` is zero it means nothing is actually being constructed, so
            // we can just return a dummy pointer.
            ptr::dangling()
        }
    }

    /// Frees the pointer returned by [Self::alloc].
    ///
    /// # Safety
    ///
    /// The memory pointed to by `this` should not be referenced after this is
    /// called.
    #[inline]
    pub unsafe fn drop_and_free(this: *const Self) {
        // SAFETY:
        // 1. It's on the caller to ensure this pointer was created in with
        //    `Self::alloc`. If it was it was allocated with `Box::new` and then
        //    leaked (so it's safe to call `Box::from_raw`).
        // 2. It's on the caller to ensure the pointer is never accessed again
        //    after this.
        drop(unsafe { Box::from_raw(this as *mut ConnInner<T>) });
    }
}
