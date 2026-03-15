//! Exports [VideoFrameStream].

mod resampled_ffmpeg_video;

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::ops::{Bound, RangeBounds, RangeInclusive};
use std::path::Path;
use std::time::Duration;

use util::channels::message_channel::{self, Inbox};
use util::channels::request_channel::{self, Client, Request};
use util::drop_join_thread::{self, DropJoinHandle};

use super::{FrameStream, FrameStreamError, StreamGenerator};
use crate::ffmpeg_tools::FFmpegResult;
use crate::ffmpeg_tools::ffmpeg_video::{self, FFmpegVideo, FFmpegVideoFrame};
use crate::fps::{self, Fps};
use crate::frame::{Dimensions, Frame, RescaleMethod};
use crate::playback_stream::{PlaybackStream, SeekablePlaybackStream};
use resampled_ffmpeg_video::ResampledFFmpegVideo;

/// A builder for creating [VideoFrameStream]s. See [VideoFrameStream::builder].
#[derive(Debug, Clone, Copy)]
pub struct VideoFrameStreamBuilder {
    target_fps: Option<Fps>,
    paused: bool,
    clip: Clip,
    playhead: usize,
    will_loop: bool,
    playback_speed: Fps,
    rescale: Option<(Dimensions, RescaleMethod)>,
    fetch_timeout: Option<Duration>,
}

impl VideoFrameStreamBuilder {
    /// Set the target frame rate (the stream will be resampled to this [Fps]).
    /// If unset the stream will have the video's native frame rate.
    #[must_use = "Builder methods take `Self` by value."]
    #[inline(always)]
    pub const fn fps(mut self, target_fps: Fps) -> Self {
        self.target_fps = Some(target_fps);
        self
    }

    /// Set whether or not the stream starts paused. The default is `false`.
    #[must_use = "Builder methods take `Self` by value."]
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
    #[must_use = "Builder methods take `Self` by value."]
    #[inline(always)]
    pub fn clip(mut self, playback_range: impl RangeBounds<usize>) -> Self {
        self.clip = Clip::from_range(playback_range);
        self
    }

    /// A const version of [Self::clip].
    #[must_use = "Builder methods take `Self` by value."]
    #[inline(always)]
    pub const fn clip_const(mut self, playback_range: RangeInclusive<usize>) -> Self {
        self.clip = Clip::new(playback_range);
        self
    }

    /// Set the starting position of the playhead. The default is 0. The
    /// `playhead` provided here will be clamped to be within the stream's
    /// [clip](Self::clip).
    ///
    /// See [SeekablePlaybackStream::playhead] and
    /// [SeekablePlaybackStream::seek_playhead].
    #[must_use = "Builder methods take `Self` by value."]
    #[inline(always)]
    pub const fn playhead(mut self, playhead: usize) -> Self {
        self.playhead = playhead;
        self
    }

    /// Set whether or not the stream should loop. The default is `false`.
    ///
    /// See [SeekablePlaybackStream::will_loop] and
    /// [SeekablePlaybackStream::set_loop].
    #[must_use = "Builder methods take `Self` by value."]
    #[inline(always)]
    pub const fn set_loop(mut self, will_loop: bool) -> Self {
        self.will_loop = will_loop;
        self
    }

    /// Set the multipler that changes the playback speed.
    #[must_use = "Builder methods take `Self` by value."]
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
    #[must_use = "Builder methods take `Self` by value."]
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
    #[must_use = "Builder methods take `Self` by value."]
    #[inline(always)]
    pub const fn rescale(mut self, dimensions: Dimensions, rescale_method: RescaleMethod) -> Self {
        self.rescale = Some((dimensions, rescale_method));
        self
    }

    /// How long to wait before giving up when fetching a frame.
    ///
    /// See [VideoFrameStream::fetch_timeout] and
    /// [VideoFrameStream::fetch_timeout_mut].
    #[must_use = "Builder methods take `Self` by value."]
    #[inline(always)]
    pub const fn fetch_timeout(mut self, timeout: Duration) -> Self {
        self.fetch_timeout = Some(timeout);
        self
    }

    /// Create a [VideoFrameStream].
    #[inline(always)]
    pub fn build(
        self,
        video_file_path: &impl AsRef<Path>,
    ) -> Request<Result<VideoFrameStream, FrameStreamError>> {
        VideoFrameStream::from_builder(self, video_file_path.as_ref())
    }

    // This function should remain private. Construction should be done with
    // `VideoFrameStream::builder`.
    #[inline(always)]
    const fn new() -> Self {
        Self {
            target_fps: None,
            paused: false,
            clip: Clip::new(0..=usize::MAX),
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
    worker_client: Client<WorkerRequest, PlaybackState>,

    // Shared State:
    target_fps: Fps,
    paused: bool,
    dimensions: Dimensions,
    rescale_method: RescaleMethod,
    clip: Clip,
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
    pub fn builder() -> VideoFrameStreamBuilder {
        VideoFrameStreamBuilder::new()
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

    /// Set how long we'll wait before giving up on fetching a frame. [None]
    /// means we'll wait forever.
    pub fn set_fetch_timeout(&mut self, new_fetch_timeout: Option<Duration>) {
        self.fetch_timeout = new_fetch_timeout;
    }

    // This function should remain private. Construction should be done with
    // `VideoFrameStreamBuilder::build`.
    fn from_builder(
        builder: VideoFrameStreamBuilder,
        video_file_path: &Path,
    ) -> Request<Result<Self, FrameStreamError>> {
        FFmpegVideo::new_mapped(
            video_file_path,
            builder.rescale,
            builder.paused,
            move |ffmpeg_video| -> Result<Self, FrameStreamError> {
                let ffmpeg_video = ResampledFFmpegVideo::new(ffmpeg_video?, builder);

                let (frame_inbox, frame_outbox) = message_channel::new();
                let (worker_server, worker_client) = request_channel::new();

                Ok(Self {
                    frame_inbox,
                    worker_client,
                    target_fps: ffmpeg_video.target_fps(),
                    paused: ffmpeg_video.paused(),
                    dimensions: ffmpeg_video.dest_dimensions(),
                    rescale_method: ffmpeg_video.rescale_method().unwrap_or_default(),
                    clip: ffmpeg_video.clip(),
                    playhead: ffmpeg_video.playhead(),
                    will_loop: ffmpeg_video.will_loop(),
                    playback_speed: ffmpeg_video.playback_speed(),
                    native_dimensions: ffmpeg_video.src_dimensions(),
                    native_fps: ffmpeg_video.src_fps(),
                    unclipped_duration: ffmpeg_video.resampled_duration_non_zero(),
                    fetch_timeout: builder.fetch_timeout,

                    _worker: drop_join_thread::spawn(move || {
                        Worker::new(ffmpeg_video).run(frame_outbox, worker_server);
                    }),
                })
            },
        )
    }

    fn apply_state(&mut self, new_state: PlaybackState) {
        self.playhead = new_state.playhead;
        self.paused = new_state.paused;
    }

    fn worker_alert(&self, msg: WorkerRequest) {
        self.worker_client.alert(msg).expect(EXPECT_WORKER);
    }

    #[must_use]
    fn worker_request_and_wait(&self, msg: WorkerRequest) -> PlaybackState {
        let mut req = self.worker_client.request(msg).expect(EXPECT_WORKER);

        // Interrupt worker if it's waiting for us to pull from the queue.
        self.frame_inbox.block_sender().expect(EXPECT_WORKER);

        // Wait for the queue to be fixed.
        let ret = req.wait().expect(EXPECT_WORKER);

        self.frame_inbox.unblock_sender().expect(EXPECT_WORKER);

        ret
    }
}

impl PlaybackStream<Frame, FrameStreamError> for VideoFrameStream {
    fn fetch(&mut self) -> Result<Frame, FrameStreamError> {
        let (frame, new_state) = match self.fetch_timeout {
            Some(timeout) => match self.frame_inbox.wait_timeout(timeout) {
                Err(e) if e.is_any_timeout_error() => return Err(e.into()),
                wait_result => wait_result,
            },

            None => self.frame_inbox.wait(),
        }
        .expect(EXPECT_WORKER)?;

        self.apply_state(new_state);
        Ok(frame)
    }

    fn set_target_fps(&mut self, new_target_fps: Fps) {
        if new_target_fps == self.target_fps {
            return;
        }

        let new_state = self.worker_request_and_wait(WorkerRequest::SetTargetFps(new_target_fps));
        self.target_fps = new_target_fps;
        self.apply_state(new_state);
    }

    fn target_fps(&self) -> Fps {
        self.target_fps
    }

    fn set_paused(&mut self, new_paused: bool) -> bool {
        if new_paused == self.paused {
            return new_paused;
        }

        let new_state = self.worker_request_and_wait(WorkerRequest::SetPaused(new_paused));
        self.paused = new_paused;
        self.apply_state(new_state);

        self.paused
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
        self.worker_alert(WorkerRequest::Recycle(Some(frame)));
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

        let new_state = self
            .worker_request_and_wait(WorkerRequest::SetDimensions(new_dimensions, rescale_method));
        self.dimensions = new_dimensions;

        self.apply_state(new_state);
    }

    fn rescale_method(&self) -> Option<RescaleMethod> {
        (self.dimensions != self.native_dimensions).then_some(self.rescale_method)
    }

    fn native_dimensions(&self) -> Dimensions {
        self.native_dimensions
    }
}

impl SeekablePlaybackStream<Frame, FrameStreamError> for VideoFrameStream {
    fn clip(&self) -> RangeInclusive<usize> {
        self.clip.into()
    }

    fn set_clip(&mut self, clip: RangeInclusive<usize>) -> RangeInclusive<usize> {
        let clip = Clip::new(clip).fix(self.unclipped_duration);
        if clip == self.clip {
            return clip.into();
        }

        let new_state = self.worker_request_and_wait(WorkerRequest::SetClip(clip));
        self.clip = clip;
        self.apply_state(new_state);

        clip.into()
    }

    fn unclipped_stream_duration_non_zero(&self) -> NonZeroUsize {
        self.unclipped_duration
    }

    fn playhead(&self) -> usize {
        self.playhead
    }

    fn seek_playhead(&mut self, new_playhead: usize) -> Result<usize, FrameStreamError> {
        let new_playhead = new_playhead.clamp(self.clip.start, self.clip.end);

        if new_playhead == self.playhead {
            return Ok(new_playhead);
        }

        let new_state = self.worker_request_and_wait(WorkerRequest::SeekPlayhead(new_playhead));
        self.playhead = new_playhead;
        self.apply_state(new_state);

        Ok(new_playhead)
    }

    fn will_loop(&self) -> bool {
        self.will_loop
    }

    fn set_loop(&mut self, will_loop: bool) {
        if will_loop == self.will_loop {
            return;
        }

        let new_state = self.worker_request_and_wait(WorkerRequest::SetLoop(will_loop));
        self.will_loop = will_loop;
        self.apply_state(new_state);
    }

    fn playback_speed(&self) -> Fps {
        self.playback_speed
    }

    fn set_playback_speed(&mut self, new_playback_speed: Fps) {
        if new_playback_speed == self.playback_speed {
            return;
        }

        let new_state =
            self.worker_request_and_wait(WorkerRequest::SetPlaybackSpeed(new_playback_speed));
        self.playback_speed = new_playback_speed;
        self.apply_state(new_state);
    }
}

const EXPECT_WORKER: &str = "The worker should be connected.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlaybackState {
    pub playhead: usize,
    pub paused: bool,
}

impl PlaybackState {
    /// Takes a snapshot of the [ResampledFFmpegVideo] and returns its state.
    pub fn snapshot(ffmpeg_video: &ResampledFFmpegVideo) -> Self {
        Self {
            playhead: ffmpeg_video.playhead(),
            paused: ffmpeg_video.paused(),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct Clip {
    pub start: usize,
    pub end: usize,
}

impl Clip {
    #[inline(always)]
    pub const fn new(range: RangeInclusive<usize>) -> Self {
        Self {
            start: *range.start(),
            end: *range.end(),
        }
    }

    #[inline(always)]
    pub fn from_range(range: impl RangeBounds<usize>) -> Self {
        let start = match range.start_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => (*n).saturating_add(1),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => (*n).saturating_sub(1),
            Bound::Unbounded => usize::MAX,
        };

        Self { start, end }
    }

    #[inline(always)]
    pub const fn contains(&self, playhead: usize) -> bool {
        playhead >= self.start && playhead <= self.end
    }

    #[inline(always)]
    pub const fn fix_in_place(&mut self, unclipped_duration: NonZeroUsize) {
        *self = self.fix(unclipped_duration);
    }

    #[must_use]
    pub const fn fix(&self, unclipped_duration: NonZeroUsize) -> Self {
        let unclipped_duration = unclipped_duration.get();
        let mut new_clip = *self;

        if new_clip.start >= unclipped_duration {
            new_clip.start = unclipped_duration - 1;
        }

        if new_clip.start > new_clip.end {
            new_clip.end = new_clip.start;
        } else if new_clip.end >= unclipped_duration {
            new_clip.end = unclipped_duration - 1;
        }

        new_clip
    }
}

impl From<RangeInclusive<usize>> for Clip {
    #[inline(always)]
    fn from(range: RangeInclusive<usize>) -> Self {
        Self::new(range)
    }
}

impl From<Clip> for RangeInclusive<usize> {
    #[inline(always)]
    fn from(clip: Clip) -> Self {
        clip.start..=clip.end
    }
}

#[derive(Debug)]
enum WorkerRequest {
    SetTargetFps(Fps),
    SetPaused(bool),
    Recycle(Option<Frame>),
    SetDimensions(Dimensions, RescaleMethod),
    SetClip(Clip),
    SeekPlayhead(usize),
    SetLoop(bool),
    SetPlaybackSpeed(Fps),
}

struct Worker {
    ffmpeg_video: ResampledFFmpegVideo,
    recycled_frames: Vec<FFmpegVideoFrame>,
    state_history: VecDeque<PlaybackState>,
    frames_to_rescale: VecDeque<FFmpegVideoFrame>,
    rescale_method: RescaleMethod,
    err_state: Option<FrameStreamError>,
}

impl Worker {
    /// Create a new [Worker].
    pub fn new(ffmpeg_video: ResampledFFmpegVideo) -> Self {
        let recycled_frames = Vec::with_capacity(16);

        let mut state_history = VecDeque::with_capacity(16);
        state_history.push_back(PlaybackState::snapshot(&ffmpeg_video));

        let rescale_method = ffmpeg_video.rescale_method().unwrap_or_default();

        Self {
            ffmpeg_video,
            recycled_frames,
            state_history,
            frames_to_rescale: VecDeque::default(),
            rescale_method,
            err_state: None,
        }
    }

    fn write_next_frame(&mut self) -> FFmpegResult<FFmpegVideoFrame> {
        // If we're able to just rescale a frame we've already generated...
        if let Some(frame_to_rescale) = self.frames_to_rescale.pop_front() {
            self.ffmpeg_video.step();
            return ffmpeg_video::rescale_ffmpeg_frame(
                frame_to_rescale,
                self.ffmpeg_video.dest_dimensions(),
                self.rescale_method,
            );
        }

        let recycled_frame = self.recycled_frames.pop();
        self.ffmpeg_video.write_next(recycled_frame)
    }

    fn get_client_state(&self) -> PlaybackState {
        *self
            .state_history
            .front()
            .expect("The oldest state should be present.")
    }
}

impl StreamGenerator for Worker {
    type Data = Result<(Frame, PlaybackState), FrameStreamError>;
    type Request = WorkerRequest;
    type Response = PlaybackState;
    type QueueInvalidNote = ();

    fn target_fps(&self) -> Fps {
        self.ffmpeg_video.target_fps()
    }

    fn new_data(&mut self, in_flight: usize) -> Self::Data {
        if let Some(e) = &self.err_state {
            return Err(e.clone());
        }

        let frame = match self.write_next_frame() {
            Ok(buffer) => Frame::from_buffer(buffer),
            Err(e) => {
                let e = FrameStreamError::from(e);
                self.err_state = Some(e.clone());
                return Err(e);
            }
        };

        while self.state_history.len() > in_flight + 1 {
            self.state_history.pop_back();
        }

        let state = PlaybackState::snapshot(&self.ffmpeg_video);
        self.state_history.push_back(state);

        Ok((frame, state))
    }

    fn handle_request(&mut self, req: &mut Self::Request) -> Option<Self::QueueInvalidNote> {
        if self.err_state.is_some() {
            return None;
        }

        match req {
            WorkerRequest::Recycle(recycled_frame) => {
                let recycled_frame = recycled_frame.take().expect("A frame was sent");
                if let Ok(buffer) = recycled_frame.into_buffer::<FFmpegVideoFrame>() {
                    self.recycled_frames.push(buffer);
                }

                // We don't need to update the queue for this.
                None
            }

            // If we need to update the queue, we'll handle the request in
            // `Self::handle_invalid_queue` when we can actually see the queue.
            WorkerRequest::SetDimensions(_, _) => Some(()),
            WorkerRequest::SetTargetFps(_) => Some(()),
            WorkerRequest::SetPaused(_) => Some(()),
            WorkerRequest::SetClip(_) => Some(()),
            WorkerRequest::SeekPlayhead(_) => Some(()),
            WorkerRequest::SetLoop(_) => Some(()),
            WorkerRequest::SetPlaybackSpeed(_) => Some(()),
        }
    }

    fn handle_invalid_queue(
        &mut self,
        queue: &mut VecDeque<Self::Data>,
        req: &mut Self::Request,
        _queue_invalid_note: Self::QueueInvalidNote,
    ) {
        assert!(queue.len() + 1 == self.state_history.len());
        let client_state = self.get_client_state();

        // Rewind to where the client is.
        self.ffmpeg_video.seek_playhead(client_state.playhead);
        self.ffmpeg_video.set_paused(client_state.paused);

        let mut invalidate_all_items = false;

        match req {
            // We shouldn't be in this function if this was the request.
            WorkerRequest::Recycle(_) => unreachable!(),

            // If we're changing the dimensions we can just move the frames with
            // the bad resolution out of the queue and into a cache. We'll pull
            // from this when we're generating frames instead of reading from
            // the video.
            WorkerRequest::SetDimensions(dimensions, rescale_method) => {
                if let Err(e) = self.ffmpeg_video.set_rescale(*dimensions, *rescale_method) {
                    self.err_state = Some(e.into());
                    return;
                }

                for frame in queue
                    .drain(..)
                    .rev()
                    .filter_map(|frame_result| frame_result.ok())
                    .map(|(frame, _)| frame.into_buffer().expect("ffmpeg-based buffer"))
                {
                    self.frames_to_rescale.push_front(frame);
                }

                self.rescale_method = *rescale_method;

                return;
            }

            // These completely change the timing of the video so we'll just
            // clear the whole queue to avoid dealing with that.
            WorkerRequest::SetTargetFps(target_fps) => {
                self.ffmpeg_video.set_target_fps(*target_fps);
                invalidate_all_items = true;
            }
            WorkerRequest::SetPlaybackSpeed(playback_speed) => {
                self.ffmpeg_video.set_playback_speed(*playback_speed);
                invalidate_all_items = true;
            }

            WorkerRequest::SetPaused(paused) => _ = self.ffmpeg_video.set_paused(*paused),
            WorkerRequest::SetClip(clip) => _ = self.ffmpeg_video.set_clip(*clip),
            WorkerRequest::SeekPlayhead(playhead) => _ = self.ffmpeg_video.seek_playhead(*playhead),
            WorkerRequest::SetLoop(will_loop) => self.ffmpeg_video.set_will_loop(*will_loop),
        }

        if invalidate_all_items {
            queue.clear();
            self.state_history.truncate(1);
            self.frames_to_rescale.clear();
            return;
        }

        let mut last_match_state = client_state;

        // We'll simulate playing back the video with the new settings until we
        // find a mismatched state. The queue is valid up until that point.
        for (matching_item_count, queue_state) in queue
            .iter_mut()
            .map_while(|frame_result| frame_result.as_mut().ok())
            .map(|(_, state)| state)
            .enumerate()
        {
            self.ffmpeg_video.step();
            let expected_state = PlaybackState::snapshot(&self.ffmpeg_video);

            // Found spot where the queue becomes invalid
            if queue_state.playhead != expected_state.playhead {
                queue.truncate(matching_item_count);
                self.state_history.truncate(matching_item_count + 1);
                self.frames_to_rescale.truncate(matching_item_count);
                break;
            }

            // If the playhead matches but the paused state doesn't we can still
            // keep the frame (we just have to change the playhead).
            queue_state.paused = expected_state.paused;

            last_match_state = expected_state;
        }

        self.ffmpeg_video.seek_playhead(last_match_state.playhead);
        self.ffmpeg_video.set_paused(last_match_state.paused);
    }

    fn create_response_for_request(&mut self, _req: Self::Request) -> Self::Response {
        self.get_client_state()
    }
}
