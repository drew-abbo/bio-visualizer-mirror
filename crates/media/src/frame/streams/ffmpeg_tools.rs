//! This module contains some useful tools for [super::FrameStream]s based on
//! FFmpeg.

#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicBool, Ordering};

use ctor::ctor;

use ffmpeg::format::Pixel as FFmpegPixelFormat;
use ffmpeg::frame::Video as FFmpegVideoFrame;
use ffmpeg::software::scaling::Context as FFmpegScalingContext;
use ffmpeg::software::scaling::flag::Flags as FFmpegScalingFlags;
use ffmpeg_next as ffmpeg;

use util::cast_slice;

use super::IntoStreamError;
use crate::frame::{Dimensions, FrameBuffer, Pixel};

/// A [FrameBuffer] based on an FFmpeg video frame.
pub struct VideoFrameBuffer(FFmpegVideoFrame);

impl VideoFrameBuffer {
    /// Create an empty [VideoFrameBuffer] with only the buffer's dimensions.
    pub fn new(dimensions: Dimensions) -> Result<Self, VideoStreamError> {
        Self::from_ffmpeg_video_frame(FFmpegVideoFrame::new(
            FFmpegPixelFormat::RGBA,
            dimensions.width(),
            dimensions.height(),
        ))
    }

    /// Create from the underlying [FFmpegVideoFrame] ([ffmpeg::frame::Video]).
    pub fn from_ffmpeg_video_frame(frame: FFmpegVideoFrame) -> Result<Self, VideoStreamError> {
        if frame.format() != FFmpegPixelFormat::RGBA {
            return Err(VideoStreamError::NotRGBA);
        }

        let frame_area = (frame.width() * frame.height()) as usize;
        if frame_area == 0 {
            return Err(VideoStreamError::ZeroLengthSide(
                frame.width(),
                frame.height(),
            ));
        }

        let expected_buffer_len = frame_area * size_of::<Pixel>();
        if frame.data(0).len() != expected_buffer_len {
            return Err(VideoStreamError::WrongBufferLen {
                expected: expected_buffer_len,
                actual: frame.data(0).len(),
            });
        }

        let buffer_alignment = frame.data(0).as_ptr() as usize % align_of::<Pixel>();
        if buffer_alignment != 0 {
            return Err(VideoStreamError::BadBufferAlignment(buffer_alignment));
        }

        Ok(Self(frame))
    }

    /// Convert to the underlying [FFmpegVideoFrame] ([ffmpeg::frame::Video]).
    pub fn into_ffmpeg_buffer(self) -> FFmpegVideoFrame {
        self.0
    }
}

impl FrameBuffer for VideoFrameBuffer {
    fn dimensions(&self) -> crate::frame::Dimensions {
        // We know the dimensions are non-zero because we provided them
        // ourselves in the constructor (so this can't panic).
        (self.0.width(), self.0.height()).into()
    }

    fn pixels_mut(&mut self) -> &mut [Pixel] {
        // SAFETY: We checked in the constructor that this data is the right
        // size and alignment to be casted to a `Pixel`. Those are of the only
        // requirements we need to hit since `Pixel` and `u8` are both "plain
        // old data".
        unsafe { cast_slice::cast_slice_mut(self.0.data_mut(0)) }
    }
}

/// Used to normalize [FFmpegVideoFrame]s ([ffmpeg::frame::Video]) to use the
/// RGBA format.
pub struct VideoFrameFormatter(Option<(FFmpegScalingContext, FFmpegVideoFrame)>);

impl VideoFrameFormatter {
    /// Create a new formatter.
    pub fn new(
        format: FFmpegPixelFormat,
        dimensions: Dimensions,
    ) -> Result<Self, VideoStreamError> {
        if format == FFmpegPixelFormat::RGBA {
            return Ok(Self(None));
        }

        let scaler = FFmpegScalingContext::get(
            // Src. format:
            format,
            dimensions.width(),
            dimensions.height(),
            // Dest. format:
            FFmpegPixelFormat::RGBA,
            dimensions.width(),
            dimensions.height(),
            // Extra options (we don't need any special behavior).
            FFmpegScalingFlags::empty(),
        )
        .map_err(|_| VideoStreamError::ScalerCreateFailure)?;

        Ok(Self(Some((scaler, FFmpegVideoFrame::empty()))))
    }

    /// If this formatter's format is not RGBA, apply the `f` operation to an
    /// intermediate buffer, then copy that buffer to an RGBA buffer `buffer`
    /// and return it. If this formatter's format is RGBA, just apply `f` to
    /// `buffer` and return it.
    ///
    /// `f` should return a [bool] for whether anything was written to the
    /// buffer. This value is returned from the outer function.
    ///
    /// `buffer` must be RGBA and have the the same dimensions as the formatter.
    pub fn format<F>(
        &mut self,
        buffer: &mut FFmpegVideoFrame,
        f: F,
    ) -> Result<bool, VideoStreamError>
    where
        F: FnOnce(&mut FFmpegVideoFrame) -> Result<bool, VideoStreamError>,
    {
        assert_eq!(
            buffer.format(),
            FFmpegPixelFormat::RGBA,
            "Input buffer must be in RGBA format."
        );

        let frame_written = if let Some((scaler, intermediate_buffer)) = self.0.as_mut() {
            let frame_written = f(intermediate_buffer)?;

            if frame_written {
                scaler
                    .run(intermediate_buffer, buffer)
                    .map_err(|_| VideoStreamError::ScaleFailure)?;
            }

            frame_written
        } else {
            f(buffer)?
        };

        Ok(frame_written)
    }
}

/// SAFETY: The [ffmpeg::software::scaling::Context] type (aliased
/// [FFmpegScalingContext] here) which we're storing in our
/// [VideoFrameFormatter] struct *is* safe to send between threads. The FFmpeg
/// library authors likely just didn't think to mark it [Send]. I opened an
/// issue about it here:
/// https://github.com/zmwangx/rust-ffmpeg/issues/252
unsafe impl Send for VideoFrameFormatter {}

/// Indicates that something went wrong creating a [VideoFrameBuffer].
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoStreamError {
    #[error("The video stream should have an RGBA pixel format.")]
    NotRGBA,
    #[error(
        "The video stream shouldn't have dimensions with a 0-length side \
        ({0}x{1} has no area)."
    )]
    ZeroLengthSide(u32, u32),
    #[error(
        "The video stream's frame data should be {expected} bytes long \
        but is actually {actual} bytes long."
    )]
    WrongBufferLen { expected: usize, actual: usize },
    #[error(
        "The video stream's frame data isn't aligned to fit 4-byte pixels \
        (alignment = {0:x}, should be 0)."
    )]
    BadBufferAlignment(usize),
    #[error(
        "The video stream's frame dimensions cannot change \
        (Expected {expected} but got {actual})."
    )]
    DimensionsChanged {
        expected: Dimensions,
        actual: Dimensions,
    },
    #[error("The video stream has no frame rate.")]
    NoFrameRate,
    #[error("Failed to get an input's context.")]
    NoInputContext,
    #[error("Failed to find an ideal video stream.")]
    NoBestVideoStream,
    #[error("Failed to create a decoder.")]
    DecoderCreateFailure,
    #[error("Failed to create a frame scaler.")]
    ScalerCreateFailure,
    #[error("Failed to scale (reformat) a frame.")]
    ScaleFailure,
    #[error("Failed to decode a frame.")]
    DecodeFailure,
}

impl IntoStreamError for VideoStreamError {}

impl IntoStreamError for ffmpeg::Error {}

/// Initializes FFmpeg. This happens when the [crate] is loaded.
///
/// You should never actually call this function.
#[ctor]
fn ffmpeg_init() {
    #[cfg(debug_assertions)]
    {
        static ALREADY_INIT: AtomicBool = AtomicBool::new(false);
        assert!(
            !ALREADY_INIT.swap(true, Ordering::SeqCst),
            "Tried to initialize FFmpeg twice. \
            THIS WOULD NOT HAVE BEEN CAUGHT IN A RELEASE BUILD."
        );
    }

    ffmpeg::init().expect("FFmpeg shouldn't fail to initialize.");
}
