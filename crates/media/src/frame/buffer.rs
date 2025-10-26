//! This module declares the [Frame] type and the [FrameBuffer] trait for what
//! it holds internally.
//!
//! # Safety
//!
//! Because frame buffers store such large amounts of data and are interacted
//! with mainly in performance critical (expensive) sections of code, this
//! module uses a lot of `unsafe` code (that's also why there's so many huge
//! comments explaining things). If you're going to modify this module (or its
//! `preulude` sub-module), be *extremely* careful.

mod prelude;

use std::any::Any;
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::mem::{self, MaybeUninit};
use std::ops::{Index, IndexMut};
use std::ptr;
use std::slice::{self, Chunks, ChunksMut};

use thiserror::Error;

pub use crate::pixel;
pub use prelude::*;

/// A buffer of data representing all of the [Pixel]s in a frame, along with the
/// frame's dimensions, meant to be stored in a [Frame] object.
///
/// For performance reasons and to ensure that returned dimensions/slices are
/// always the same, [Frame]s (which store [FrameBuffer] trait objects) will
/// only ever call the [FrameBuffer::dimensions] and [FrameBuffer::pixels_mut]
/// functions once, caching the results.
///
/// # Contract
///
/// The following rules should be upheld when implementing:
///
/// - The result of `b.dimensions().area() == b.pixels_mut().len()` is always
///   true for any `b: impl FrameBuffer`.
/// - All calls to [FrameBuffer::dimensions] with the same `self` parameter will
///   always return the same value.
/// - All calls to [FrameBuffer::pixels_mut] with the same `self` parameter will
///   always return a reference to the same buffer with the same length (this
///   does not mean the contents of the buffer can't change).
pub trait FrameBuffer: Any + Send + Sync + 'static {
    /// The dimensions of this frame.
    fn dimensions(&self) -> Dimensions;

    /// A *mutable* reference to the underlying pixels.
    fn pixels_mut(&mut self) -> &mut [Pixel];
}

/// The data that makes up a frame, storing a [FrameBuffer] trait object
/// internally.
///
/// For performance reasons and to ensure that returned dimensions/slices are
/// always the same, [Frame]s (which store [FrameBuffer] trait objects) will
/// only ever call the [FrameBuffer::dimensions] and [FrameBuffer::pixels_mut]
/// functions once, caching the results.
pub struct Frame {
    /// This is the cached value of calling [FrameBuffer::pixels_mut] on
    /// [Self::buffer].
    ///
    /// We're caching this value since getting it from [Self::buffer] every time
    /// requires pointer indirection and a vtable lookup. The [FrameBuffer]
    /// trait's documentation is clear that the return value of
    /// [FrameBuffer::pixels_mut] should be the same every call, so this is ok.
    ///
    /// # Safety
    ///
    /// See the two big comments in [Self::from_parts]. We are being very
    /// careful with this sucker.
    pixels: *mut [Pixel],

    /// This is the cached value of calling [FrameBuffer::dimensions] on
    /// [Self::buffer].
    ///
    /// We're caching this value since getting it from [Self::buffer] every time
    /// requires pointer indirection and a vtable lookup. The [FrameBuffer]
    /// trait's documentation is clear that the return value of
    /// [FrameBuffer::dimensions] should be the same every call, so this is ok.
    dimensions: Dimensions,

    uid: Uid,

    /// We never really use this field (the only time we use it is to return it
    /// from the [Self::to_internal] function, which consumes `self`), but we
    /// need to hold onto it because we're referencing it through the
    /// [Self::pixels] field in a way that the Rust compiler just can't 100% see
    /// (look at [Self::from_parts] to see that, we're doing some evil work
    /// there). TLDR: We need to hold onto this so that it doesn't drop because
    /// we're referencing it through the [Self::pixels] field.
    ///
    /// If this field is ever *not* unused, there is likely a problem with the
    /// code.
    buffer: Box<dyn FrameBuffer>,
}

impl Frame {
    /// Create a frame from any type that implements [FrameBuffer].
    ///
    /// If `buffer.dimensions() != buffer.pixels_mut().len()`, this function
    /// will panic.
    pub fn from_buffer<B: FrameBuffer>(buffer: B) -> Self {
        Self::from_internal(Box::new(buffer))
    }

    /// Create a frame from the [Box]ed [FrameBuffer] trait object that will be
    /// stored internally.
    ///
    /// If `buffer.dimensions() != buffer.pixels_mut().len()`, this function
    /// will panic.
    pub fn from_internal(buffer: Box<dyn FrameBuffer>) -> Self {
        // CONTRACT: The provided UID is unique because it's brand new.
        Self::from_parts(buffer, Uid::new())
    }

    /// Creates a new frame with all pixels set to `fill_pixel`.
    ///
    /// Also see [Self::from_fill_with] and [Self::from_fill_with_coords].
    pub fn from_fill(dimensions: Dimensions, fill_pixel: Pixel) -> Self {
        Self::from_buffer(BasicFrame {
            pixels: vec![fill_pixel; dimensions.area()].into_boxed_slice(),
            dimensions,
        })
    }

    /// Creates a new frame with all pixels set to the result of the `f`
    /// callback.
    ///
    /// Also see [Self::from_fill] and [Self::from_fill_with_coords].
    pub fn from_fill_with<F>(dimensions: Dimensions, mut f: F) -> Self
    where
        F: FnMut() -> Pixel,
    {
        // SAFETY: We're calling `fill_with` which will hit every pixel. All
        // memory will be initialized.
        unsafe {
            Self::from_uninitialized_pixels(dimensions, |new_pixels| {
                new_pixels.fill_with(|| MaybeUninit::new(f()))
            })
        }
    }

    /// Creates a new frame with all pixels set to the result of the `f`
    /// callback, where `f` takes the current pixel's row and column as an
    /// argument.
    ///
    /// Also see [Self::from_fill] and [Self::from_fill_with].
    pub fn from_fill_with_coords<F>(dimensions: Dimensions, mut f: F) -> Self
    where
        F: FnMut(usize, usize) -> Pixel,
    {
        let mut index = 0;
        Self::from_fill_with(dimensions, || {
            let row = index / dimensions.width();
            let col = index % dimensions.width();
            index += 1;
            f(row, col)
        })
    }

    /// Creates a new frame with all pixels being set to [Pixel::BLACK].
    pub fn new(dimensions: Dimensions) -> Self {
        Self::from_fill(dimensions, Pixel::BLACK)
    }

    /// Tries to create a new frame, returning an error if
    /// `pixels.len() != dimensions.area()`.
    pub fn from_pixels(
        pixels: Box<[Pixel]>,
        dimensions: Dimensions,
    ) -> Result<Self, TryFromSliceError> {
        if pixels.len() != dimensions.area() {
            Err(TryFromSliceError::LenError)
        } else {
            // SAFETY: We just checked that the data is long enough.
            Ok(unsafe { Self::from_pixels_unchecked(pixels, dimensions) })
        }
    }

    /// Tries to create a frame with a raw data slice, returning an error if
    /// `data.len() != dimensions.area() * size_of::<Pixel>` or if the `data`
    /// slice is not aligned in a way that would allow it to be reinterpreted as
    /// a slice of [Pixel]s.
    pub fn from_raw_data(
        data: Box<[u8]>,
        dimensions: Dimensions,
    ) -> Result<Self, TryFromSliceError> {
        if data.len() != dimensions.area() * size_of::<Pixel>() {
            Err(TryFromSliceError::LenError)
        } else if data.as_ptr().align_offset(mem::align_of::<Pixel>()) != 0 {
            Err(TryFromSliceError::AlignmentError)
        } else {
            // SAFETY: We just checked that the data is long enough and that
            // it's aligned properly.
            Ok(unsafe { Self::from_raw_data_unchecked(data, dimensions) })
        }
    }

    /// The dimensions of this frame.
    pub const fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    /// A reference to the underlying pixels.
    pub const fn pixels(&self) -> &[Pixel] {
        // SAFETY: While the caller holds this reference, there will be no way
        // for `pixels` to be mutated and the only way for `pixels` to be read
        // is through the returned reference (or one of its derivatives). No
        // `&mut self` or `self` methods will be able to be called until the
        // reference is dropped (so the compiler will not allow `pixels` to be
        // mutated until there are no more references to it).
        unsafe { &*self.pixels }
    }

    /// A *mutable* reference to the underlying pixels.
    pub const fn pixels_mut(&mut self) -> &mut [Pixel] {
        // SAFETY: While the caller holds this reference, the only way for
        // `pixels` to be mutated is through the returned reference. No `&self`,
        // `&mut self`, or `self` methods will be able to be called until the
        // reference is dropped (meaning the reference we return is the only
        // window into the `pixels` data.
        unsafe { &mut *self.pixels }
    }

    /// A reference to the raw data in the underlying buffer.
    pub const fn raw_data(&self) -> &[u8] {
        // SAFETY: We're just casting one slice type to another. This is fine
        // since both `Pixel` and `u8` are just plain old data.
        unsafe { cast_slice(self.pixels()) }
    }

    /// A *mutable* reference to the raw data in the underlying buffer.
    pub const fn raw_data_mut(&mut self) -> &mut [u8] {
        // SAFETY: We're just casting one slice type to another. This is fine
        // since both `Pixel` and `u8` are just plain old data.
        unsafe { cast_slice_mut(self.pixels_mut()) }
    }

    /// An iterator over rows of pixels in the frame.
    pub fn pixel_rows(&self) -> Chunks<'_, Pixel> {
        self.pixels().chunks(self.dimensions.width())
    }

    /// An iterator over *mutable* rows of pixels in the frame.
    pub fn pixel_rows_mut(&mut self) -> ChunksMut<'_, Pixel> {
        let width = self.dimensions.width();
        self.pixels_mut().chunks_mut(width)
    }

    /// An iterator over rows of the raw data in the underlying buffer.
    pub fn raw_data_rows(&self) -> Chunks<'_, u8> {
        self.raw_data()
            .chunks(self.dimensions.width() * size_of::<Pixel>())
    }

    /// An iterator over *mutable* rows of the raw data in the underlying
    /// buffer.
    pub fn raw_data_rows_mut(&mut self) -> ChunksMut<'_, u8> {
        let width = self.dimensions.width();
        self.raw_data_mut().chunks_mut(width * size_of::<Pixel>())
    }

    /// Sets all pixels in the frame to be `fill_pixel`.
    ///
    /// Also see [Self::fill_with], [Self::fill_with_coords], and
    /// [Self::fill_from_frame].
    pub fn fill(&mut self, fill_pixel: Pixel) {
        self.pixels_mut().fill(fill_pixel);
    }

    /// Sets all pixels in the frame to be the result of the callback `f`.
    ///
    /// Also see [Self::fill], [Self::fill_with_coords], and
    /// [Self::fill_from_frame].
    pub fn fill_with<F: FnMut() -> Pixel>(&mut self, f: F) {
        self.pixels_mut().fill_with(f);
    }

    /// Sets all pixels in the frame to be the result of the callback `f`, where
    /// `f` is passed the row and column of the pixel that is being filled.
    ///
    /// Also see [Self::fill], [Self::fill_with], and [Self::fill_from_frame].
    pub fn fill_with_coords<F: FnMut(usize, usize) -> Pixel>(&mut self, mut f: F) {
        let width = self.dimensions().width();
        for (index, pixel) in self.pixels_mut().iter_mut().enumerate() {
            *pixel = f(index / width, index % width);
        }
    }

    /// Copies all of the pixels from `src_frame` to `self`, returning an error
    /// if `src_frame` has different dimensions.
    ///
    /// Also see [Self::fill], [Self::fill_with], and [Self::fill_with_coords].
    pub fn fill_from_frame(&mut self, src_frame: &Frame) -> Result<(), DifferentDimensionsError> {
        if self.dimensions() != src_frame.dimensions() {
            Err(DifferentDimensionsError {
                expected: self.dimensions(),
                actual: src_frame.dimensions(),
            })
        } else {
            self.pixels_mut().copy_from_slice(src_frame.pixels());
            Ok(())
        }
    }

    /// An ID that uniquely identifies this frame against all others. For the
    /// duration of a [Frame]'s lifetime, no other frames will have an equal
    /// [Uid].
    ///
    /// Equality and hashing is based on this value.
    pub const fn uid(&self) -> Uid {
        self.uid
    }

    /// Rescale this frame to have new [Dimensions] using any [RescaleMethod].
    ///
    /// Rescaling on the CPU is computationally expensive. See [RescaleMethod]
    /// for a comparison of each algorithm's performance. It's also important to
    /// note that this function may not be implemented in the most
    /// performance-optimal way.
    ///
    /// Also see [Self::rescale_nearest_neighbor], [Self::rescale_bilinear], and
    /// [Self::rescale_bicubic].
    ///
    /// This will return a new [Frame], similar to how [Self::clone] does. The
    /// new [Frame] may have a different internal type (a different internal
    /// [FrameBuffer] implementation).
    pub fn rescale(&self, new_dimensions: Dimensions, rescale_method: RescaleMethod) -> Self {
        match rescale_method {
            RescaleMethod::NearestNeighbor => self.rescale_nearest_neighbor(new_dimensions),
            RescaleMethod::Bilinear => self.rescale_bilinear(new_dimensions),
            RescaleMethod::Bicubic => self.rescale_bicubic(new_dimensions),
        }
    }

    /// Rescale this [Frame] to have new [Dimensions] using the
    /// [nearest neighbor](RescaleMethod::NearestNeighbor) rescaling algorithm.
    ///
    /// Rescaling on the CPU is computationally expensive. See [RescaleMethod]
    /// for a comparison of each algorithm's performance. It's also important to
    /// note that this function may not be implemented in the most
    /// performance-optimal way.
    ///
    /// Also see [Self::rescale], [Self::rescale_bilinear], and
    /// [Self::rescale_bicubic].
    ///
    /// This will return a new [Frame], similar to how [Self::clone] does. The
    /// new [Frame] may have a different internal type (a different internal
    /// [FrameBuffer] implementation).
    pub fn rescale_nearest_neighbor(&self, new_dimensions: Dimensions) -> Self {
        let scale_x = self.dimensions().width() as f64 / new_dimensions.width() as f64;
        let scale_y = self.dimensions().height() as f64 / new_dimensions.height() as f64;

        Self::from_fill_with_coords(new_dimensions, |row, col| {
            get_pixel_clamped(
                self,
                ((row as f64) * scale_y) as isize,
                ((col as f64) * scale_x) as isize,
            )
        })
    }

    /// Rescale this [Frame] to have new [Dimensions] using the
    /// [bilinear](RescaleMethod::Bilinear) rescaling algorithm.
    ///
    /// Rescaling on the CPU is computationally expensive. See [RescaleMethod]
    /// for a comparison of each algorithm's performance. It's also important to
    /// note that this function may not be implemented in the most
    /// performance-optimal way.
    ///
    /// Also see [Self::rescale], [Self::rescale_nearest_neighbor], and
    /// [Self::rescale_bicubic].
    ///
    /// This will return a new [Frame], similar to how [Self::clone] does. The
    /// new [Frame] may have a different internal type (a different internal
    /// [FrameBuffer] implementation).
    pub fn rescale_bilinear(&self, new_dimensions: Dimensions) -> Self {
        let scale_x = self.dimensions().width() as f64 / new_dimensions.width() as f64;
        let scale_y = self.dimensions().height() as f64 / new_dimensions.height() as f64;

        Self::from_fill_with_coords(new_dimensions, |row, col| {
            let x = (col as f64 + 0.5) * scale_x - 0.5;
            let y = (row as f64 + 0.5) * scale_y - 0.5;

            let x0 = x as isize;
            let y0 = y as isize;
            let x1 = (x0 + 1).min(self.dimensions().width() as isize - 1);
            let y1 = (y0 + 1).min(self.dimensions().height() as isize - 1);

            let dx = x - x0 as f64;
            let dy = y - y0 as f64;

            let p00 = get_pixel_clamped(self, y0, x0);
            let p10 = get_pixel_clamped(self, y0, x1);
            let p01 = get_pixel_clamped(self, y1, x0);
            let p11 = get_pixel_clamped(self, y1, x1);

            let get_channel = |k: usize| -> u8 {
                let top = p00.channels()[k] as f64 * (1.0 - dx) + p10.channels()[k] as f64 * dx;
                let bottom = p01.channels()[k] as f64 * (1.0 - dx) + p11.channels()[k] as f64 * dx;
                (top * (1.0 - dy) + bottom * dy).round() as u8
            };

            Pixel::from_rgba(
                get_channel(Pixel::RED_OFFSET),
                get_channel(Pixel::GREEN_OFFSET),
                get_channel(Pixel::BLUE_OFFSET),
                get_channel(Pixel::ALPHA_OFFSET),
            )
        })
    }

    /// Rescale this [Frame] using the to have new [Dimensions] using the
    /// [bicubic](RescaleMethod::Bicubic) rescaling algorithm.
    ///
    /// Rescaling on the CPU is computationally expensive. See [RescaleMethod]
    /// for a comparison of each algorithm's performance. It's also important to
    /// note that this function may not be implemented in the most
    /// performance-optimal way.
    ///
    /// Also see [Self::rescale], [Self::rescale_nearest_neighbor], and
    /// [Self::rescale_bilinear].
    ///
    /// This will return a new [Frame], similar to how [Self::clone] does. The
    /// new [Frame] may have a different internal type (a different internal
    /// [FrameBuffer] implementation).
    pub fn rescale_bicubic(&self, new_dimensions: Dimensions) -> Self {
        /// Catmull-Rom spline weight function.
        fn cubic_weight(t: f64) -> f64 {
            let a = -0.5;
            let t = t.abs();
            if t <= 1.0 {
                (a + 2.0) * t.powf(3.0) - (a + 3.0) * t.powf(2.0) + 1.0
            } else if t < 2.0 {
                a * t.powf(3.0) - 5.0 * a * t.powf(2.0) + 8.0 * a * t - 4.0 * a
            } else {
                0.0
            }
        }

        let scale_x = self.dimensions().width() as f64 / new_dimensions.width() as f64;
        let scale_y = self.dimensions().height() as f64 / new_dimensions.height() as f64;

        Self::from_fill_with_coords(new_dimensions, |row, col| {
            let x = (col as f64 + 0.5) * scale_x - 0.5;
            let y = (row as f64 + 0.5) * scale_y - 0.5;

            let x_abs = x.floor();
            let y_abs = y.floor();

            let mut total_weight = 0.0;

            let mut channels = [0.0, 0.0, 0.0, 0.0];

            for m in -1..3 {
                for n in -1..3 {
                    let src_pixel = get_pixel_clamped(self, y_abs as isize + m, x_abs as isize + n);
                    let wx = cubic_weight(n as f64 - (x - x_abs));
                    let wy = cubic_weight(m as f64 - (y - y_abs));
                    let w = wx * wy;
                    total_weight += w;
                    for (k, channel) in channels.iter_mut().enumerate() {
                        *channel += src_pixel.channels()[k] as f64 * w;
                    }
                }
            }

            channels
                .map(|channel| (channel / total_weight).round() as u8)
                .into()
        })
    }

    /// Turn this [Frame] into the concrete [FrameBuffer] type `B` that's being
    /// stored internally.
    ///
    /// If the concrete type of the internal [FrameBuffer] is not of type `B`,
    /// this function will return an [Err] which will store the reconstructed
    /// [Frame] (the [Uid] will not have changed). See [Self::to_internal] if
    /// you just want the internal [Box]ed [FrameBuffer].
    ///
    /// If you just need access to the [FrameBuffer] temporarily but will end up
    /// reconstructing another [Frame] with it, see [Self::use_buffer] or
    /// [Self::swap_internal]
    pub fn to_buffer<B: FrameBuffer>(self) -> Result<B, Frame> {
        let original_uid = self.uid();

        match (self.to_internal() as Box<dyn Any>).downcast::<B>() {
            Ok(buffer) => Ok(*buffer),

            Err(boxed_any) => Err(
                // CONTRACT: The provided UID is from a frame that no longer
                // exists, so it is unique.
                Frame::from_parts(
                    *boxed_any
                        .downcast::<Box<dyn FrameBuffer>>()
                        .expect("`FrameBuffer` to `Any` and back to `FrameBuffer` should be ok."),
                    original_uid,
                ),
            ),
        }
    }

    /// Turn this [Frame] into the concrete [FrameBuffer] type `B` that's being
    /// stored internally, call `f` on it, and reconstruct a new [Frame] with
    /// the same [Uid] as the original.
    ///
    /// If the concrete type of the internal [FrameBuffer] is not of type `B`,
    /// this function will return an [Err] which will store the reconstructed
    /// [Frame] (the [Uid] will not have changed). See [Self::swap_internal] or
    /// [Self::to_internal] if you just want access to the internal
    /// [FrameBuffer].
    pub fn use_buffer<B, F>(self, f: F) -> Result<Self, Self>
    where
        B: FrameBuffer,
        F: FnOnce(&mut B),
    {
        let original_uid = self.uid();

        self.to_buffer::<B>().map(|mut buffer| {
            f(&mut buffer);

            // CONTRACT: The provided UID is from a frame that no longer exists,
            // so it is unique.
            Frame::from_parts(Box::new(buffer), original_uid)
        })
    }

    /// Turn this [Frame] into the [Box]ed [FrameBuffer] being stored
    /// internally.
    ///
    /// If you just need access to the [FrameBuffer] temporarily but will end up
    /// reconstructing another [Frame] with it, see [Self::use_buffer] or
    /// [Self::swap_internal]. If you know the concrete type of the internal
    /// [FrameBuffer], you may want to use [Self::to_buffer].
    pub fn to_internal(self) -> Box<dyn FrameBuffer> {
        // SAFETY: If we have a `self` reference, that means there are no live
        // references to `pixels` and the only reference to `self.buffer` is
        // `pixels`. Since `pixels` is a raw pointer, dropping it (which we're
        // doing at the end of the scope) cannot cause any reads/writes to
        // `buffer`.
        self.buffer
    }

    /// Turn this [Frame] into the [Box]ed [FrameBuffer] being stored
    /// internally, call `f` on it, and reconstruct a new [Frame] with whatever
    /// [Box]ed [FrameBuffer] `f` returned and the same [Uid] as the original.
    ///
    /// This function is useful for downcasting to a concrete type, doing some
    /// operation, and then reconstructing the [Frame] without changing the
    /// [FrameBuffer]. Its benefit is that it allows you to skip [Uid]
    /// generation (which is a non-trivial process).
    ///
    /// Nothing about the [FrameBuffer] that `f` returns has to be the same as
    /// the [FrameBuffer] that was passed in (e.g. dimensions can change,
    /// contents can change, even the concrete [FrameBuffer] type can change).
    ///
    /// If you know the concrete type of the internal [FrameBuffer], you may
    /// want to use [Self::use_buffer]. If you just want to extract the internal
    /// [FrameBuffer], see [Self::to_internal] or [Self::to_buffer].
    pub fn swap_internal<F>(self, f: F) -> Self
    where
        F: FnOnce(Box<dyn FrameBuffer>) -> Box<dyn FrameBuffer>,
    {
        let uid = self.uid();
        let buffer = f(self.to_internal());

        // CONTRACT: The provided UID is from a frame that no longer exists, so
        // it is unique.
        Self::from_parts(buffer, uid)
    }

    /// Create a frame without checking that the `pixels` slice length and
    /// dimensions work together.
    ///
    /// # Safety
    ///
    /// You shouldn't call this function if `pixels.len() != dimensions.area()`.
    /// Failing to uphold this can't cause undefined behavior, but it may result
    /// in a hard-to-debug panic down the line (from an out-of-bounds read). For
    /// this reason, this function is marked `unsafe`. See [Self::from_pixels]
    /// for a safe version.
    pub unsafe fn from_pixels_unchecked(pixels: Box<[Pixel]>, dimensions: Dimensions) -> Self {
        Self::from_buffer(BasicFrame { pixels, dimensions })
    }

    /// Create a frame with a raw data slice without checking the `data` slice
    /// length and dimensions work together and also without checking alignment.
    ///
    /// # Safety
    ///
    /// You shouldn't call this function if
    /// `data.len() != dimensions.area() * size_of::<Pixel>` or if the `data`
    /// slice is not aligned in a way that would allow it to be reinterpreted as
    /// a slice of [Pixel]s. Failing to uphold this can cause undefined
    /// behavior. See [Self::from_raw_data] for a safe version.
    pub unsafe fn from_raw_data_unchecked(data: Box<[u8]>, dimensions: Dimensions) -> Self {
        Self::from_buffer(BasicFrame {
            // SAFETY: It's on the caller to ensure `data` is long enough and
            // aligned properly. We're just casting from one "plain old data"
            // type to another. Since the length and alignment is on the caller,
            // we're ok here.
            pixels: unsafe {
                Box::from_raw(slice::from_raw_parts_mut(
                    Box::into_raw(data) as *mut Pixel,
                    dimensions.area(),
                ))
            },
            dimensions,
        })
    }

    /// Creates a new frame without initializing out the buffer. Instead, an
    /// `initializer` callback is used to initialize the data (without reading
    /// from it, since that would be undefined behavior).
    ///
    /// # Safety
    ///
    /// If `initializer` fails to initialize any of the memory it will likely
    /// result in undefined behavior.
    pub unsafe fn from_uninitialized_pixels<F>(dimensions: Dimensions, initializer: F) -> Self
    where
        F: FnOnce(&mut [MaybeUninit<Pixel>]),
    {
        let mut uninit_pixels = Box::new_uninit_slice(dimensions.area());
        // SAFETY:
        // 1. It's on the caller to ensure their callback initializes all of the
        //    memory.
        // 2. We created `uninit_pixels` with the exact amount of data required.
        initializer(&mut uninit_pixels);
        unsafe { Self::from_pixels_unchecked(uninit_pixels.assume_init(), dimensions) }
    }

    /// Similar to [Self::from_uninitialized_pixels] (see that documentation
    /// first), but the callback is provided a [u8] slice instead of a [Pixel]
    /// slice.
    ///
    /// # Safety
    ///
    /// If `initializer` fails to initialize any of the memory it will likely
    /// result in undefined behavior.
    pub unsafe fn from_uninitialized_raw_data<F>(dimensions: Dimensions, initializer: F) -> Self
    where
        F: FnOnce(&mut [MaybeUninit<u8>]),
    {
        // SAFETY:
        // 1. It's on the caller to ensure their callback initializes all of the
        //    memory.
        // 2. `Pixel` is just plain old data, so it's safe to cast to a `u8`
        //    slice.
        unsafe {
            Self::from_uninitialized_pixels(dimensions, |pixels| {
                initializer(cast_slice_mut(pixels));
            })
        }
    }

    /// Create a frame from the [Box]ed [FrameBuffer] trait object that will be
    /// stored internally and an unused [Uid].
    ///
    /// If `buffer.dimensions() != buffer.pixels_mut().len()`, this function
    /// will panic.
    ///
    /// # Contract
    ///
    /// The provided `uid` parameter *must* be unique. No other live frames can
    /// be storing the same [Uid] internally. This contract is unchecked, so
    /// this constructor is not public.
    fn from_parts(mut buffer: Box<dyn FrameBuffer>, uid: Uid) -> Self {
        let dimensions = buffer.dimensions();
        let pixels = buffer.pixels_mut();

        assert_eq!(
            dimensions.area(),
            pixels.len(),
            "The frame buffer's dimensions are not right for its buffer's length."
        );

        // We're about to do something really unsafe and weird and scary (and we
        // won't even need the `unsafe` keyword yet!) so I'll explain why first.
        // In the struct we're going to store a pointer to the pixels buffer
        // that `buffer.pixels_mut()` returns (`pixels` will reference itself).
        // We're doing this for the same reason we're also storing a copy of
        // `buffer.dimensions()` in a separate `dimensions` field. Yes, we can
        // access both of them through the `Box<dyn FrameBuffer>` field
        // (`buffer`), but that has some major performance tradeoffs for every
        // single method call:
        //
        // 1. To actually get access to the `buffer`'s pixels or dimensions it
        //    would require a heap pointer indirection (dereferenicng the `Box`
        //    pointer), a vtable lookup (figuring out what function to call, the
        //    runtime polymorphism indicated by the `dyn` keyword), and a
        //    function call (which the compiler will likely never be able to
        //    optimize out or inline since it has no idea what function is about
        //    to be called, again because of the runtime polymorphism (`dyn`)).
        // 2. We'd also need to validate that the `FrameBuffer` is actually
        //    implemented correctly. A `FrameBuffer` implementation could
        //    theoretically return two different dimensions on two subsequent
        //    calls to `dimensions()`, or it could return a different frame
        //    buffer each time you call `pixels_mut()`. If we only call those
        //    functions once, we only have to validate those values once (the
        //    assertion above is all we need). Skipping these checks wouldn't
        //    lead to undefined behavior, but they would almost definitely lead
        //    to a near impossible-to-debug panic from an out of bounds read (it
        //    would happen way later and probably on another thread).

        // SAFETY: We're casting away all of the safety of references here to
        // create a pointer to the pixels buffer `buffer.pixels_mut()` returns.
        // This is okay because:
        //
        // 1. We do not read/write using this reference after we drop the buffer
        //    that owns the data. We either own it until we consume `self` in
        //    `to_internal` and return without there being any reads/writes to
        //    `pixels` or we drop normally and it gets dropped after `pixels`
        //    (because fields are dropped in definition order).
        // 2. No methods will return a reference to this data with a lifetime
        //    longer than the lifetime of this object (notice how no methods
        //    even specify a lifetime explicitly, they're all `'_`).
        // 3. Because the buffer object is boxed, `FrameBuffer` doesn't have to
        //    be `Unpin`. It also means this type doesn't have to be `!Unpin`
        //    (we're not pointing to anything that could ever be on the stack,
        //    moving this object won't move the buffer we're pointing at).
        // 4. The `pixels` field is the only way we ever read/write to this
        //    data, except for returning from the `to_internal` function (a
        //    function that consumes `self`, so nothing external can even access
        //    the `pixels` field from the start of that function).
        // 5. Since `pixels` is a raw pointer and not a reference, Rust drops
        //    some of its aliasing restrictions, allowing us to store a second
        //    mutable way of accessing `buffer` (through `pixels`). Since we
        //    were given a `&mut` reference we are the only ones with any sort
        //    of read/write access to the buffer, we just have to ensure that we
        //    now only mutate the buffer through our new `pixels` pointer.
        let pixels = pixels as *mut [Pixel];

        Self {
            pixels,
            dimensions,
            buffer,

            // CONTRACT: It's on the caller to ensure this `Uid` is unique.
            uid,
        }
    }
}

// SAFETY: This thread is safe to send between threads, despite storing a raw
// pointer, because that raw pointer points to data on the heap that will not
// move when ownership is transferred between threads. `FrameBuffer` is also
// `Send`.
unsafe impl Send for Frame {}

// SAFETY: It is safe to share immutable references to this type between
// threads, despite storing a raw pointer, because this type upholds Rust's
// normal borrowing rules and the internal state of this object cannot be
// mutated through an immutable reference without synchronization. `FrameBuffer`
// is also `Sync`.
unsafe impl Sync for Frame {}

impl Debug for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("pixels", &self.pixels)
            .field("dimensions", &self.dimensions)
            .field("uid", &self.uid)
            .field("buffer", &"[omitted]")
            .finish()
    }
}

/// Note that cloning a [Frame] may result in the new frame having a different
/// internal type (a different internal [FrameBuffer] implementation).
impl Clone for Frame {
    /// Note that cloning a [Frame] may result in the new frame having a
    /// different internal type (a different internal [FrameBuffer]
    /// implementation).
    fn clone(&self) -> Self {
        // More unsafe :/ but *way* faster than initializing all pixels to a
        // solid color and then using a loop to change them.

        // SAFETY:
        // 1. We're initializing all of the pixels (`size_of_val(new_pixels)`
        //    bytes). No memory will be uninitialized.
        // 2. We're copying every byte from the original buffer to a new one.
        //    This is fine since `Pixel`s are just "plain old data" and
        //    `new_pixels` is guaranteed to be aligned and non-overlapping with
        //    `self`'s buffer.
        // 3. The pointer cast from `*mut MaybeUninit<Pixel>` to `*mut Pixel` is
        //    ok since `ptr::copy_nonoverlapping` will not write to any memory
        //    in the new buffer without having written to it first.
        unsafe {
            Self::from_uninitialized_pixels(self.dimensions, |new_pixels| {
                ptr::copy_nonoverlapping(
                    self.pixels().as_ptr(),
                    new_pixels.as_mut_ptr() as *mut Pixel,
                    self.pixels().len(),
                );
            })
        }
    }
}

/// Equality and hashing is based on the frame's [UID](Self::uid).
impl PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid
    }
}

/// Equality and hashing is based on the frame's [UID](Self::uid).
impl Eq for Frame {}

/// Equality and hashing is based on the frame's [UID](Self::uid).
impl Hash for Frame {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
    }
}

impl From<Box<dyn FrameBuffer>> for Frame {
    fn from(buffer: Box<dyn FrameBuffer>) -> Self {
        Self::from_internal(buffer)
    }
}

impl<B: FrameBuffer> From<B> for Frame {
    fn from(buffer: B) -> Self {
        Self::from_buffer(buffer)
    }
}

/// Use the `[]` operator to get a reference to a row from the buffer.
impl Index<usize> for Frame {
    type Output = [Pixel];

    fn index(&self, index: usize) -> &Self::Output {
        self.pixel_rows()
            .nth(index)
            .expect("Index shouldn't be out of bounds.")
    }
}

/// Use the `[]` operator to get a *mutable* reference to a row from the buffer.
impl IndexMut<usize> for Frame {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.pixel_rows_mut()
            .nth(index)
            .expect("Index shouldn't be out of bounds.")
    }
}

/// What method to use to [rescale](Frame::rescale) a [Frame].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RescaleMethod {
    /// The fastest algorithm, low quality.
    NearestNeighbor,
    /// A good balance of speed and quality.
    Bilinear,
    /// The slowest algorithm, highest quality.
    Bicubic,
}

/// The default method is [Bilinear](RescaleMethod::Bilinear).
impl Default for RescaleMethod {
    fn default() -> Self {
        Self::Bilinear
    }
}

/// Indicates that a slice was invalid for creating a frame.
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TryFromSliceError {
    #[error("The slice was an invalid length.")]
    LenError,
    #[error("The slice was not aligned properly.")]
    AlignmentError,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("Expected dimensions {expected} but got {actual}.")]
pub struct DifferentDimensionsError {
    pub expected: Dimensions,
    pub actual: Dimensions,
}

/// A basic [FrameBuffer]. This is what is stored internally when you call
/// [Frame::new] (or any of the other constructors where you don't explicitly
/// provide a [FrameBuffer]).
#[derive(Debug, Clone)]
struct BasicFrame {
    pixels: Box<[Pixel]>,
    dimensions: Dimensions,
}

impl FrameBuffer for BasicFrame {
    fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    fn pixels_mut(&mut self) -> &mut [Pixel] {
        &mut self.pixels
    }
}

/// Cast one kind of slice to another.
///
/// # Safety
///
/// It's on the caller to ensure this does not invoke undefined behavior.
const unsafe fn cast_slice<Src, Dest>(slice: &[Src]) -> &[Dest] {
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
const unsafe fn cast_slice_mut<Src, Dest>(slice: &mut [Src]) -> &mut [Dest] {
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

fn get_pixel_clamped(frame: &Frame, row: isize, col: isize) -> Pixel {
    let row = row.clamp(0, frame.dimensions().height() as isize - 1) as usize;
    let col = col.clamp(0, frame.dimensions().width() as isize - 1) as usize;
    frame[row][col]
}
