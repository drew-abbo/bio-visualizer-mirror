//! Declares the [Dimensions] type, a type that [super::Frame] depends on.

use std::fmt::{self, Display, Formatter};
use std::num::{NonZeroU32};

/// A width and a height, both guaranteed to be non-zero.
///
/// # Example
///
/// [From] is implemented for `(u32, u32)` and [Dimensions]. If either side is
/// `0`, the thread will panic. [Into::into] should really only be used if
/// you're providing the side lengths as literals (e.g. `(1920, 1080).into()`).
///
/// ```
/// use media::frame::Dimensions;
///
/// let d: Dimensions = (1920, 1080).into();
/// assert_eq!(d.width(), 1920);
/// assert_eq!(d.height(), 1080);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dimensions {
    // NOTE: It's common for most libraries to use `u32` for dimensions instead
    // of `usize`, so that's what we're doing here.
    width: NonZeroU32,
    height: NonZeroU32,
}

impl Dimensions {
    /// Construct from a width and a height.
    ///
    /// This function will return [None] if the width or height are 0. Also see
    /// [Self::from_non_zero].
    pub const fn new(width: u32, height: u32) -> Option<Self> {
        let Some(width) = NonZeroU32::new(width) else {
            return None;
        };
        let Some(height) = NonZeroU32::new(height) else {
            return None;
        };

        Some(Self::from_non_zero(width, height))
    }

    /// Construct from a non-zero width and a height.
    ///
    /// Unlike [Self::new], this function will always succeed (since
    /// [NonZeroU32] ensures the sides are both non-zero at compile time).
    pub const fn from_non_zero(width: NonZeroU32, height: NonZeroU32) -> Self {
        Self { width, height }
    }

    /// The same as [Self::new], just without checking that the `width` and
    /// `height` are both non-zero.
    ///
    /// # Safety
    ///
    /// Either parameter being `0` will invoke undefined behavior.
    pub const unsafe fn new_unchecked(width: u32, height: u32) -> Self {
        // SAFETY: It's on the caller to ensure that `width` and `height` are
        // both non-zero.
        unsafe {
            Self::from_non_zero(
                NonZeroU32::new(width).unwrap_unchecked(),
                NonZeroU32::new(height).unwrap_unchecked(),
            )
        }
    }

    /// The dimensions' width.
    ///
    /// This will never be `0`. Also see [Self::width_non_zero].
    pub const fn width(&self) -> u32 {
        self.width.get()
    }

    /// The dimensions' width as a [NonZeroU32].
    pub const fn width_non_zero(&self) -> NonZeroU32 {
        self.width
    }

    /// The dimensions' height.
    ///
    /// This will never be `0`. Also see [Self::height_non_zero].
    pub const fn height(&self) -> u32 {
        self.height.get()
    }

    /// The dimensions' height as a [NonZeroU32].
    pub const fn height_non_zero(&self) -> NonZeroU32 {
        self.height
    }

    /// The area a rectangle would have with the dimensions' width and height.
    ///
    /// This will never be `0`. Also see [Self::area_non_zero].
    pub const fn area(&self) -> u32 {
        self.width.get() * self.height.get()
    }

    /// The area a rectangle would have with the dimensions' width and height as
    /// a [NonZeroU32].
    pub const fn area_non_zero(&self) -> NonZeroU32 {
        NonZeroU32::new(self.area()).unwrap()
    }

    /// Find the aspect ratio of these dimensions.
    ///
    /// # Example
    ///
    /// ```
    /// use media::frame::Dimensions;
    ///
    /// let d: Dimensions = (1920, 1080).into();
    /// let ratio = d.aspect_ratio();
    /// assert_eq!(ratio, (16, 9).into());
    /// ```
    pub const fn aspect_ratio(&self) -> Self {
        let gcd = greatest_common_divisor(self.width.get(), self.height.get());

        // SAFETY: The sides are non-zero, `gcd` cannot be 0, and `gcd` also
        // cannot be greater than the width or height. Because of all of this,
        // dividing a side by `gcd` cannot result in a side length of `0`.
        unsafe { Self::new_unchecked(self.width.get() / gcd, self.height.get() / gcd) }
    }

    /// Try to rescale to match a new height. This function will fail if
    /// rescaling would mean the the aspect ratio would change.
    ///
    /// # Example
    ///
    /// ```
    /// use media::frame::Dimensions;
    ///
    /// let d: Dimensions = (1920, 1080).into();
    /// let ratio = d.rescale_height(720).unwrap();
    /// assert_eq!(ratio, (1280, 720).into());
    /// ```
    pub fn rescale_height(&self, new_height: u32) -> Option<Self> {
        let scaled_width = self.width.get() * new_height;
        if !scaled_width.is_multiple_of(self.height.get()) {
            return None;
        }
        Self::new(scaled_width / self.height.get(), new_height)
    }

    /// Like [Self::rescale_height], but it will round to the closest aspect
    /// ratio instead of failing. *Aspect ratio may not be preserved!*.
    ///
    /// [None] is returned if either side would be `0`.
    ///
    /// # Example
    ///
    /// ```
    /// use media::frame::Dimensions;
    ///
    /// let d: Dimensions = (1920, 1080).into();
    /// let ratio = d.rescale_height_rounded(721).unwrap();
    /// assert_eq!(ratio, (1282, 721).into());
    /// ```
    pub fn rescale_height_rounded(&self, new_height: u32) -> Option<Self> {
        let new_width =
            (((self.width.get() * new_height) as f64 / self.height.get() as f64).round() as u32)
                .max(1);
        Self::new(new_width, new_height)
    }

    /// Try to rescale to match a new width. This function will fail if
    /// rescaling would mean the aspect ratio would change.
    ///
    /// # Example
    ///
    /// ```
    /// use media::frame::Dimensions;
    ///
    /// let d: Dimensions = (1920, 1080).into();
    /// let ratio = d.rescale_width(1280).unwrap();
    /// assert_eq!(ratio, (1280, 720).into());
    /// ```
    pub fn rescale_width(&self, new_width: u32) -> Option<Self> {
        let scaled_height = self.height.get() * new_width;
        if !scaled_height.is_multiple_of(self.width.get()) {
            return None;
        }
        Self::new(new_width, scaled_height / self.width.get())
    }

    /// Like [Self::rescale_width], but it will round to the closest aspect
    /// ratio instead of failing. *Aspect ratio may not be preserved!*.
    ///
    /// [None] is returned if either side would be `0`.
    ///
    /// # Example
    ///
    /// ```
    /// use media::frame::Dimensions;
    ///
    /// let d: Dimensions = (1920, 1080).into();
    /// let ratio = d.rescale_width_rounded(1281).unwrap();
    /// assert_eq!(ratio, (1281, 721).into());
    /// ```
    pub fn rescale_width_rounded(&self, new_width: u32) -> Option<Self> {
        let new_height =
            (((self.height.get() * new_width) as f64 / self.width.get() as f64).round() as u32)
                .max(1);
        Self::new(new_width, new_height)
    }
}

/// Ordering depends on [area](Self::area).
impl Ord for Dimensions {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.area().cmp(&other.area())
    }
}

/// Ordering depends on [area](Self::area).
impl PartialOrd for Dimensions {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// When displayed, [Dimensions] will look like `WxH` (e.g. `1920x1080`).
impl Display for Dimensions {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// If either side is `0`, the thread will panic. [Into::into] should really
/// only be used if you're providing the side lengths as literals (e.g.
/// `(1920, 1080).into()`).
impl From<(u32, u32)> for Dimensions {
    fn from(dimensions: (u32, u32)) -> Self {
        Self::new(dimensions.0, dimensions.1).expect("Both sides must be non-zero.")
    }
}

impl From<Dimensions> for (u32, u32) {
    fn from(dimensions: Dimensions) -> Self {
        (dimensions.width(), dimensions.height())
    }
}

impl From<(NonZeroU32, NonZeroU32)> for Dimensions {
    fn from(dimensions: (NonZeroU32, NonZeroU32)) -> Self {
        Self::from_non_zero(dimensions.0, dimensions.1)
    }
}

impl From<Dimensions> for (NonZeroU32, NonZeroU32) {
    fn from(dimensions: Dimensions) -> Self {
        (dimensions.width_non_zero(), dimensions.height_non_zero())
    }
}

const fn greatest_common_divisor(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (b, a) = (a % b, b)
    }
    a
}
