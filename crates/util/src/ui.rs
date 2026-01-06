//! Common UI utilities.

pub mod icons;

use std::cell::Cell;
use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::process::Command;

use egui::gui_zoom::{self, kb_shortcuts};
use egui::load::{ImagePoll, LoadError};
use egui::{Context, ImageSource, Key, Modal, Modifiers, RichText, SizeHint, Ui, Vec2};

/// A hacky fix to make scrolling smooth on trackpads w/ Windows. See issue:
/// <https://github.com/emilk/egui/issues/4350>
///
/// This compiles down to a no-op when not on Windows.
#[inline(always)]
pub fn windows_scroll_fix(ctx: &Context) {
    #[cfg(windows)]
    #[inline(always)]
    fn inner(ctx: &Context) {
        let scrolled_recently = ctx.input(|i| i.time_since_last_scroll() < 1.0);
        if scrolled_recently {
            ctx.request_repaint();
        }
    }

    #[cfg(not(windows))]
    #[inline(always)]
    fn inner(_ctx: &Context) {}

    inner(ctx);
}

/// Tries to load an image using [Context::try_load_image]. `true` is returned
/// when the image has been loaded and is ready.
///
/// `img` *must* be an [ImageSource::Bytes] variant or the function will panic.
/// (i.e. `img` should come from something like [egui::include_image]).
pub fn bytes_image_is_loaded<'a>(ctx: &Context, img: ImageSource<'a>) -> Result<bool, LoadError> {
    let ImageSource::Bytes { uri, bytes } = img else {
        panic!("`img` must be an `ImageSource::Bytes` variant.")
    };

    ctx.include_bytes(uri.clone(), bytes);
    Ok(matches!(
        ctx.try_load_image(&uri, SizeHint::default())?,
        ImagePoll::Ready { .. }
    ))
}

/// The same as [bytes_image_is_loaded], but for a series of images.
pub fn bytes_images_are_loaded<'a, const N: usize>(
    ctx: &Context,
    imgs: [ImageSource<'a>; N],
) -> Result<bool, LoadError> {
    let mut ret = true;
    for img in imgs {
        ret = ret && bytes_image_is_loaded(ctx, img)?;
    }
    Ok(ret)
}

/// Draw a header with a dimmed background that takes up the entire width of the
/// window (ignoring the window's inner margin). This is meant to be placed as
/// the first element in a window-like container (e.g. [Modal]).
pub fn window_header(ui: &mut Ui, title: impl Into<String>) {
    let pad = ui.spacing().window_margin;

    let heading_frame = egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::same(8))
        .outer_margin(egui::Margin {
            left: -pad.left,
            right: -pad.right,
            top: -pad.top,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading(title.into());
            });
        });

    // Draw a line along the bottom of the header.
    let heading_frame_rect = heading_frame.response.rect;
    let line_l = heading_frame_rect.left_bottom();
    let line_r = heading_frame_rect.right_bottom();
    ui.painter().line_segment(
        [
            (line_l.x - pad.left as f32, line_l.y).into(),
            (line_r.x + pad.right as f32, line_r.y).into(),
        ],
        ui.visuals().window_stroke(),
    );
}

/// Opens a popup window with a header, dimming and blocking interaction for the
/// rest of the UI below (see [Modal]).
///
/// `title` should be globally unique.
pub fn popup_window<T, F, R>(ctx: &Context, title: T, add_contents: F)
where
    T: Into<String>,
    F: FnOnce(&mut Ui) -> R,
{
    let title = title.into();
    let viewport_size = ctx.viewport_rect().size();

    // We're making the modal's ID depend on the viewport's size so that the
    // modal's area gets re-computed when the viewport's size changes (e.g. from
    // window resizing or zoom changes). If we don't do this, the popup can get
    // cut off by the edges of the window.
    //
    // This does unfortunately cause some flickering when resizing since egui
    // doesn't draw popups immediately. I couldn't find a nice solution to this
    // unfortunately.
    let popup_id = format!("popup='{title}' viewport={viewport_size}").into();

    Modal::new(popup_id).show(ctx, |ui| {
        window_header(ui, title);
        add_contents(ui)
    });
}

/// A simple error popup window system that uses [Modal].
///
/// # Example Implementation
///
/// To implement this trait, your type will need to store a queue of error
/// messages (usually strings).
///
/// ```
/// use std::collections::VecDeque;
///
/// use eframe::{App, Frame};
///
/// use egui::Context;
///
/// use util::ui::ErrorPopup;
///
/// struct MyApp {
///     // -- snip --
///     error_popup_messages: VecDeque<String>,
/// }
///
/// impl ErrorPopup<String> for MyApp {
///     fn error_queue_mut(&mut self) -> &mut VecDeque<String> {
///         &mut self.error_popup_messages
///     }
/// }
///
/// impl eframe::App for MyApp {
///     fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
///         // -- snip --
///
///         // Call `self.error_popup_messages.push_back()` with an error
///         // message if anything went wrong.
///
///         self.show_any_error_popups(ctx);
///     }
/// }
/// ```
pub trait ErrorPopup<S>
where
    String: for<'a> From<&'a S>,
{
    /// A queue of errors to display to the user.
    fn error_queue_mut(&mut self) -> &mut VecDeque<S>;

    /// If [ErrorPopup::error_queue_mut] returns a non-empty queue, a popup
    /// window that informs the user of an error is shown until they acknowledge
    /// it (see [popup_window]).
    fn show_any_error_popups(&mut self, ctx: &Context) {
        let error_queue = self.error_queue_mut();

        let Some(error_msg) = error_queue.front() else {
            return;
        };
        let error_msg: String = error_msg.into();

        popup_window(ctx, "⚠", |ui| {
            ui.vertical_centered(|ui| {
                ui.label(error_msg);

                ui.add_space(10.0);
                let ok_button = ui.button("Ok");

                ui.add_space(5.0);
                let errors_remaining = error_queue.len() - 1;
                ui.label(
                    RichText::new(format!(
                        "{errors_remaining} additional error{} remaining",
                        if errors_remaining != 1 { "s" } else { "" }
                    ))
                    .small()
                    .weak(),
                );

                if ok_button.clicked() {
                    error_queue.pop_front();
                }
            })
        });
    }
}

/// Handles zoom in/out shortcuts (which are enabled by default in [egui], this
/// just clamps the amount you can actually zoom in/out by). Whether the zoom
/// changed is returned.
///
/// Note that the zoom will not be taken into account until the next draw.
///
/// # Example
///
/// ```ignore
/// if util::ui::handle_zoom_shortcuts(ctx, 0.5, 2.0) {
///     println!("Zoom changed.");
/// }
/// ```
pub fn handle_zoom_shortcuts(ctx: &Context, min: f32, max: f32) -> bool {
    let zoom_in_requested = ctx.input_mut(|i| {
        i.consume_shortcut(&kb_shortcuts::ZOOM_IN)
            | i.consume_shortcut(&kb_shortcuts::ZOOM_IN_SECONDARY)
    });
    if zoom_in_requested {
        let zoom_factor_in_range = ctx.zoom_factor() < max;
        if zoom_factor_in_range {
            gui_zoom::zoom_in(ctx);
        }
        return zoom_factor_in_range;
    }

    let zoom_out_requested = ctx.input_mut(|i| i.consume_shortcut(&kb_shortcuts::ZOOM_OUT));
    if zoom_out_requested && ctx.zoom_factor() > min {
        gui_zoom::zoom_out(ctx);
        return true;
    }
    false
}

/// Use to determine whether viewport size (derived from
/// [Context::viewport_rect]) has changed since this function was last called.
/// [None] is returned if the size hasn't changed. [Some] is returned with the
/// last size of the size has changed.
pub fn viewport_size_changed(ctx: &Context) -> Option<Vec2> {
    thread_local! {
        static VIEWPORT_SIZE: Cell<Vec2> = const { Cell::new(Vec2::INFINITY) };
    }

    let curr_viewport_size = ctx.viewport_rect().size();

    VIEWPORT_SIZE.with(|viewport_size| {
        let last_viewport_size = viewport_size.get();
        if last_viewport_size != curr_viewport_size {
            viewport_size.set(curr_viewport_size);
            Some(last_viewport_size)
        } else {
            None
        }
    })
}

/// A more readable helper for determining if a key ([Key]) was pressed.
pub fn key_pressed(ctx: &Context, key: Key) -> bool {
    shortcut_pressed(ctx, Modifiers::NONE, key)
}

/// A more readable helper for determining if a shortcut ([Modifiers] + [Key])
/// was pressed.
pub fn shortcut_pressed(ctx: &Context, mods: Modifiers, key: Key) -> bool {
    ctx.input_mut(|i| i.consume_key(mods, key))
}

/// Opens a folder with the OS's file explorer.
pub fn open_folder_in_file_explorer(path: &Path) -> Result<(), io::Error> {
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        compile_error!("Unsupported target OS.");
    }

    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Path isn't a directory: {}", path.display()),
        ));
    }

    crate::debug_log_info!("Opening file explorer: `{}`", path.display());

    if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else if cfg!(target_os = "linux") {
        // `xdg-open` is standard on most Linux distros
        Command::new("xdg-open").arg(path).spawn()
    } else {
        unreachable!()
    }
    .map(|_| ())
}
