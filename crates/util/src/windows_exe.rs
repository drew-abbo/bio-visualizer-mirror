//! A *build-time* utility for setting the icon of a binary on windows. On other
//! platforms everything here is a no-op.

use std::error::Error;

/// Used for setting details on a Windows executable at *build-time*. On other
/// platforms everything here is a no-op.
///
/// See [details] to construct.
pub trait Details {
    /// Apply changes.
    fn apply(self) -> Result<(), impl Error>;

    /// Sets the executable's icon. `icon_path` should be the path (relative to
    /// the project root) of a `.ico` file.
    #[must_use]
    fn icon(self, icon_path: impl AsRef<str>) -> Self;

    /// Sets the executable's `ProductName`.
    #[must_use]
    fn name(self, name: impl AsRef<str>) -> Self;
}

/// Create a builder for setting Windows exe details at *build-time*. On other
/// platforms this returns a dummy object where all methods are no-ops.
pub fn details() -> impl Details {
    DetailsImpl::default()
}

#[cfg(windows)]
#[derive(Debug)]
struct DetailsImpl(winresource::WindowsResource);

impl Default for DetailsImpl {
    fn default() -> Self {
        let mut res = winresource::WindowsResource::new();

        res.set_language(0x0409 /* English (US) */);
        res.set("FileVersion", crate::version::APP_VERSION);
        res.set("ProductVersion", crate::version::APP_VERSION);

        Self(res)
    }
}

#[cfg(windows)]
impl Details for DetailsImpl {
    fn apply(self) -> Result<(), impl Error> {
        self.0.compile()
    }

    fn icon(mut self, icon_path: impl AsRef<str>) -> Self {
        let icon_path = icon_path.as_ref();
        self.0.set_icon(icon_path);
        println!("cargo:rerun-if-changed={icon_path}");
        self
    }

    fn name(mut self, name: impl AsRef<str>) -> Self {
        self.0.set("ProductName", name.as_ref());
        self
    }
}

#[cfg(not(windows))]
#[derive(Debug, Default)]
struct BuilderImpl;

#[cfg(not(windows))]
impl Details for DetailsImpl {
    fn new() -> Self {
        Self
    }

    fn apply(self) -> Result<(), impl Error> {
        Result::<(), std::convert::Infallible>::Ok(())
    }

    fn icon(self, _icon_path: impl AsRef<str>) -> Self {
        self
    }

    fn name(self, _name: impl AsRef<str>) -> Self {
        self
    }
}
