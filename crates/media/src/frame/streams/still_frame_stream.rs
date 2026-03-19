//! Exports [StillFrameStream].

use std::borrow::Cow;
use std::collections::VecDeque;

use util::channels::message_channel::{self, Inbox};
use util::channels::request_channel::{self, Client};
use util::drop_join_thread::{self, DropJoinHandle};

use super::{FrameStream, FrameStreamError, StreamGenerator};
use crate::fps::Fps;
use crate::frame::{Dimensions, Frame, RescaleMethod};
use crate::playback_stream::{PlaybackStream, SeekablePlaybackStream};

/// A [FrameStream] of the same frame over and over again.
///
/// # Example
///
/// ```
/// use media::fps::Fps;
/// use media::frame::{Dimensions, Frame, Pixel, RescaleMethod};
/// use media::frame::streams::{FrameStream, StillFrameStream};
/// use media::playback_stream::PlaybackStream;
///
/// let base_frame = Frame::from_fill((1920, 1080).into(), Pixel::BLUE);
/// let target_fps = Fps::from_int(60).unwrap();
///
/// let mut stream = StillFrameStream::new(base_frame.clone(), target_fps);
///
/// for _ in 0..50 {
///     let new_frame = stream.fetch().unwrap();
///     assert_eq!(new_frame.dimensions(), base_frame.dimensions());
///     assert_eq!(new_frame.pixels(), base_frame.pixels());
///     stream.recycle(new_frame);
/// }
///
/// let new_dimensions = Dimensions::new(1280, 720).unwrap();
/// stream.set_dimensions(new_dimensions, RescaleMethod::fastest());
///
/// for _ in 0..50 {
///     let new_frame = stream.fetch().unwrap();
///     assert_eq!(new_frame.dimensions(), new_dimensions);
///     stream.recycle(new_frame);
/// }
/// ```
#[derive(Debug)]
pub struct StillFrameStream {
    // Worker Communication:
    frame_inbox: Inbox<Frame>,
    worker_client: Client<WorkerRequest, ()>,

    // Shared State:
    target_fps: Fps,
    paused: bool,
    dimensions: Dimensions,
    rescale_method: RescaleMethod,

    // Src Info (Final):
    native_dimensions: Dimensions,

    // Keep this field last. Channels must be dropped before joining thread.
    _worker: DropJoinHandle<()>,
}

impl StillFrameStream {
    /// Create a [StillFrameStream].
    pub fn new(frame: Frame, target_fps: Fps) -> Self {
        Self::with_paused(frame, target_fps, false)
    }

    /// The same as [Self::new], but with a configurable `paused` parameter in
    /// case you don't want to configure whether or not the stream is paused by
    /// default.
    pub fn with_paused(frame: Frame, target_fps: Fps, paused: bool) -> Self {
        let dimensions = frame.dimensions();
        let (frame_inbox, frame_outbox) = message_channel::new::<Frame>();
        let (worker_server, worker_client) = request_channel::new::<WorkerRequest, ()>();
        let worker = drop_join_thread::spawn(move || {
            Worker::new(&frame, target_fps).run(frame_outbox, worker_server);
        });

        Self {
            frame_inbox,
            worker_client,
            target_fps,
            paused,
            dimensions,
            rescale_method: RescaleMethod::default(),
            native_dimensions: dimensions,
            _worker: worker,
        }
    }

    fn worker_alert(&self, msg: WorkerRequest) {
        self.worker_client.alert(msg).expect(EXPECT_WORKER);
    }

    fn worker_request_and_wait(&self, msg: WorkerRequest) {
        let mut req = self.worker_client.request(msg).expect(EXPECT_WORKER);

        // Interrupt worker if it's waiting for us to pull from the queue.
        self.frame_inbox.block_sender().expect(EXPECT_WORKER);

        // Wait for the queue to be fixed.
        req.wait().expect(EXPECT_WORKER);

        self.frame_inbox.unblock_sender().expect(EXPECT_WORKER);
    }
}

impl PlaybackStream<Frame, FrameStreamError> for StillFrameStream {
    fn fetch(&mut self) -> Result<Frame, FrameStreamError> {
        debug_assert!(self.frame_inbox.is_send_blocked() != Ok(true));

        Ok(self.frame_inbox.wait().expect(EXPECT_WORKER))
    }

    fn set_target_fps(&mut self, new_target_fps: Fps) {
        if new_target_fps == self.target_fps {
            return;
        }

        self.worker_alert(WorkerRequest::SetTargetFps(new_target_fps));
        self.target_fps = new_target_fps;
    }

    fn target_fps(&self) -> Fps {
        self.target_fps
    }

    fn set_paused(&mut self, paused: bool) -> bool {
        self.paused = paused;
        paused
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn seek_controls(
        &mut self,
    ) -> Option<&mut dyn SeekablePlaybackStream<Frame, FrameStreamError>> {
        None
    }

    fn recycle(&mut self, frame: Frame) {
        self.worker_alert(WorkerRequest::Recycle(Some(frame)));
    }
}

impl FrameStream for StillFrameStream {
    fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    fn set_dimensions(&mut self, new_dimensions: Dimensions, rescale_method: RescaleMethod) {
        if new_dimensions == self.dimensions
            && (rescale_method == self.rescale_method || new_dimensions == self.native_dimensions)
        {
            return;
        }

        self.worker_request_and_wait(WorkerRequest::SetDimensions(new_dimensions, rescale_method));
        self.dimensions = new_dimensions;
    }

    fn rescale_method(&self) -> Option<RescaleMethod> {
        (self.dimensions != self.native_dimensions).then_some(self.rescale_method)
    }

    fn native_dimensions(&self) -> Dimensions {
        self.native_dimensions
    }
}

const EXPECT_WORKER: &str = "The worker should be connected.";

#[derive(Debug)]
enum WorkerRequest {
    SetTargetFps(Fps),
    Recycle(Option<Frame>),
    SetDimensions(Dimensions, RescaleMethod),
}

#[derive(Debug)]
struct Worker<'a> {
    base_frame: &'a Frame,
    rescaled_base_frame: Cow<'a, Frame>,
    recycled_frames: Vec<Frame>,
    target_fps: Fps,
}

impl<'a> StreamGenerator for Worker<'a> {
    type Data = Frame;
    type Request = WorkerRequest;
    type Response = ();
    type QueueInvalidNote = ();

    fn target_fps(&self) -> Fps {
        self.target_fps
    }

    fn new_data(&mut self, _in_flight: usize) -> Self::Data {
        while let Some(mut recycled_frame) = self.recycled_frames.pop() {
            match recycled_frame.fill_from_frame(self.rescaled_base_frame.as_ref()) {
                Ok(_) => return recycled_frame,
                Err(_) => continue,
            }
        }
        self.rescaled_base_frame.as_ref().clone()
    }

    fn handle_request(&mut self, req: &mut Self::Request) -> Option<Self::QueueInvalidNote> {
        let mut queue_is_invalid = false;

        match req {
            WorkerRequest::SetTargetFps(target_fps) => self.target_fps = *target_fps,

            WorkerRequest::Recycle(recycled_frame) => self
                .recycled_frames
                .push(recycled_frame.take().expect("A frame was sent")),

            WorkerRequest::SetDimensions(new_dimensions, rescale_method) => {
                self.rescaled_base_frame = if *new_dimensions != self.base_frame.dimensions() {
                    Cow::Owned(self.base_frame.rescale(*new_dimensions, *rescale_method))
                } else {
                    Cow::Borrowed(self.base_frame)
                };

                queue_is_invalid = true;
            }
        }

        queue_is_invalid.then_some(())
    }

    fn handle_invalid_queue(
        &mut self,
        queue: &mut VecDeque<Self::Data>,
        _req: &mut Self::Request,
        _queue_invalid_note: Self::QueueInvalidNote,
    ) {
        queue.clear();
    }

    fn create_response_for_request(&mut self, _req: Self::Request) -> Self::Response {}
}

impl<'a> Worker<'a> {
    pub fn new(base_frame: &'a Frame, target_fps: Fps) -> Self {
        let recycled_frames = Vec::with_capacity(16);
        let rescaled_base_frame = Cow::Borrowed(base_frame);
        Self {
            base_frame,
            rescaled_base_frame,
            recycled_frames,
            target_fps,
        }
    }
}
