//! Contains the [Producer] type for producing [Frame]s from a
//! [streams::FrameStream].
//!
//! It's important to note that a "frame buffer" and "buffered frames" are
//! different concepts. A "frame buffer" refers to a block of memory where the
//! data for a frame lives (an array of pixels). "Buffered frames" are frames
//! that we have pre-loaded ahead of time (think about the lighter gray bar that
//! sits ahead of your playhead when you're streaming video over an internet
//! connection).

use std::iter;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;

use util::channels::message_channel::{self, Inbox, Outbox};
use util::channels::{ChannelError, ChannelResult};

use super::*;
use streams::still_frame::StillFrame;
use streams::{FrameStream, StreamError, StreamStats};

/// The maximum amount of time a [Producer] will wait for a frame when
/// [Producer::fetch_frame] is called.
///
/// This number is larger for non-debug builds to account for slow/overloaded
/// computers. In a dev environment, this timeout being hit is probably the
/// result of a deadlock or infinite loop.
pub const DEFAULT_FETCH_FRAME_TIMEOUT: Duration =
    Duration::from_secs(if cfg!(debug_assertions) { 5 } else { 60 });

/// Starts up a worker thread that caches/buffers the [Frame]s a [FrameStream]
/// produces. This allows [Frame]s to be fetched "instantly" on the current
/// thread (so long as the computer isn't under too heavy of a load).
pub struct Producer {
    buffered_frames: Option<Inbox<Result<Frame, ProducerError>>>,
    frame_fetched_signal: Option<Outbox<()>>,
    recycled_frames: Option<Outbox<Frame>>,
    last_frame_uid: Option<Uid>,
    last_fetch_timed_out: bool,
    stream_stats: StreamStats,
    worker: Option<JoinHandle<()>>,
}

impl Producer {
    /// Create a new [Producer] that produces frames from the provided
    /// [FrameStream].
    ///
    /// An error will be returned if the [OnStreamEnd] rule is invalid.
    pub fn new<S: FrameStream>(
        stream: S,
        on_stream_end: OnStreamEnd,
    ) -> Result<Self, OnStreamEndError> {
        match on_stream_end {
            OnStreamEnd::HoldLastFrame if stream.stats().stream_length.is_none() => {
                return Err(OnStreamEndError::HoldLastFrameWithoutKnownLength);
            }

            OnStreamEnd::Loop if stream.stats().stream_length.is_none() => {
                return Err(OnStreamEndError::LoopWithoutKnownLength);
            }

            OnStreamEnd::HoldFrame(ref frame)
                if stream.stats().dimensions != frame.dimensions() =>
            {
                return Err(OnStreamEndError::LoopWithoutKnownLength);
            }

            _ => {}
        }

        let stream_stats = stream.stats();
        let buffering_recommendation = stream_stats.buffering_recommendation.min(1);

        let (buffered_frames_inbox, buffered_frames_outbox) =
            message_channel::with_capacity(buffering_recommendation);

        let (frame_fetched_signal_inbox, frame_fetched_signal_outbox) =
            message_channel::with_capacity(buffering_recommendation);

        let (recycled_frames_inbox, recycled_frames_outbox) =
            message_channel::with_capacity(buffering_recommendation);

        let worker = thread::spawn(move || {
            _ = start_worker(
                buffered_frames_outbox,
                frame_fetched_signal_inbox,
                FrameFetcher::new(Box::new(stream), recycled_frames_inbox, on_stream_end),
                buffering_recommendation,
            );
        });

        Ok(Self {
            buffered_frames: Some(buffered_frames_inbox),
            frame_fetched_signal: Some(frame_fetched_signal_outbox),
            recycled_frames: Some(recycled_frames_outbox),
            last_frame_uid: None,
            last_fetch_timed_out: false,
            stream_stats,
            worker: Some(worker),
        })
    }

    /// Used to recycle the last frame that was returned by this [Producer].
    ///
    /// If `recycled_frame` is not the last frame that was returned by this
    /// [Producer], this function will panic.
    pub fn recycle_frame(&mut self, recycled_frame: Frame) {
        assert_eq!(
            Some(recycled_frame.uid()),
            self.last_frame_uid,
            "You can only recycle the last frame that was returned."
        );

        self.recycled_frames
            .as_ref()
            .expect("The channel should be present.")
            .send(recycled_frame)
            .expect(THREAD_PANIC_MSG);
    }

    /// Fetch the next frame of the stream. This function will block, but unless
    /// the computer is under heavy load, this should produce a frame close to
    /// immediately
    ///
    /// This function will time out after [DEFAULT_FETCH_FRAME_TIMEOUT]. If you
    /// want to specify a specific time, see [Self::fetch_frame_timeout].
    ///
    /// Make sure to recycle the last frame that was returned before calling
    /// this function (see [Self::recycle_frame]).
    pub fn fetch_frame(&mut self) -> Result<Frame, ProducerError> {
        self.fetch_frame_timeout(DEFAULT_FETCH_FRAME_TIMEOUT)
    }

    /// Fetch the next frame of the stream. This function will block, but unless
    /// the computer is under heavy load, this should produce a frame close to
    /// immediately.
    ///
    /// This function will time out after `duration`. If you want to time out
    /// after the default amount of time, see [Self::fetch_frame].
    ///
    /// Make sure to recycle the last frame that was returned before calling
    /// this function (see [Self::recycle_frame]).
    pub fn fetch_frame_timeout(&mut self, timeout: Duration) -> Result<Frame, ProducerError> {
        // We fetch the next frame before notifying the worker that we're
        // fetching. Not giving the worker as much of a head start as possible
        // seems bad, but it's in case the worker is extremely fast. If the
        // worker sees that we're fetching and then blocks the `buffered_frames`
        // channel to add a new frame right away, we'll go to pull a frame out
        // to return and it will be blocked. We're prioritizing this function's
        // speed.
        let fetch_result = self
            .buffered_frames
            .as_ref()
            .expect("The channel should be present.")
            .wait_timeout(timeout);

        if !self.last_fetch_timed_out {
            self.frame_fetched_signal
                .as_ref()
                .expect("The channel should be present.")
                .send(())
                .expect(THREAD_PANIC_MSG);
        }

        let frame = match fetch_result {
            Ok(frame_result) => {
                self.last_fetch_timed_out = false;

                frame_result?
            }

            Err(ChannelError::Timeout { timeout }) => {
                self.last_fetch_timed_out = true;

                return Err(ProducerError::Timeout { timeout });
            }

            Err(_) => panic!("{THREAD_PANIC_MSG}"),
        };

        if frame.dimensions() != self.stats().dimensions {
            return Err(ProducerError::UnexpectedDimensions {
                expected: self.stats().dimensions,
                actual: frame.dimensions(),
            });
        }

        self.last_frame_uid = Some(frame.uid());

        Ok(frame)
    }

    /// Stats about the underlying [FrameStream] that this producer is using.
    /// See [StreamStats].
    pub fn stats(&self) -> StreamStats {
        self.stream_stats
    }
}

/// A [Drop] implementation is needed to join the worker thread that caches
/// frames.
impl Drop for Producer {
    /// A [Drop] implementation is needed to join the worker thread that caches
    /// frames.
    fn drop(&mut self) {
        self.worker
            .take()
            .expect("The worker join handle should be present.")
            .join()
            .expect(THREAD_PANIC_MSG);
    }
}

/// What a [Producer] should do when a [FrameStream] ends.
///
/// - [HoldLastFrame](OnStreamEnd::HoldLastFrame) and [Loop](OnStreamEnd::Loop)
///   are invalid options for streams without a
///   [length](StreamStats::stream_length).
/// - The [Dimensions] of the [Frame] provided with
///   [HoldFrame](OnStreamEnd::HoldLastFrame) must match the [Dimensions] of the
///   [FrameStream].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnStreamEnd {
    /// The last frame that was produced will be repeated forever. If the stream
    /// produces no frames it will produce completely black frames.
    HoldLastFrame,
    /// The first frame that was produced will be repeated forever. If the
    /// stream produces no frames it will produce completely black frames.
    HoldFirstFrame,
    /// Repeat some arbitrary frame forever.
    HoldFrame(Frame),
    /// The stream will produce completely black frames.
    HoldSolidBlack,
    /// The stream will loop from the beginning. If the stream produces no
    /// frames it will produce completely black frames.
    Loop,
    /// Return a [ProducerError::UnexpectedStreamEnd] error.
    Error,
    /// Panic.
    Unreachable,
}

/// The default is [HoldSolidBlack](OnStreamEnd::HoldSolidBlack).
impl Default for OnStreamEnd {
    fn default() -> Self {
        Self::HoldSolidBlack
    }
}

/// An unrecoverable error from a [Producer].
#[derive(Error, Debug)]
pub enum ProducerError {
    #[error("The stream ended unexpectedly.")]
    UnexpectedStreamEnd,
    #[error("The stream was supposed to end but didn't.")]
    Timeout { timeout: Duration },
    #[error("The stream failed unexpectedly: {0}")]
    StreamError(#[from] StreamError),
    #[error("Expected frame dimensions {expected} but got {actual}.")]
    UnexpectedDimensions {
        expected: Dimensions,
        actual: Dimensions,
    },
    #[error(transparent)]
    InvalidOnStreamEnd(#[from] OnStreamEndError),
    #[error("The producer is permanently stuck in an error state.")]
    PermanentErrorState,
}

/// Indicates that something was wrong with an [OnStreamEnd] configuration.
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OnStreamEndError {
    #[error("`HoldLastFrame` is invalid for streams without a known length.")]
    HoldLastFrameWithoutKnownLength,
    #[error("`Loop` is invalid for streams without a known length.")]
    LoopWithoutKnownLength,
    #[error(
        "{} (expected {expected} but got {actual}).",
        "The hold frame must have the same dimensions as the stream"
    )]
    InvalidHoldFrameDimensions {
        expected: Dimensions,
        actual: Dimensions,
    },
}

const THREAD_PANIC_MSG: &str = "The thread that holds the other end of a channel panicked.";

/// Abstracts away the process of fetching frames and handling stream endings.
struct FrameFetcher {
    stream: Box<dyn FrameStream>,
    recycled_frames: Inbox<Frame>,
    frames_fetched: usize,
    stream_end_state: StreamEndState,
}

impl FrameFetcher {
    pub fn new(
        stream: Box<dyn FrameStream>,
        recycled_frames: Inbox<Frame>,
        on_stream_end: OnStreamEnd,
    ) -> Self {
        let stream_end_state = match on_stream_end {
            OnStreamEnd::HoldLastFrame => StreamEndState::HoldLastFrame(None),
            OnStreamEnd::HoldFirstFrame => StreamEndState::HoldFirstFrame(None),
            OnStreamEnd::HoldFrame(frame) => StreamEndState::HoldFrame(Some(frame)),
            OnStreamEnd::HoldSolidBlack => StreamEndState::HoldSolidBlack,
            OnStreamEnd::Loop => StreamEndState::Loop,
            OnStreamEnd::Error => StreamEndState::Error,
            OnStreamEnd::Unreachable => StreamEndState::Unreachable,
        };

        Self {
            stream,
            recycled_frames,
            stream_end_state,
            frames_fetched: 0,
        }
    }

    pub fn fetch_frame(&mut self) -> ChannelResult<Result<Frame, ProducerError>> {
        if let Some(stream_length) = self.stream.stats().stream_length
            && self.frames_fetched == stream_length
        {
            self.handle_stream_end()
        } else {
            self.get_frame()
        }
    }

    fn get_frame(&mut self) -> ChannelResult<Result<Frame, ProducerError>> {
        let frame_result = if let Some(recycled_frame) = self.recycled_frames.check()? {
            self.stream.write_next_frame(recycled_frame)
        } else {
            self.stream.create_next_frame()
        };

        let frame = match frame_result {
            Ok(frame) => frame,
            Err(err) => return Ok(Err(err.into())),
        };

        if frame.dimensions() != self.stream.stats().dimensions {
            return Ok(Err(ProducerError::UnexpectedDimensions {
                expected: self.stream.stats().dimensions,
                actual: frame.dimensions(),
            }));
        }

        self.frames_fetched += 1;

        match &mut self.stream_end_state {
            StreamEndState::HoldLastFrame(last_frame) => {
                if let Some(stream_length) = self.stream.stats().stream_length
                    && self.frames_fetched == stream_length
                {
                    *last_frame = Some(frame.clone())
                }
            }

            StreamEndState::HoldFirstFrame(first_frame) => {
                if first_frame.is_none() {
                    *first_frame = Some(frame.clone());
                }
            }

            _ => {}
        }

        Ok(Ok(frame))
    }

    fn handle_stream_end(&mut self) -> ChannelResult<Result<Frame, ProducerError>> {
        let solid_black = || {
            StillFrame::new(Frame::from_fill(
                self.stream.stats().dimensions,
                Pixel::BLACK,
            ))
        };

        let frame_or_solid_black = |frame: &mut Option<Frame>| {
            if let Some(frame) = frame.take() {
                StillFrame::new(frame)
            } else {
                solid_black()
            }
        };

        let new_stream = match &mut self.stream_end_state {
            StreamEndState::HoldLastFrame(frame) => frame_or_solid_black(frame),
            StreamEndState::HoldFirstFrame(frame) => frame_or_solid_black(frame),
            StreamEndState::HoldFrame(frame) => {
                StillFrame::new(frame.take().expect("There should be a hold-frame."))
            }
            StreamEndState::HoldSolidBlack => solid_black(),

            StreamEndState::Loop => {
                if let Err(err) = self.stream.start_over() {
                    return Ok(Err(err.into()));
                }
                self.frames_fetched = 0;
                return self.get_frame();
            }

            StreamEndState::Error => return Ok(Err(ProducerError::UnexpectedStreamEnd)),
            StreamEndState::Unreachable => unreachable!("This stream shouldn't be able to end."),
        };

        // We'll just completely change the underlying stream.
        self.stream = Box::new(new_stream);
        self.stream_end_state = StreamEndState::Unreachable;
        self.frames_fetched = 0;
        self.get_frame()
    }
}

#[derive(Debug)]
enum StreamEndState {
    HoldLastFrame(Option<Frame>),
    HoldFirstFrame(Option<Frame>),
    HoldFrame(Option<Frame>),
    HoldSolidBlack,
    Loop,
    Error,
    Unreachable,
}

fn start_worker(
    buffered_frames: Outbox<Result<Frame, ProducerError>>,
    frame_fetched_signal: Inbox<()>,
    mut frame_fetcher: FrameFetcher,
    buffering_recommendation: usize,
) -> ChannelResult<()> {
    // We won't bother checking if the producer thread has asked for any frames
    // until we've pre-fetched at least `buffering_recommendation` of them.
    // We'll fetch frames only when one has been taken out of the queue after
    // that (if we're running fast, there will never be more than 1 frame
    // missing from the buffer).
    let mut bootstrap_then_wait_for_fetch_signal = (0..buffering_recommendation)
        .map(|_| ())
        .chain(iter::repeat(()).take_while(|_| frame_fetched_signal.wait().is_ok()));

    while bootstrap_then_wait_for_fetch_signal.next().is_some() {
        let to_send = frame_fetcher.fetch_frame()?;
        let is_err = to_send.is_err();

        buffered_frames.send(to_send)?;

        if is_err {
            // We'll enter a permanent error state (until we're disconnected),
            // only responding with "go away... I'm broken 😞".
            //
            // We still respond (even though producer errors are not recoverable
            // for the producer) because we're not supposed to disconnect until
            // the other thread does.
            while frame_fetched_signal.wait().is_ok() {
                buffered_frames.send(Err(ProducerError::PermanentErrorState))?;
            }
        }
    }

    Ok(())
}
