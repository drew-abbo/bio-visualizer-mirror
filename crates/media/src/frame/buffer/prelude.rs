//! This module declares all of the types [Frame](super::Frame)s depend on.
//!
//! # Safety
//!
//! This module's parent ([super]) uses `unsafe` code (for performance reasons)
//! that assumes certain things about the data types in this module (especially
//! [Pixel]). Be very careful when modifying this module.

use std::cell::{OnceCell, RefCell};
use std::fmt::{self, Debug, Display, Formatter};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A width and a height, both guaranteed to be non-zero.
///
/// # Example
///
/// [From<(usize, usize)>] is implemented for [Dimensions]. If either side is
/// `0`, the thread will panic. [Into::into] should really only be used if
/// you're providing the side lengths as literals (e.g. `(1920, 1080).into()`).
///
/// ```
/// let d: Dimensions = (1920, 1080).into();
/// assert_eq!(d.width(), 1920);
/// assert_eq!(d.width(), 1080);
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
    /// let d: Dimensions = (1920, 1080).into();
    /// let ratio = d.aspect_ratio().unwrap();
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

/// A macro for creating [Pixel]s at compile-time from hex-strings in the format
/// `#RRGGBBAA` or `#RRGGBB`.
///
/// # Example
///
/// ```
/// use media::frame::Pixel;
/// use media::pixel;
///
/// let pixel = pixel!("#AA5500FF");
/// assert_eq!(pixel.red(), 0xAA);
/// assert_eq!(pixel.green(), 0x55);
/// assert_eq!(pixel.blue(), 0x00);
/// assert_eq!(pixel.alpha(), 0xFF);
/// assert_eq!(pixel, Pixel::YELLOW);
/// ```
#[macro_export]
macro_rules! pixel {
    ($s: literal) => {{
        $crate::frame::Pixel::from_hex_str($s).expect(concat!(
            "Invalid hex string format. Expected something like ",
            "`pixel!(\"#RRGGBB\")` or `pixel!(\"#RRGGBBAA\")`."
        ))
    }};
}

/// A 32-bit RGBA pixel in the sRGB color space with four 8-bit channels: red,
/// green, blue, and alpha (opacity).
///
/// Internally, a [Pixel] is just 4 [u8]s stored contiguously with no extra
/// padding. As such, [Pixel]s are just "plain old data". This means they are
/// trivially copyable (i.e. by functions like [std::ptr::copy]), exactly 4
/// bytes in size (32 bits), and 100% safe to reinterpret as arrays/slices of
/// [u8].
///
/// For the color channels, a higher number means more color (i.e. `0` is no
/// color, `FF` (`255`) is max color). For the alpha channel, a higher number
/// means more opacity (i.e. `0` is completely transparent, `FF` (`255`) is
/// completely opaque).
///
/// | Channel         | Byte Offset |
/// | --------------- | ----------- |
/// | Red             | 0           |
/// | Green           | 1           |
/// | Blue            | 2           |
/// | Alpha (opacity) | 3           |
///
/// See the [pixel] macro for a nice way to construct [Pixel]s at compile-time.
///
/// # Example
///
/// ```
/// use media::frame::Pixel;
/// use media::pixel;
///
/// let pixel = pixel!("#000000FF");
/// assert_eq!(size_of_val(&pixel), 4);
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Pixel {
    /// SAFETY: Do not change this internal type. Slices of [Pixel]s are
    /// frequently reinterpreted as slices of [u8]s. [Pixel] must remain 4 bytes
    /// of "plain old data" or you'll break unsafe code and create undefined
    /// behavior.
    channels: [u8; 4],
}

impl Pixel {
    /// The byte offset of the [red](Self::red) channel.
    pub const RED_OFFSET: usize = 0;

    /// The byte offset of the [green](Self::green) channel.
    pub const GREEN_OFFSET: usize = 1;

    /// The byte offset of the [blue](Self::blue) channel.
    pub const BLUE_OFFSET: usize = 2;

    /// The byte offset of the [alpha](Self::alpha) (opacity) channel.
    pub const ALPHA_OFFSET: usize = 3;

    /// A constant representing the color BLACK (`#000000FF`).
    pub const BLACK: Self = pixel!("#000000FF");

    /// A constant representing the color RED (`#AA0000FF`).
    pub const RED: Self = pixel!("#AA0000FF");

    /// A constant representing the color GREEN (`#00AA00FF`).
    pub const GREEN: Self = pixel!("#00AA00FF");

    /// A constant representing the color YELLOW (`#AA5500FF`).
    pub const YELLOW: Self = pixel!("#AA5500FF");

    /// A constant representing the color BLUE (`#0000AAFF`).
    pub const BLUE: Self = pixel!("#0000AAFF");

    /// A constant representing the color MAGENTA (`#AA00AAFF`).
    pub const MAGENTA: Self = pixel!("#AA00AAFF");

    /// A constant representing the color CYAN (`#00AAAAFF`).
    pub const CYAN: Self = pixel!("#00AAAAFF");

    /// A constant representing the color WHITE (`#AAAAAAFF`).
    pub const WHITE: Self = pixel!("#AAAAAAFF");

    /// A constant representing the color BRIGHT_BLACK (`#555555FF`).
    pub const BRIGHT_BLACK: Self = pixel!("#555555FF");

    /// A constant representing the color BRIGHT_RED (`#FF5555FF`).
    pub const BRIGHT_RED: Self = pixel!("#FF5555FF");

    /// A constant representing the color BRIGHT_GREEN (`#55FF55FF`).
    pub const BRIGHT_GREEN: Self = pixel!("#55FF55FF");

    /// A constant representing the color BRIGHT_YELLOW (`#FFFF55FF`).
    pub const BRIGHT_YELLOW: Self = pixel!("#FFFF55FF");

    /// A constant representing the color BRIGHT_BLUE (`#5555FFFF`).
    pub const BRIGHT_BLUE: Self = pixel!("#5555FFFF");

    /// A constant representing the color BRIGHT_MAGENTA (`#FF55FFFF`).
    pub const BRIGHT_MAGENTA: Self = pixel!("#FF55FFFF");

    /// A constant representing the color BRIGHT_CYAN (`#55FFFFFF`).
    pub const BRIGHT_CYAN: Self = pixel!("#55FFFFFF");

    /// A constant representing the color BRIGHT_WHITE (`#FFFFFFFF`).
    pub const BRIGHT_WHITE: Self = pixel!("#FFFFFFFF");

    /// Create a new pixel from each of the RGBA channels.
    ///
    /// Also see the [pixel] macro.
    pub const fn from_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            channels: [red, green, blue, alpha],
        }
    }

    /// Create a new pixel from the only the RGB channels, setting the
    /// [alpha](Self::alpha) channel to `0xFF` (100%, completely opaque).
    ///
    /// Also see the [pixel] macro.
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::from_rgba(red, green, blue, 0xFF)
    }

    /// Create a new pixel from each of the RGBA channels as normalized floating
    /// point values in the range `[0.0, 1.0]` (inclusive). Inputs are clamped.
    pub const fn from_rgba_normalized(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self::from_rgba(
            Self::denormalize_channel(red),
            Self::denormalize_channel(green),
            Self::denormalize_channel(blue),
            Self::denormalize_channel(alpha),
        )
    }

    /// Create a new pixel from only the RGB channels as normalized floating
    /// point values in the range `[0.0, 1.0]` (inclusive). The
    /// [alpha](Self::alpha) channel will be set to to `0xFF` (100%, completely
    /// opaque). Inputs are clamped.
    pub const fn from_rgb_normalized(red: f64, green: f64, blue: f64) -> Self {
        Self::from_rgb(
            Self::denormalize_channel(red),
            Self::denormalize_channel(green),
            Self::denormalize_channel(blue),
        )
    }

    /// Create a [Pixel] from a string in the format `#RRGGBBAA` or `#RRGGBB`.
    /// If `s` is not the above format, [None] will be returned.
    ///
    /// To construct a [Pixel] from a string at compile-time, see the [pixel]
    /// macro or [Self::from_hex_str].
    pub fn from_hex<S: AsRef<str>>(s: S) -> Option<Self> {
        Self::from_hex_str(s.as_ref())
    }

    /// Create a [Pixel] from a string in the format `#RRGGBBAA` or `#RRGGBB`.
    /// If `s` is not the above format, [None] will be returned.
    ///
    /// This function only accepts [str] to remain `const`-compatible. For a
    /// version that accepts any kind of string, see [Self::from_hex]. If you
    /// need `const`-compatibility, you're probably better off using the [pixel]
    /// macro in most cases.
    pub const fn from_hex_str(s: &str) -> Option<Self> {
        const fn chr_to_u8(chr: u8) -> Option<u8> {
            match chr {
                b'0'..=b'9' => Some(chr - b'0'),
                b'a'..=b'f' => Some(chr - b'a' + 10),
                b'A'..=b'F' => Some(chr - b'A' + 10),
                _ => None,
            }
        }

        const fn two_digits(s: &[u8], i: usize) -> Option<u8> {
            let Some(digit1) = chr_to_u8(s[i]) else {
                return None;
            };
            let Some(digit2) = chr_to_u8(s[i + 1]) else {
                return None;
            };

            Some(digit1 << 4 | digit2)
        }

        let s = s.as_bytes();

        if (s.len() != 9 && s.len() != 7) || s[0] != b'#' {
            return None;
        }

        let Some(red) = two_digits(s, 1) else {
            return None;
        };
        let Some(green) = two_digits(s, 3) else {
            return None;
        };
        let Some(blue) = two_digits(s, 5) else {
            return None;
        };
        let alpha = if s.len() == 7 {
            0xFF
        } else {
            let Some(alpha) = two_digits(s, 7) else {
                return None;
            };
            alpha
        };

        Some(Self::from_rgba(red, green, blue, alpha))
    }

    /// The red channel.
    pub const fn red(&self) -> u8 {
        self.channels[0]
    }

    /// The green channel.
    pub const fn green(&self) -> u8 {
        self.channels[1]
    }

    /// The blue channel.
    pub const fn blue(&self) -> u8 {
        self.channels[2]
    }

    /// The alpha (opacity) channel.
    pub const fn alpha(&self) -> u8 {
        self.channels[3]
    }

    /// An array of all 4 channels, ordered red, green, blue, alpha (opacity).
    pub const fn channels(&self) -> [u8; 4] {
        self.channels
    }

    /// A *mutable* reference to the internal red channel.
    pub const fn red_mut(&mut self) -> &mut u8 {
        &mut self.channels[0]
    }

    /// A *mutable* reference to the internal green channel.
    pub const fn green_mut(&mut self) -> &mut u8 {
        &mut self.channels[1]
    }

    /// A *mutable* reference to the internal blue channel.
    pub const fn blue_mut(&mut self) -> &mut u8 {
        &mut self.channels[2]
    }

    /// A *mutable* reference to the internal alpha (opacity) channel.
    pub const fn alpha_mut(&mut self) -> &mut u8 {
        &mut self.channels[3]
    }

    /// A *mutable* reference to the internal array of all 4 channels, ordered
    /// red, green, blue, alpha (opacity).
    pub const fn channels_mut(&mut self) -> &mut [u8; 4] {
        &mut self.channels
    }

    /// The red channel as a float in the range `[0.0, 1.0]` (inclusive).
    pub const fn red_normalized(&self) -> f64 {
        Self::normalize_channel(self.red())
    }

    /// The green channel as a float in the range `[0.0, 1.0]` (inclusive).
    pub const fn green_normalized(&self) -> f64 {
        Self::normalize_channel(self.green())
    }

    /// The blue channel as a float in the range `[0.0, 1.0]` (inclusive).
    pub const fn blue_normalized(&self) -> f64 {
        Self::normalize_channel(self.blue())
    }

    /// The alpha channel as a float in the range `[0.0, 1.0]` (inclusive).
    pub const fn alpha_normalized(&self) -> f64 {
        Self::normalize_channel(self.alpha())
    }

    /// Whether this pixel is completely transparent.
    pub const fn is_transparent(&self) -> bool {
        self.alpha() == 0x00
    }

    /// Whether this pixel is completely opaque.
    pub const fn is_opaque(&self) -> bool {
        self.alpha() == 0xFF
    }

    /// Whether this pixel is neither completely transparent or completely
    /// opaque.
    pub const fn is_translucent(&self) -> bool {
        !self.is_transparent() && !self.is_opaque()
    }

    /// The "percieved" brightness of a pixel, calculated using the Rec. 709
    /// relative luminance formula. The alpha channel is ignored.
    pub fn perceptual_brightness(&self) -> u8 {
        Self::denormalize_channel(self.perceptual_brightness_normalized())
    }

    /// The "percieved" brightness of a pixel, calculated using the Rec. 709
    /// relative luminance formula as a normalized float in the range
    /// `[0.0, 1.0]` (inclusive). The alpha channel is ignored.
    pub fn perceptual_brightness_normalized(&self) -> f64 {
        /// Converts from sRGB (display-encoded) to linear light.
        fn srgb_to_linear(channel: f64) -> f64 {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }

        let red = srgb_to_linear(self.red_normalized());
        let green = srgb_to_linear(self.green_normalized());
        let blue = srgb_to_linear(self.blue_normalized());

        0.2126 * red + 0.7152 * green + 0.0722 * blue
    }

    /// Create a new pixel with the [alpha](Self::alpha) channel set to `0xFF`
    /// (100%), making the pixel completely opaque.
    #[must_use]
    pub const fn remove_transparency(&self) -> Self {
        Self::from_rgb(self.red(), self.green(), self.blue())
    }

    /// Get a new pixel with a different [red](Self::red) channel.
    #[must_use]
    pub const fn set_red(&self, red: u8) -> Self {
        Self::from_rgba(red, self.green(), self.blue(), self.alpha())
    }

    /// Get a new pixel with a different [green](Self::green) channel.
    #[must_use]
    pub const fn set_green(&self, green: u8) -> Self {
        Self::from_rgba(self.red(), green, self.blue(), self.alpha())
    }

    /// Get a new pixel with a different [blue](Self::blue) channel.
    #[must_use]
    pub const fn set_blue(&self, blue: u8) -> Self {
        Self::from_rgba(self.red(), self.green(), blue, self.alpha())
    }

    /// Get a new pixel with a different [alpha](Self::alpha) (opacity) channel.
    #[must_use]
    pub const fn set_alpha(&self, alpha: u8) -> Self {
        Self::from_rgba(self.red(), self.green(), self.blue(), alpha)
    }

    /// Get a new pixel with a different [red](Self::red) channel value as a
    /// [normalized](Self::normalize_channel) float in the range `[0.0, 1.0]`
    /// (inclusive). Inputs are clamped.
    #[must_use]
    pub const fn set_red_normalized(&self, red: f64) -> Self {
        Self::from_rgba(
            Self::denormalize_channel(red),
            self.green(),
            self.blue(),
            self.alpha(),
        )
    }

    /// Get a new pixel with a different [green](Self::green) channel value as a
    /// [normalized](Self::normalize_channel) float in the range `[0.0, 1.0]`
    /// (inclusive). Inputs are clamped.
    #[must_use]
    pub const fn set_green_normalized(&self, green: f64) -> Self {
        Self::from_rgba(
            self.red(),
            Self::denormalize_channel(green),
            self.blue(),
            self.alpha(),
        )
    }

    /// Get a new pixel with a different [blue](Self::blue) channel value as a
    /// [normalized](Self::normalize_channel) float in the range `[0.0, 1.0]`
    /// (inclusive). Inputs are clamped.
    #[must_use]
    pub const fn set_blue_normalized(&self, blue: f64) -> Self {
        Self::from_rgba(
            self.red(),
            self.green(),
            Self::denormalize_channel(blue),
            self.alpha(),
        )
    }

    /// Get a new pixel with a different [alpha](Self::alpha) (opacity) channel
    /// value as a [normalized](Self::normalize_channel) float in the range
    /// `[0.0, 1.0]` (inclusive). Inputs are clamped.
    #[must_use]
    pub const fn set_alpha_normalized(&self, alpha: f64) -> Self {
        Self::from_rgba(
            self.red(),
            self.green(),
            self.blue(),
            Self::denormalize_channel(alpha),
        )
    }

    /// Convert a channel's value (a [u8] value, 0-255) to a float between 0.0
    /// and 1.0 (inclusive).
    ///
    /// Also see [Self::denormalize_channel].
    pub const fn normalize_channel(channel: u8) -> f64 {
        channel as f64 / 255.0
    }

    /// Convert a channel's normalized value (an [f64] value between 0.0 and 1.0
    /// (inclusive)) to a [u8] value (0-255). The input is clamped.
    ///
    /// Also see [Self::normalize_channel].
    pub const fn denormalize_channel(channel: f64) -> u8 {
        (channel.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

/// The default for [Pixel] is [Pixel::BLACK].
impl Default for Pixel {
    fn default() -> Self {
        Self::BLACK
    }
}

impl From<(u8, u8, u8, u8)> for Pixel {
    fn from(pixel: (u8, u8, u8, u8)) -> Self {
        Self::from_rgba(pixel.0, pixel.1, pixel.2, pixel.3)
    }
}

impl From<Pixel> for (u8, u8, u8, u8) {
    fn from(pixel: Pixel) -> Self {
        (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha())
    }
}

impl From<(u8, u8, u8)> for Pixel {
    fn from(pixel: (u8, u8, u8)) -> Self {
        Self::from_rgb(pixel.0, pixel.1, pixel.2)
    }
}

impl From<Pixel> for (u8, u8, u8) {
    fn from(pixel: Pixel) -> Self {
        (pixel.red(), pixel.green(), pixel.blue())
    }
}

impl From<(f64, f64, f64, f64)> for Pixel {
    fn from(pixel: (f64, f64, f64, f64)) -> Self {
        Self::from_rgba_normalized(pixel.0, pixel.1, pixel.2, pixel.3)
    }
}

impl From<Pixel> for (f64, f64, f64, f64) {
    fn from(pixel: Pixel) -> Self {
        (
            pixel.red_normalized(),
            pixel.green_normalized(),
            pixel.blue_normalized(),
            pixel.alpha_normalized(),
        )
    }
}

impl From<(f64, f64, f64)> for Pixel {
    fn from(pixel: (f64, f64, f64)) -> Self {
        Self::from_rgb_normalized(pixel.0, pixel.1, pixel.2)
    }
}

impl From<Pixel> for (f64, f64, f64) {
    fn from(pixel: Pixel) -> Self {
        (
            pixel.red_normalized(),
            pixel.green_normalized(),
            pixel.blue_normalized(),
        )
    }
}

impl From<[u8; 4]> for Pixel {
    fn from(pixel: [u8; 4]) -> Self {
        Self { channels: pixel }
    }
}

impl From<Pixel> for [u8; 4] {
    fn from(pixel: Pixel) -> Self {
        pixel.channels()
    }
}

impl AsRef<[u8; 4]> for Pixel {
    fn as_ref(&self) -> &[u8; 4] {
        &self.channels
    }
}

impl AsMut<[u8; 4]> for Pixel {
    fn as_mut(&mut self) -> &mut [u8; 4] {
        &mut self.channels
    }
}

impl AsRef<[u8]> for Pixel {
    fn as_ref(&self) -> &[u8] {
        &self.channels
    }
}

impl AsMut<[u8]> for Pixel {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.channels
    }
}

// Pixels must remain 4 bytes (32 bits) no matter what.
const _: () = assert!(size_of::<Pixel>() == 4);

/// A unique identifier for a [Frame](super::Frame). For the
/// duration of a [Frame](super::Frame)'s lifetime, no other frames will have an
/// equal [Uid].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uid {
    major_id: usize,
    minor_id: usize,
}

impl Uid {
    /// Generates a new [Uid].
    ///
    /// The first UID generated on a thread requires some synchronization (an
    /// atomic fetch-add). All subsequent UIDs will be able to be generated
    /// entirely thread-locally.
    pub fn new() -> Self {
        // `Uid`s hold a major ID (`major_id`) and a minor ID (`minor_id`).
        //
        // The major ID uniquely identifies the thread that the `Uid` came from
        // (e.g. if `major_id == 6`, the `Uid` was generated by the 6th thread
        // to generate a `Uid`).
        //
        // The minor ID uniquely identifies a `Uid` relative to its thread (e.g.
        // if `major_id == 6` and `minor_id == 7`, the `Uid` is the 7th `Uid`
        // to be generated by the 6th thread to generate a `Uid`).
        //
        // Doing it like this means that a thread generating a `Uid` only needs
        // to synchronize with other threads for the first `Uid` it generates
        // (fetching and incrementing the global `NEXT_THREAD_ID` value). It can
        // then use a thread local cache to store its major ID (`MAJOR_ID`) and
        // a thread local counter for the minor ID (`NEXT_MINOR_ID`) so that it
        // never has to synchronize again.
        Uid {
            major_id: MAJOR_ID.with(|thread_id| {
                *thread_id.get_or_init(|| NEXT_MAJOR_ID.fetch_add(1, Ordering::SeqCst))
            }),

            minor_id: NEXT_MINOR_ID.with_borrow_mut(|next_frame_id| {
                let frame_id = *next_frame_id;
                *next_frame_id += 1;
                frame_id
            }),
        }
    }
}

thread_local! {
    /// See [Uid::new].
    static MAJOR_ID: OnceCell<usize> = const { OnceCell::new() };
    /// See [Uid::new].
    static NEXT_MINOR_ID: RefCell<usize> = const { RefCell::new(0) };
}
/// See [Uid::new].
static NEXT_MAJOR_ID: AtomicUsize = const { AtomicUsize::new(0) };

const fn greatest_common_divisor(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (b, a) = (a % b, b)
    }
    a
}
