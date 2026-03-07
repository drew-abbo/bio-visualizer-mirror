//! Exports [StillFrameStream].

use std::borrow::Cow;
use std::collections::VecDeque;
use std::convert::Infallible;

use util::channels::ChannelError;
use util::channels::message_channel::{self, Inbox, Outbox};
use util::channels::request_channel::{self, Client, ReqRes, Server};
use util::drop_join_thread::{self, DropJoinHandle};

use super::{FrameStream, FrameStreamError};
use crate::fps::Fps;
use crate::frame::{Dimensions, Frame, RescaleMethod};
use crate::playback_stream::{BufferingSuggestor, PlaybackStream, SeekablePlaybackStream};

/// A [FrameStream] of the same frame over and over again.
///
/// ```
/// use media::fps::Fps;
/// use media::frame::{Dimensions, Frame, Pixel, streams::StillFrameStream};
/// use media::playback_stream::PlaybackStream;
///
/// let base_frame = Frame::from_fill(Dimensions::new(1920, 1080).unwrap(), Pixel::BLUE);
///
/// let mut stream = StillFrameStream::new(base_frame.clone(), Fps::from_int(60).unwrap());
///
/// for _ in 0..100 {
///     let new_frame = stream.fetch().unwrap();
///     assert_eq!(base_frame.dimensions(), new_frame.dimensions());
///     assert_eq!(base_frame.pixels(), new_frame.pixels());
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
            _ = Worker::new(&frame, frame_outbox, worker_server, target_fps).run();
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
}

impl PlaybackStream<Frame, FrameStreamError> for StillFrameStream {
    fn fetch(&mut self) -> Result<Frame, FrameStreamError> {
        Ok(self.frame_inbox.wait().expect(EXPECT_WORKER))
    }

    fn set_target_fps(&mut self, new_target_fps: Fps) {
        if new_target_fps == self.target_fps {
            return;
        }

        self.worker_client
            .alert(WorkerRequest::SetTargetFps(new_target_fps))
            .expect(EXPECT_WORKER);

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
        self.worker_client
            .alert(WorkerRequest::Recycle(frame))
            .expect(EXPECT_WORKER);
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

const EXPECT_WORKER: &str = "The worker should be connected.";

#[derive(Debug)]
enum WorkerRequest {
    SetTargetFps(Fps),
    Recycle(Frame),
    SetDimensions(Dimensions, RescaleMethod),
}

#[derive(Debug)]
struct Worker<'a> {
    // Client communication:
    frame_outbox: Outbox<Frame>,
    worker_server: Server<WorkerRequest, ()>,

    // Frame generation:
    base_frame: &'a Frame,
    rescaled_base_frame: Cow<'a, Frame>,
    buffering_suggestor: BufferingSuggestor,
    buffering_suggestion: usize,
    recycled_frames: Vec<Frame>,
}

impl<'a> Worker<'a> {
    pub fn new(
        base_frame: &'a Frame,
        frame_outbox: Outbox<Frame>,
        worker_server: Server<WorkerRequest, ()>,
        starting_target_fps: Fps,
    ) -> Self {
        let rescaled_base_frame = Cow::Borrowed(base_frame);

        let buffering_suggestor = BufferingSuggestor::new(starting_target_fps);
        let buffering_suggestion = buffering_suggestor.buffering_suggestion();

        let recycled_frames = Vec::with_capacity(buffering_suggestion);

        Self {
            base_frame,
            rescaled_base_frame,
            frame_outbox,
            worker_server,
            buffering_suggestor,
            buffering_suggestion,
            recycled_frames,
        }
    }

    pub fn run(mut self) -> Result<Infallible, ChannelError> {
        loop {
            let (elapsed_time, result) = BufferingSuggestor::run_timed(|| {
                self.handle_requests()?;

                let new_frame = self.new_rescaled_base_frame_clone();
                self.frame_outbox
                    .send_bounded(new_frame, self.buffering_suggestion)?;

                Ok(())
            });
            _ = result?;

            self.buffering_suggestor.add_time_sample(elapsed_time);
            self.buffering_suggestion = self.buffering_suggestor.buffering_suggestion();
        }
    }

    fn handle_requests(&mut self) -> Result<(), ChannelError> {
        let mut handle_request = |req: WorkerRequest| match req {
            WorkerRequest::SetTargetFps(target_fps) => {
                self.buffering_suggestor.set_dest_fps(target_fps);
            }

            WorkerRequest::Recycle(recycled_frame) => {
                if recycled_frame.dimensions() == self.rescaled_base_frame.dimensions() {
                    self.recycled_frames.push(recycled_frame);
                }
            }

            WorkerRequest::SetDimensions(new_dimensions, rescale_method) => {
                debug_assert!(
                    new_dimensions != self.rescaled_base_frame.dimensions(),
                    "This should be caught before being sent."
                );

                self.rescaled_base_frame = if new_dimensions != self.base_frame.dimensions() {
                    Cow::Owned(self.base_frame.rescale(new_dimensions, rescale_method))
                } else {
                    Cow::Borrowed(self.base_frame)
                };

                // Because we changed the dimensions, all of our cached frames
                // are invalid.
                self.frame_outbox
                    .with_queue_in_place(|frame_queue| frame_queue.clear());
            }
        };

        let for_each_request =
            |worker_requests: &mut VecDeque<ReqRes<WorkerRequest, ()>>| -> Result<(), ChannelError> {
                for (req, res) in worker_requests.drain(..) {
                    handle_request(req);

                    if let Some(res) = res {
                        res.respond(())?;
                    }
                }
                Ok(())
            };

        self.worker_server
            .check_in_place(for_each_request)?
            .unwrap_or(Ok(()))?;

        Ok(())
    }

    fn new_rescaled_base_frame_clone(&mut self) -> Frame {
        while let Some(mut recycled_frame) = self.recycled_frames.pop() {
            match recycled_frame.fill_from_frame(self.rescaled_base_frame.as_ref()) {
                Ok(_) => return recycled_frame,
                Err(_) => continue,
            }
        }
        self.rescaled_base_frame.as_ref().clone()
    }
}
