//! Contains [PlaybackStream] and [SeekablePlaybackStream], streams of data
//! intended to be fetched (played back) at a known frame rate (target
//! [FPS](Fps)).

use crate::Fps;

/// A stream of data where data is intended to be fetched (played back) at a
/// known frame rate (target [FPS](Fps)).
pub trait PlaybackStream<T, E> {
    /// Fetch a data point from the stream. This should be called at relatively
    /// even intervals as many times per second as is dictated by the stream's
    /// target FPS (see [Self::target_fps]).
    ///
    /// If this stream wraps a data source that produces data targeting a
    /// playback frame rate of `30.0` [FPS](Fps) but the
    /// [target FPS](Self::target_fps) is set to `60.0`, then this function
    /// should return new data every second call. Similarly, if the data source
    /// produces data targeting a playback frame rate of `120.0` [FPS](Fps),
    /// then this function should return new data every call, but should
    /// skip/merge every second data point from the source.
    ///
    /// When the stream is [paused](Self::pause), it should return a neutral
    /// data point until it's [un-paused](Self::play) (e.g. a repeat of the last
    /// returned data point or an empty data point). Streams can pause on their
    /// own (e.g. if they end).
    ///
    /// The intent is that this function returns near-immediately. For streams
    /// where lots of computation or I/O must be done to fetch data, data should
    /// be prepared ahead of time (e.g. by a worker thread).
    fn fetch(&mut self) -> Result<T, E>;

    /// The frame rate that is being targeted. [Self::fetch] should be called at
    /// relatively even intervals this many times per second.
    ///
    /// Also see [Self::set_target_fps].
    fn target_fps(&self) -> Fps;

    /// Update the target [FPS](Fps) (see [Self::target_fps] and [Self::fetch]).
    fn set_target_fps(&mut self, new_fps: Fps);

    /// Pauses the media if it's playing, meaning [Self::fetch] should a neutral
    /// data point until it's [un-paused](Self::play). Streams can pause on
    /// their own (e.g. if they end).
    ///
    /// Also see [Self::play], [Self::toggle_play_pause], [Self::is_paused], and
    /// [Self::is_playing].
    fn pause(&mut self);

    /// Plays the media if it's paused and can be played. `true` is returned if
    /// the stream is now playing.
    ///
    /// Also see [Self::pause], [Self::toggle_play_pause], [Self::is_paused],
    /// and [Self::is_playing].
    fn play(&mut self) -> bool;

    /// Whether or not the media is paused (not playing).
    ///
    /// Also see [Self::play], [Self::pause], [Self::toggle_play_pause], and
    /// [Self::is_playing].
    fn is_paused(&self) -> bool;

    /// Whether or not the media is playing (not paused).
    ///
    /// Also see [Self::play], [Self::pause], [Self::toggle_play_pause], and
    /// [Self::is_paused].
    fn is_playing(&self) -> bool {
        !self.is_paused()
    }

    /// Plays the media if it's paused and can be played. Pauses the media if
    /// it's playing. `true` is returned if the stream is now playing.
    ///
    /// Also see [Self::play], [Self::pause], [Self::is_paused], and
    /// [Self::is_playing].
    fn toggle_play_pause(&mut self) -> bool {
        if self.is_playing() {
            self.pause();
            false
        } else {
            self.play()
        }
    }

    /// For some data types that are expensive to construct (e.g. frame buffers)
    /// it can help to return produced data back to the producer so that
    /// internal buffers can be reused.
    ///
    /// `data` should always be an object that was returned by an earlier call
    /// to [Self::fetch].
    ///
    /// There is no promise that this method will actually recycle anything (the
    /// default implementation just drops `data`).
    fn recycle(&mut self, data: T) -> Result<(), E> {
        drop(data);
        Ok(())
    }

    /// Some streams have additional playback capabilities (see
    /// [SeekablePlaybackStream]). Streams that don't will return [None].
    ///
    /// This function should never return [None] on one call and [Some] on
    /// another for the same `self`.
    fn seek_controls(&mut self) -> Option<&mut dyn SeekablePlaybackStream<T, E>>;
}

/// A trait for [PlaybackStream]s with additional playback capabilities (a known
/// duration, seeking controls, and a toggle for whether or not the stream
/// should loop).
pub trait SeekablePlaybackStream<T, E>: PlaybackStream<T, E> {
    /// The number of unique data points in the stream.
    ///
    /// This value is not tied to [PlaybackStream::target_fps], meaning it may
    /// not correspond to the number of unique data points
    /// [PlaybackStream::fetch] will return.
    fn stream_duration(&self) -> usize;

    /// The index of the next data point in the stream.
    ///
    /// This value should never be greater than the
    /// [stream duration](Self::stream_duration). If this value *equals* the
    /// [stream duration](Self::stream_duration), the stream has ended (see
    /// [Self::is_over]).
    ///
    /// Also see [Self::seek_playhead] and [Self::seek_playhead_scalar].
    fn playhead(&self) -> usize;

    /// Seek to a specific [playhead](Self::playhead) position in the stream.
    ///
    /// `playhead` values should be clamped to be no more than the
    /// stream duration (see [Self::stream_duration]).
    ///
    /// If the stream is set to loop ([Self::will_loop] returns `true`) and
    /// `playhead` is greater than or equal to the
    /// [stream's duration](Self::stream_duration), then the playhead should
    /// actually be set to `0`.
    ///
    /// Also see [Self::playhead] and [Self::seek_playhead_scalar].
    fn seek_playhead(&mut self, playhead: usize) -> Result<(), E>;

    /// The same as [Self::seek_playhead] except you can provide a value in the
    /// range `[0.0, 1.0]` (inclusive) instead of a specific
    /// [playhead](Self::playhead) value.
    ///
    /// Out-of-range `playhead` values should be clamped to be within the range
    /// (meaning values like [f64::INFINITY] can be used to jump to the end of a
    /// stream).
    fn seek_playhead_scalar(&mut self, playhead: f64) -> Result<(), E> {
        let playhead = playhead.clamp(0.0, 1.0);

        // No-op for non-zero abnormal numbers like `NaN`.
        if playhead != 0.0 && !playhead.is_normal() {
            return Ok(());
        }

        self.seek_playhead((playhead / self.stream_duration() as f64) as usize)
    }

    /// Whether or not the stream is over (true if [Self::playhead] returns the
    /// same value as [Self::stream_duration]).
    fn is_over(&self) -> bool {
        debug_assert!(self.playhead() <= self.stream_duration());
        self.playhead() == self.stream_duration()
    }

    /// Whether or not the stream will loop instead of ending, meaning
    /// [Self::is_over] will never return `true` (unless the stream has no
    /// duration).
    ///
    /// Also see [Self::set_will_loop] and [Self::is_over].
    fn will_loop(&self) -> bool;

    /// Configure whether or not the stream should loop.
    ///
    /// Also see [Self::will_loop] and [Self::is_over].
    fn set_will_loop(&mut self, should_loop: bool) -> bool;
}
