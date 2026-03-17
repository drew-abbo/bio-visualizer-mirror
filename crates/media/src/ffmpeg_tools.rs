//! Tools for dealing with FFmpeg.

pub mod ffmpeg_video;

mod impls;

use ffmpeg_next as ffmpeg;

pub type FFmpegResult<T> = Result<T, ffmpeg::Error>;

/// Initializes FFmpeg. This happens when the library is loaded.
///
/// You should never actually call this function.
#[ctor::ctor]
fn ffmpeg_init() {
    #[cfg(debug_assertions)]
    {
        use std::sync::atomic::{AtomicBool, Ordering};

        static ALREADY_INIT: AtomicBool = AtomicBool::new(false);
        assert!(
            !ALREADY_INIT.swap(true, Ordering::SeqCst),
            "Tried to initialize FFmpeg twice."
        );
    }

    ffmpeg::init().expect("FFmpeg shouldn't fail to initialize.");
}
