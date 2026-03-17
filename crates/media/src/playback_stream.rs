//! Contains [PlaybackStream] and [SeekablePlaybackStream], streams of data
//! intended to be fetched (played back) at a known frame rate (target
//! [FPS](Fps)).

use std::any::Any;
use std::num::NonZeroUsize;
use std::ops::RangeInclusive;

use crate::fps::{self, Fps, FpsError};

mod buffering_suggestor;
pub use buffering_suggestor::BufferingSuggestor;

/// A stream of data where data is intended to be fetched (played back) at a
/// known frame rate (target [FPS](Fps)).
pub trait PlaybackStream<T, E>: Any + 'static {
    /// Fetch a data point from the stream. This should be called at relatively
    /// even intervals as many times per second as is dictated by the stream's
    /// target FPS (see [Self::target_fps]).
    ///
    /// If this stream wraps a data source that produces data targeting a
    /// different playback rate than the [target FPS](Self::target_fps), the
    /// underlying data source should be resampled in some way so that it
    /// matches the [target FPS](Self::target_fps). For example, if a stream
    /// wraps a data source that produces data targeting a playback frame rate
    /// of `30.0` [FPS](Fps) but the [target FPS](Self::target_fps) is set to
    /// `60.0`, then this function could return new data every second call
    /// (returning a duplicate of the last data in the interim). Similarly, if
    /// the data source produced data targeting a playback frame rate of `120.0`
    /// [FPS](Fps), then this function could return new data every call, but
    /// should skip/merge every second data point from the source. See
    /// [fps::resample] for help implementing this.
    ///
    /// When the stream is [paused](Self::pause), it should return a neutral
    /// data point until it's [un-paused](Self::play) (e.g. a repeat of the last
    /// returned data point or an empty data point). Streams can pause on their
    /// own (e.g. if they end).
    ///
    /// The intent is that this function returns near-immediately. For streams
    /// where lots of computation or I/O must be done to fetch data, data should
    /// be prepared ahead of time (e.g. by a worker thread). See
    /// [BufferingSuggestor] for help implementing this.
    fn fetch(&mut self) -> Result<T, E>;

    /// Update the target [FPS](Fps) (see [Self::target_fps] and [Self::fetch]).
    fn set_target_fps(&mut self, new_target_fps: Fps);

    /// The frame rate that is being targeted. [Self::fetch] should be called at
    /// relatively even intervals this many times per second.
    ///
    /// Also see [Self::set_target_fps].
    fn target_fps(&self) -> Fps;

    /// Plays the media if it's paused and can be played. `true` is returned if
    /// the stream is now playing.
    ///
    /// Also see [Self::pause], [Self::toggle_play_pause], [Self::set_paused],
    /// [Self::set_playing], [Self::is_paused], and [Self::is_playing].
    fn play(&mut self) -> bool {
        self.set_paused(false)
    }

    /// Pauses the media if it's playing, meaning [Self::fetch] should a neutral
    /// data point until it's [un-paused](Self::play). Streams can pause on
    /// their own (e.g. if they end).
    ///
    /// Also see [Self::play], [Self::toggle_play_pause], [Self::set_paused],
    /// [Self::set_playing], [Self::is_paused], and [Self::is_playing].
    fn pause(&mut self) {
        self.set_paused(true);
    }

    /// Plays the media if it's paused and can be played. Pauses the media if
    /// it's playing. `true` is returned if the stream is now playing.
    ///
    /// Also see [Self::play], [Self::pause], [Self::set_paused],
    /// [Self::set_playing], [Self::is_paused], and [Self::is_playing].
    fn toggle_play_pause(&mut self) -> bool {
        self.set_paused(!self.is_paused())
    }

    /// Tries to set whether or not the stream is paused. Note that a stream
    /// cannot always be played (e.g. if it is over) and streams can pause on
    /// their own. See [Self::pause] and [Self::play] for more info.
    ///
    /// Also see [Self::play], [Self::pause], [Self::toggle_play_pause],
    /// [Self::set_playing], [Self::is_paused], and [Self::is_playing].
    fn set_paused(&mut self, paused: bool) -> bool;

    /// Tries to set whether or not the stream is playing. Note that a stream
    /// cannot always be played (e.g. if it is over) and streams can pause on
    /// their own. See [Self::pause] and [Self::play] for more info.
    ///
    /// Also see [Self::play], [Self::pause], [Self::toggle_play_pause],
    /// [Self::set_paused], [Self::is_paused], and [Self::is_playing].
    fn set_playing(&mut self, playing: bool) -> bool {
        self.set_paused(!playing)
    }

    /// Whether or not the media is paused (not playing).
    ///
    /// Also see [Self::play], [Self::pause], [Self::toggle_play_pause],
    /// [Self::set_paused], [Self::set_playing], and [Self::is_playing].
    fn is_paused(&self) -> bool;

    /// Whether or not the media is playing (not paused).
    ///
    /// Also see [Self::play], [Self::pause], [Self::toggle_play_pause],
    /// [Self::set_paused], [Self::set_playing], and [Self::is_paused].
    fn is_playing(&self) -> bool {
        !self.is_paused()
    }

    /// For some data types that are expensive to construct (e.g. frame buffers)
    /// it can help to return produced data back to the producer so that
    /// internal buffers can be reused.
    ///
    /// When calling, `data` should always be an *unmodified* object that was
    /// returned by an earlier call to [Self::fetch]. If this is detectably not
    /// the case, the implementation may panic.
    ///
    /// There is no promise that this method will actually recycle anything (the
    /// default implementation just drops `data`).
    fn recycle(&mut self, data: T) {
        drop(data);
    }

    /// Some streams have additional playback capabilities (see
    /// [SeekablePlaybackStream]). Streams that don't will return [None].
    ///
    /// This function should never return [None] on one call and [Some] on
    /// another for the same `self`. Streams that implement
    /// [SeekablePlaybackStream] should just return a `dyn` reference to
    /// themselves.
    fn seek_controls(&mut self) -> Option<&mut dyn SeekablePlaybackStream<T, E>>;
}

/// A trait for [PlaybackStream]s with additional playback capabilities
/// (intended for recorded data that can be replayed).
pub trait SeekablePlaybackStream<T, E>: PlaybackStream<T, E> {
    /// The range of this stream that can be played. The length of the returned
    /// range is the number of frames that can be
    /// [fetched](PlaybackStream::fetch) before the stream ends (pauses on the
    /// last data point) or [loops](Self::will_loop). The range's start and end
    /// are both inclusive (e.g. `start..=end`). `start <= end` should always
    /// be true.
    ///
    /// This can change on its own if the
    /// [target FPS changes](PlaybackStream::set_target_fps) or if the stream is
    /// [re-clipped](Self::set_clip).
    ///
    /// The range end should never be greater than or equal to the stream's
    /// [unclipped duration](Self::unclipped_stream_duration). The start bound
    /// should always be less than or equal to the end bound (meaning the clip
    /// cannot have 0 length).
    fn clip(&self) -> RangeInclusive<usize>;

    /// This function should clip this stream to a new playback range so that it
    /// starts and/or ends at new times. The range's start and end are both
    /// inclusive (e.g. `start..=end`).
    ///
    /// For a range `start..=end`, and an
    /// [unclipped duration](Self::unclipped_stream_duration) `n`, `start` and
    /// `end` should both be clamped to be less than `n`. If `start > end`,
    /// `start` should be used in place of `end` (`start..=start`). The fixed
    /// range should be returned.
    ///
    /// If the [playhead](Self::playhead) is outside of the range it should
    /// [seek](Self::seek_playhead) to be inside of the range (jumping to the
    /// start of the range if it's before and jumping to the end of the stream
    /// or looping if it's after).
    fn set_clip(&mut self, playback_range: RangeInclusive<usize>) -> RangeInclusive<usize>;

    /// The length of the stream when it's not [clipped](Self::set_clip) to be
    /// shorter.
    ///
    /// If this returns `n`, the largest range [Self::clip] can return is
    /// `0..=(n - 1)`.
    fn unclipped_stream_duration(&self) -> usize {
        self.unclipped_stream_duration_non_zero().get()
    }

    /// The same as [Self::unclipped_stream_duration] but non-zero, since all
    /// streams must have some duration.
    fn unclipped_stream_duration_non_zero(&self) -> NonZeroUsize;

    /// The [clipped](Self::set_clip) length of the stream (also see
    /// [Self::unclipped_stream_duration]).
    fn clipped_stream_duration(&self) -> usize {
        self.clipped_stream_duration_non_zero().get()
    }

    /// The same as [Self::clipped_stream_duration] but non-zero, since all
    /// streams must have some duration.
    fn clipped_stream_duration_non_zero(&self) -> NonZeroUsize {
        let clip = self.clip();
        NonZeroUsize::new(clip.end() - clip.start() + 1).expect("start <= end")
    }

    /// The index of the next data point that will be
    /// [fetched](PlaybackStream::fetch) from the stream.
    ///
    /// This value should never be before start bound or after the end bound of
    /// the range [Self::clip] returns.
    fn playhead(&self) -> usize;

    /// Seek to a specific global [playhead](Self::playhead) position in the
    /// stream.
    ///
    /// `playhead` values should be clamped to be inside the range [Self::clip]
    /// returns.
    fn seek_playhead(&mut self, playhead: usize) -> Result<usize, E>;

    /// Like [Self::seek_playhead] except `playhead` starts at the beginning of
    /// the [clip](Self::clip) (i.e. a `playhead` value of 0 corresponds to the
    /// start bound of [Self::clip]).
    fn seek_playhead_relative(&mut self, playhead: usize) -> Result<usize, E> {
        self.seek_playhead(self.clip().start() + playhead)
    }

    /// Whether or not the stream will loop instead of pausing at the end. When
    /// `true`, the [playhead](Self::playhead) will not pause on the last frame
    /// of the [clip](Self::clip).
    ///
    /// Also see [Self::set_loop].
    fn will_loop(&self) -> bool;

    /// Configure whether or not the stream should loop or pause when it ends.
    /// When `true`, the [playhead](Self::playhead) will not pause on the
    /// last frame of the [clip](Self::clip).
    ///
    /// Also see [Self::will_loop].
    fn set_loop(&mut self, do_loop: bool);

    /// The multipler that changes the playback speed of a stream.
    fn playback_speed(&self) -> Fps;

    /// Set a multipler that changes the playback speed of a stream.
    fn set_playback_speed(&mut self, multipler: Fps);

    /// The same as [Self::playback_speed] but as a floating point number.
    fn playback_speed_float(&self) -> f64 {
        self.playback_speed().as_float()
    }

    /// The same as [Self::playback_speed] but the parameter is a floating point
    /// number. The new multipler (as a rational approximation) is returned.
    ///
    /// An error can be returned if the float fails to approximate a positive
    /// rational. See [Fps::from_float_raw].
    fn set_playback_speed_float(&mut self, multipler: f64) -> Result<Fps, FpsError> {
        let multipler = Fps::from_float_raw(multipler)?;
        self.set_playback_speed(multipler);
        Ok(multipler)
    }

    /// Whether or not the [playback speed](Self::playback_speed) multipler is
    /// unset (or equal to `1/1`).
    fn is_normal_playback_speed(&self) -> bool {
        self.playback_speed() == fps::consts::FPS_1
    }
}
