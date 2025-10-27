//! Declares the [Dimensions] type, a type that [super::Frame] depends on.

use std::fmt::{self, Display, Formatter};
use std::num::NonZeroUsize;

/// A width and a height, both guaranteed to be non-zero.
///
/// # Example
///
/// [From<(usize, usize)>] is implemented for [Dimensions]. If either side is
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
    width: NonZeroUsize,
    height: NonZeroUsize,
}

impl Dimensions {
    /// Construct from a width and a height.
    ///
    /// This function will return [None] if the width or height are 0. Also see
    /// [Self::from_non_zero].
    pub const fn new(width: usize, height: usize) -> Option<Self> {
        let Some(width) = NonZeroUsize::new(width) else {
            return None;
        };
        let Some(height) = NonZeroUsize::new(height) else {
            return None;
        };

        Some(Self::from_non_zero(width, height))
    }

    /// Construct from a non-zero width and a height.
    ///
    /// Unlike [Self::new], this function will always succeed (since
    /// [NonZeroUsize] ensures the sides are both non-zero at compile time).
    pub const fn from_non_zero(width: NonZeroUsize, height: NonZeroUsize) -> Self {
        Self { width, height }
    }

    /// The same as [Self::new], just without checking that the `width` and
    /// `height` are both non-zero.
    ///
    /// # Safety
    ///
    /// Either parameter being `0` will invoke undefined behavior.
    pub const unsafe fn new_unchecked(width: usize, height: usize) -> Self {
        // SAFETY: It's on the caller to ensure that `width` and `height` are
        // both non-zero.
        unsafe {
            Self::from_non_zero(
                NonZeroUsize::new(width).unwrap_unchecked(),
                NonZeroUsize::new(height).unwrap_unchecked(),
            )
        }
    }

    /// The dimensions' width.
    ///
    /// This will never be `0`. Also see [Self::width_non_zero].
    pub const fn width(&self) -> usize {
        self.width.get()
    }

    /// The dimensions' width as a [NonZeroUsize].
    pub const fn width_non_zero(&self) -> NonZeroUsize {
        self.width
    }

    /// The dimensions' height.
    ///
    /// This will never be `0`. Also see [Self::height_non_zero].
    pub const fn height(&self) -> usize {
        self.height.get()
    }

    /// The dimensions' height as a [NonZeroUsize].
    pub const fn height_non_zero(&self) -> NonZeroUsize {
        self.height
    }

    /// The area a rectangle would have with the dimensions' width and height.
    ///
    /// This will never be `0`. Also see [Self::area_non_zero].
    pub const fn area(&self) -> usize {
        self.width.get() * self.height.get()
    }

    /// The area a rectangle would have with the dimensions' width and height as
    /// a [NonZeroUsize].
    pub const fn area_non_zero(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.area()).unwrap()
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
    pub fn rescale_height(&self, new_height: usize) -> Option<Self> {
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
    pub fn rescale_height_rounded(&self, new_height: usize) -> Option<Self> {
        let new_width =
            (((self.width.get() * new_height) as f64 / self.height.get() as f64).round() as usize)
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
    pub fn rescale_width(&self, new_width: usize) -> Option<Self> {
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
    pub fn rescale_width_rounded(&self, new_width: usize) -> Option<Self> {
        let new_height =
            (((self.height.get() * new_width) as f64 / self.width.get() as f64).round() as usize)
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
impl From<(usize, usize)> for Dimensions {
    fn from(dimensions: (usize, usize)) -> Self {
        Self::new(dimensions.0, dimensions.1).expect("Both sides must be non-zero.")
    }
}

impl From<Dimensions> for (usize, usize) {
    fn from(dimensions: Dimensions) -> Self {
        (dimensions.width(), dimensions.height())
    }
}

impl From<(NonZeroUsize, NonZeroUsize)> for Dimensions {
    fn from(dimensions: (NonZeroUsize, NonZeroUsize)) -> Self {
        Self::from_non_zero(dimensions.0, dimensions.1)
    }
}

impl From<Dimensions> for (NonZeroUsize, NonZeroUsize) {
    fn from(dimensions: Dimensions) -> Self {
        (dimensions.width_non_zero(), dimensions.height_non_zero())
    }
}

const fn greatest_common_divisor(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (b, a) = (a % b, b)
    }
    a
}
