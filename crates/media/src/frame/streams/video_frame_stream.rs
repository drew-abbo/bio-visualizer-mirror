//! Exports [VideoFrameStream].

use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::ops::{Bound, RangeBounds, RangeInclusive};
use std::path::Path;
use std::time::Duration;

use util::channels::ChannelError;
use util::channels::message_channel::{self, Inbox, Outbox};
use util::channels::request_channel::{self, Client, Request, Server};
use util::drop_join_thread::{self, DropJoinHandle};

use super::{FrameStream, FrameStreamError};
use crate::ffmpeg_tools::ffmpeg_video::{FFmpegVideo, FFmpegVideoFrame};
use crate::fps::{self, Fps, Resampler};
use crate::frame::{Dimensions, Frame, RescaleMethod};
use crate::playback_stream::{BufferingSuggestor, PlaybackStream, SeekablePlaybackStream};

/// A builder for creating [VideoFrameStream]s. See [VideoFrameStream::builder].
#[derive(Debug, Clone, Copy)]
pub struct VideoFrameStreamBuilder<'a> {
    video_file_path: &'a Path,
    target_fps: Option<Fps>,
    paused: bool,
    clip: (usize, usize),
    playhead: usize,
    will_loop: bool,
    playback_speed: Fps,
    rescale: Option<(Dimensions, RescaleMethod)>,
    fetch_timeout: Option<Duration>,
}

impl<'a> VideoFrameStreamBuilder<'a> {
    /// Set the path of the video file to load.
    #[must_use]
    #[inline(always)]
    pub fn video_file_path(
        mut self,
        video_file_path: &'a impl AsRef<Path>,
    ) -> VideoFrameStreamBuilder<'a> {
        self.video_file_path = video_file_path.as_ref();
        self
    }

    /// A const version of [Self::video_file_path].
    #[must_use]
    #[inline(always)]
    pub const fn video_file_path_const(
        mut self,
        video_file_path: &'a Path,
    ) -> VideoFrameStreamBuilder<'a> {
        self.video_file_path = video_file_path;
        self
    }

    /// Set the target frame rate (the stream will be resampled to this [Fps]).
    /// If unset the stream will have the video's native frame rate.
    #[must_use]
    #[inline(always)]
    pub const fn fps(mut self, target_fps: Fps) -> Self {
        self.target_fps = Some(target_fps);
        self
    }

    /// Set whether or not the stream starts paused. The default is `false`.
    #[must_use]
    #[inline(always)]
    pub const fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    /// Set the clip's playback range. The default is `..` (the entire stream
    /// will be played). The clip range provided here will be clamped to be
    /// within the stream's duration and at least 1 in length.
    ///
    /// See [SeekablePlaybackStream::clip] and
    /// [SeekablePlaybackStream::set_clip].
    #[must_use]
    #[inline(always)]
    pub fn clip(mut self, playback_range: impl RangeBounds<usize>) -> Self {
        let start = match playback_range.start_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => (*n).checked_add(1).unwrap_or(usize::MAX),
            Bound::Unbounded => 0,
        };
        let end = match playback_range.end_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => (*n).checked_sub(1).unwrap_or(0),
            Bound::Unbounded => usize::MAX,
        };

        self.clip = (start, end);
        self
    }

    /// A const version of [Self::clip].
    #[must_use]
    #[inline(always)]
    pub const fn clip_const(mut self, playback_range: RangeInclusive<usize>) -> Self {
        self.clip = (*playback_range.start(), *playback_range.end());
        self
    }

    /// Set the starting position of the playhead. The default is 0. The
    /// `playhead` provided here will be clamped to be within the stream's
    /// [clip](Self::clip).
    ///
    /// See [SeekablePlaybackStream::playhead] and
    /// [SeekablePlaybackStream::seek_playhead].
    #[must_use]
    #[inline(always)]
    pub const fn playhead(mut self, playhead: usize) -> Self {
        self.playhead = playhead;
        self
    }

    /// Set whether or not the stream should loop. The default is `false`.
    ///
    /// See [SeekablePlaybackStream::will_loop] and
    /// [SeekablePlaybackStream::set_loop].
    #[must_use]
    #[inline(always)]
    pub const fn set_loop(mut self, will_loop: bool) -> Self {
        self.will_loop = will_loop;
        self
    }

    /// Set the multipler that changes the playback speed.
    #[must_use]
    #[inline(always)]
    pub const fn playback_speed(mut self, multipler: Fps) -> Self {
        self.playback_speed = multipler;
        self
    }

    /// Set the multipler that changes the playback speed with a floating point
    /// number.
    ///
    /// An error can be returned if the float fails to approximate a positive
    /// rational (see [Fps::from_float_raw]). In this case, the returned builder
    /// will not have had its playback speed set.
    #[must_use]
    #[inline(always)]
    pub const fn playback_speed_float(self, multipler: f64) -> Result<Self, Self> {
        match Fps::from_float_raw(multipler) {
            Ok(multipler) => Ok(self.playback_speed(multipler)),
            Err(_) => Err(self),
        }
    }

    /// Set the stream to return frames rescaled to these dimensions. No
    /// rescaling will happen if the stream already produces frames with these
    /// dimensions. If unset the stream's frames will have the video's native
    /// dimensions.
    ///
    /// See [FrameStream::dimensions], [FrameStream::set_dimensions], and
    /// [FrameStream::native_dimensions].
    #[must_use]
    #[inline(always)]
    pub const fn rescale(mut self, dimensions: Dimensions, rescale_method: RescaleMethod) -> Self {
        self.rescale = Some((dimensions, rescale_method));
        self
    }

    /// How long to wait before giving up when fetching a frame.
    ///
    /// See [VideoFrameStream::fetch_timeout] and
    /// [VideoFrameStream::fetch_timeout_mut].
    #[must_use]
    #[inline(always)]
    pub const fn fetch_timeout(mut self, timeout: Duration) -> Self {
        self.fetch_timeout = Some(timeout);
        self
    }

    /// Create a [VideoFrameStream].
    #[inline(always)]
    pub fn build(self) -> Request<Result<VideoFrameStream, FrameStreamError>> {
        VideoFrameStream::from_builder(self)
    }

    // This function should remain private. Construction should be done with
    // `VideoFrameStream::builder`.
    #[inline(always)]
    fn new(video_file_path: &'a Path) -> Self {
        Self {
            video_file_path,
            target_fps: None,
            paused: false,
            clip: (0, usize::MAX),
            playhead: 0,
            will_loop: false,
            playback_speed: fps::consts::FPS_1,
            rescale: None,
            fetch_timeout: None,
        }
    }
}

/// A [FrameStream] of frames from a video file.
#[derive(Debug)]
pub struct VideoFrameStream {
    // Worker Communication:
    frame_inbox: Inbox<Result<(Frame, PlaybackState), FrameStreamError>>,
    worker_client: Client<WorkerRequest, ()>,

    // Shared State:
    target_fps: Fps,
    paused: bool,
    dimensions: Dimensions,
    rescale_method: RescaleMethod,
    clip: RangeInclusive<usize>,
    playhead: usize,
    will_loop: bool,
    playback_speed: Fps,

    // Src Info (Final):
    native_dimensions: Dimensions,
    native_fps: Fps,
    unclipped_duration: NonZeroUsize,

    // Local State:
    fetch_timeout: Option<Duration>,

    // Keep this field last. Channels must be dropped before joining thread.
    _worker: DropJoinHandle<()>,
}

impl VideoFrameStream {
    /// Get a [builder](VideoFrameStreamBuilder) for creating a
    /// [VideoFrameStream].
    #[inline(always)]
    pub fn builder(video_file_path: &impl AsRef<Path>) -> VideoFrameStreamBuilder<'_> {
        VideoFrameStreamBuilder::new(video_file_path.as_ref())
    }

    /// The dimensions of frames this stream can produce without rescaling.
    pub fn native_dimensions(&self) -> Dimensions {
        self.native_dimensions
    }

    /// The frame rate this stream can run at without resampling.
    pub fn native_fps(&self) -> Fps {
        self.native_fps
    }

    /// How long we'll wait before giving up on fetching a frame. [None] means
    /// we'll wait forever.
    pub fn fetch_timeout(&self) -> Option<Duration> {
        self.fetch_timeout
    }

    /// A *mutable* reference to how long we'll wait before giving up on
    /// fetching a frame. [None] means we'll wait forever.
    pub fn fetch_timeout_mut(&mut self) -> &mut Option<Duration> {
        &mut self.fetch_timeout
    }

    // This function should remain private. Construction should be done with
    // `VideoFrameStreamBuilder::build`.
    fn from_builder(builder: VideoFrameStreamBuilder) -> Request<Result<Self, FrameStreamError>> {
        let VideoFrameStreamBuilder {
            video_file_path,
            target_fps,
            paused,
            clip,
            playhead,
            will_loop,
            playback_speed,
            rescale,
            fetch_timeout,
        } = builder;

        FFmpegVideo::new_mapped(
            video_file_path,
            rescale,
            paused,
            move |ffmpeg_video| -> Result<Self, FrameStreamError> {
                let ffmpeg_video = ffmpeg_video?;

                let clip = fix_clip(clip.0..=clip.1, ffmpeg_video.duration_non_zero());

                let target_fps = target_fps.unwrap_or(ffmpeg_video.src_fps());
                let (dimensions, rescale_method) =
                    rescale.unwrap_or((ffmpeg_video.src_dimensions(), RescaleMethod::default()));
                let native_dimensions = ffmpeg_video.src_dimensions();
                let native_fps = ffmpeg_video.src_fps();
                let unclipped_duration = ffmpeg_video.duration_non_zero();

                let (frame_inbox, frame_outbox) = message_channel::new();
                let (worker_server, worker_client) = request_channel::new();

                let worker = drop_join_thread::spawn(move || {
                    _ = Worker::new(WorkerSetup {
                        ffmpeg_video,
                        frame_outbox,
                        worker_server,
                        target_fps,
                        paused,
                        dimensions,
                        rescale_method,
                        clip,
                        will_loop,
                        playback_speed,
                    })
                    .run();
                });

                let mut ret = Self {
                    frame_inbox,
                    worker_client,
                    target_fps,
                    paused,
                    dimensions,
                    rescale_method,
                    clip: 0..=(unclipped_duration.get() - 1),
                    playhead: 0,
                    will_loop,
                    playback_speed,
                    fetch_timeout,
                    native_dimensions,
                    native_fps,
                    unclipped_duration,
                    _worker: worker,
                };

                ret.seek_playhead(playhead)?;

                ret.worker_client
                    .alert(WorkerRequest::StartSignal)
                    .expect(EXPECT_WORKER);

                Ok(ret)
            },
        )
    }
}

impl PlaybackStream<Frame, FrameStreamError> for VideoFrameStream {
    fn fetch(&mut self) -> Result<Frame, FrameStreamError> {
        let (frame, new_state) = match self.fetch_timeout {
            Some(timeout) => match self.frame_inbox.wait_timeout(timeout) {
                Err(e) if e.is_timeout_error() => return Err(e.into()),
                wait_result => wait_result,
            },

            None => self.frame_inbox.wait(),
        }
        .expect(EXPECT_WORKER)?;

        self.playhead = new_state.playhead;
        self.paused = new_state.paused;
        Ok(frame)
    }

    fn set_target_fps(&mut self, new_target_fps: Fps) {
        if new_target_fps == self.target_fps {
            return;
        }

        self.worker_client
            .request(WorkerRequest::SetTargetFps(new_target_fps))
            .expect(EXPECT_WORKER)
            .wait() // Wait for queue to be fixed.
            .expect(EXPECT_WORKER);

        self.target_fps = new_target_fps;
    }

    fn target_fps(&self) -> Fps {
        self.target_fps
    }

    fn set_paused(&mut self, new_paused: bool) -> bool {
        if new_paused == self.paused {
            return new_paused;
        }

        // We can't unpause if we're at the end of the video and not looping.
        if self.playhead == *self.clip.end() && !self.will_loop {
            return true;
        }

        self.worker_client
            .request(WorkerRequest::SetPaused(new_paused))
            .expect(EXPECT_WORKER)
            .wait() // Wait for queue to be fixed.
            .expect(EXPECT_WORKER);

        new_paused
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn seek_controls(
        &mut self,
    ) -> Option<&mut dyn SeekablePlaybackStream<Frame, FrameStreamError>> {
        Some(self as _)
    }

    fn recycle(&mut self, frame: Frame) {
        self.worker_client
            .alert(WorkerRequest::Recycle(frame))
            .expect(EXPECT_WORKER)
    }
}

impl FrameStream for VideoFrameStream {
    fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    fn set_dimensions(&mut self, new_dimensions: Dimensions, rescale_method: RescaleMethod) {
        if new_dimensions == self.dimensions
            && (rescale_method == self.rescale_method || new_dimensions == self.native_dimensions)
        {
            return;
        }

        self.worker_client
            .request(WorkerRequest::SetDimensions(new_dimensions, rescale_method))
            .expect(EXPECT_WORKER)
            .wait() // Wait for queue to be fixed.
            .expect(EXPECT_WORKER);

        self.dimensions = new_dimensions;
    }

    fn rescale_method(&self) -> Option<RescaleMethod> {
        (self.dimensions != self.native_dimensions).then(|| self.rescale_method)
    }

    fn native_dimensions(&self) -> Dimensions {
        self.native_dimensions
    }
}

impl SeekablePlaybackStream<Frame, FrameStreamError> for VideoFrameStream {
    fn clip(&self) -> RangeInclusive<usize> {
        self.clip.clone()
    }

    fn set_clip(&mut self, playback_range: RangeInclusive<usize>) -> RangeInclusive<usize> {
        // This doesn't ensure the playhead stays within range!
        // Maybe we do that in the fetch function?
        todo!();

        if playback_range == self.clip {
            return playback_range;
        }

        self.worker_client
            .request(WorkerRequest::SetClip(playback_range.clone()))
            .expect(EXPECT_WORKER)
            .wait() // Wait for queue to be fixed.
            .expect(EXPECT_WORKER);

        self.clip = playback_range.clone();
        playback_range
    }

    fn unclipped_stream_duration_non_zero(&self) -> NonZeroUsize {
        self.unclipped_duration
    }

    fn playhead(&self) -> usize {
        self.playhead
    }

    fn seek_playhead(&mut self, new_playhead: usize) -> Result<usize, FrameStreamError> {
        let new_playhead = new_playhead.clamp(*self.clip.start(), *self.clip.end());

        if new_playhead == self.playhead {
            return Ok(new_playhead);
        }

        self.worker_client
            .request(WorkerRequest::SeekPlayhead(new_playhead))
            .expect(EXPECT_WORKER)
            .wait() // Wait for queue to be fixed.
            .expect(EXPECT_WORKER);

        // If we're at the end we're always paused.
        if new_playhead == *self.clip.end() {
            self.paused = true;
        }

        self.playhead = new_playhead;
        Ok(new_playhead)
    }

    fn will_loop(&self) -> bool {
        self.will_loop
    }

    fn set_loop(&mut self, will_loop: bool) {
        if will_loop == self.will_loop {
            return;
        }

        self.worker_client
            .request(WorkerRequest::SetLoop(will_loop))
            .expect(EXPECT_WORKER)
            .wait() // Wait for queue to be fixed.
            .expect(EXPECT_WORKER);

        self.will_loop = will_loop;
    }

    fn playback_speed(&self) -> Fps {
        self.playback_speed
    }

    fn set_playback_speed(&mut self, new_playback_speed: Fps) {
        if new_playback_speed == self.playback_speed {
            return;
        }

        self.worker_client
            .request(WorkerRequest::SetPlaybackSpeed(new_playback_speed))
            .expect(EXPECT_WORKER)
            .wait() // Wait for queue to be fixed.
            .expect(EXPECT_WORKER);

        self.playback_speed = new_playback_speed;
    }
}

const EXPECT_WORKER: &str = "The worker should be connected.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlaybackState {
    pub playhead: usize,
    pub paused: bool,
}

#[derive(Debug)]
enum WorkerRequest {
    StartSignal,
    SetTargetFps(Fps),
    SetPaused(bool),
    Recycle(Frame),
    SetDimensions(Dimensions, RescaleMethod),
    SetClip(RangeInclusive<usize>),
    SeekPlayhead(usize),
    SetLoop(bool),
    SetPlaybackSpeed(Fps),
}

#[derive(Debug)]
struct WorkerSetup {
    pub ffmpeg_video: FFmpegVideo,
    pub frame_outbox: Outbox<Result<(Frame, PlaybackState), FrameStreamError>>,
    pub worker_server: Server<WorkerRequest, ()>,
    pub target_fps: Fps,
    pub paused: bool,
    pub dimensions: Dimensions,
    pub rescale_method: RescaleMethod,
    pub clip: RangeInclusive<usize>,
    pub will_loop: bool,
    pub playback_speed: Fps,
}

#[derive(Debug)]
struct Worker {
    // Client communication:
    frame_outbox: Outbox<Result<(Frame, PlaybackState), FrameStreamError>>,
    worker_server: Server<WorkerRequest, ()>,

    // Frame generation:
    ffmpeg_video: FFmpegVideo,
    buffering_suggestor: BufferingSuggestor,
    buffering_suggestion: usize,
    recycled_frames: Vec<Frame>,
    received_start_signal: bool,

    // Shared state:
    target_fps: Fps,
    dimensions: Dimensions,
    rescale_method: RescaleMethod,
    clip: RangeInclusive<usize>,
    will_loop: bool,
    playback_speed: Fps,
    client_playhead: usize,
    client_paused: bool,

    // Local state:
    fps_resampler: Resampler,
    ahead_of_client_by: usize,
    paused_for: usize,
    frames_since_loop: usize,
}

impl Worker {
    pub fn new(worker_setup: WorkerSetup) -> Self {
        let WorkerSetup {
            ffmpeg_video,
            frame_outbox,
            worker_server,
            target_fps,
            paused,
            dimensions,
            rescale_method,
            clip,
            will_loop,
            playback_speed,
        } = worker_setup;

        let buffering_suggestor = BufferingSuggestor::new(target_fps);
        let buffering_suggestion = buffering_suggestor.buffering_suggestion();

        let recycled_frames = Vec::with_capacity(buffering_suggestion);

        let fps_resampler = Resampler::new(ffmpeg_video.src_fps() * playback_speed, target_fps);

        Self {
            frame_outbox,
            worker_server,
            ffmpeg_video,
            buffering_suggestor,
            buffering_suggestion,
            recycled_frames,
            received_start_signal: false,
            target_fps,
            dimensions,
            rescale_method,
            clip,
            will_loop,
            playback_speed,
            client_playhead: 0,
            client_paused: paused,
            fps_resampler,
            ahead_of_client_by: 0,
            paused_for: 0,
            frames_since_loop: 0,
        }
    }

    pub fn run(mut self) -> Result<Infallible, ChannelError> {
        loop {
            let (elapsed_time, result) = BufferingSuggestor::run_timed(|| {
                self.handle_requests()?;

                let new_frame_and_state = self.next_frame();
                self.frame_outbox
                    .send_bounded(new_frame_and_state, self.buffering_suggestion)?;

                Ok(())
            });
            _ = result?;

            self.buffering_suggestor.add_time_sample(elapsed_time);
            self.buffering_suggestion = self.buffering_suggestor.buffering_suggestion();
        }
    }

    fn handle_requests(&mut self) -> Result<(), ChannelError> {
        todo!()
    }

    fn next_frame(&mut self) -> Result<(Frame, PlaybackState), FrameStreamError> {
        let recycled_frame = loop {
            let Some(frame) = self.recycled_frames.pop() else {
                break None;
            };
            if let Ok(ffmpeg_frame) = frame.into_buffer::<FFmpegVideoFrame>() {
                break Some(ffmpeg_frame);
            }
        };

        // Handle skipping frames. Figure out the next frame we want...
        // This includes looping & pausing.
        // Aren't we able to go 1 past the end of the actual playhead? Fuck...
        todo!();

        let new_frame = self.ffmpeg_video.write_next(recycled_frame)?;

        todo!()
    }

    fn rollback(&mut self) -> Result<(), FrameStreamError> {
        // Rollback `self.ahead_of_client_by` frames. We may be rolling back
        // past loops, past pause/unpause actions, ect...
        todo!()
    }
}

const fn fix_clip(
    clip: RangeInclusive<usize>,
    unclipped_duration: NonZeroUsize,
) -> RangeInclusive<usize> {
    let mut start = *clip.start();
    let mut end = *clip.end();

    if start >= unclipped_duration.get() {
        start = unclipped_duration.get();
    }
    if end >= unclipped_duration.get() {
        end = unclipped_duration.get();
    }

    if start > end {
        end = start;
    }

    start..=end
}
