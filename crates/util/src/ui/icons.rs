//! Shared icons for the UI.
//!
//! # Troubleshooting
//!
//! You'll need image loaders to use this. Run the following:
//!
//! ```sh
//! cargo add egui-extras -F image
//! ```
//!
//! Then make sure to install the image loaders when the app starts. More info:
//! <https://users.rust-lang.org/t/egui-include-image-dont-work/129500/2>
//!
//! # Example Usage
//!
//! ```ignore
//! use util::ui::icons;
//!
//! ui.image(icons::trash_64x64());
//! ```

use egui::ImageSource;

/// A trash icon.
pub const fn trash_64x64() -> ImageSource<'static> {
    egui::include_image!("icons/trash_64x64.png")
}

/// A folder trash icon.
pub const fn folder_64x64() -> ImageSource<'static> {
    egui::include_image!("icons/folder_64x64.png")
}
