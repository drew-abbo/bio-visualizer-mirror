//! Exports utilities for determining when you should switch frames.

use std::time::{Duration, Instant};

use super::Fps;

/// A clock for tracking when it's time to switch to the next frame (given a
/// target [Fps]). See [Self::is_switch_time].
#[derive(Debug)]
pub struct SwitchTimer {
    target_fps: Fps,
    start_time: Option<Instant>,
    frame_idx: usize,
}

impl SwitchTimer {
    /// Create a new [SwitchTimer].
    pub fn new(target_fps: Fps) -> Self {
        Self {
            target_fps,
            start_time: None,
            frame_idx: 0,
        }
    }

    /// Whether or not it's time to switch to a new frame (given the
    /// [target FPS](Self::target_fps) and when the clock started). This
    /// function will *always* return `true` the first time it's called.
    ///
    /// This function starts an internal clock the first time it's called.
    /// `true` may be returned many times in a row if it gets behind. See
    /// [Self::reset].
    ///
    /// The intention is that this function should indicate when to *switch to*
    /// a new frame, not when to start to creating a new frame. The next frame
    /// should already be ready when this is called so that it can immediately
    /// be switched to.
    pub fn is_switch_time(&mut self) -> bool {
        let now = Instant::now();
        let Some(start_time) = self.start_time else {
            self.start_time = Some(now);
            return true;
        };

        let elapsed_nanos = now.duration_since(start_time).as_nanos();
        let frames_elapsed = elapsed_nanos
            .saturating_mul(self.target_fps.num() as u128)
            / ((self.target_fps.den() as u128) * 1_000_000_000u128);
        let frame_intervals_since_start = frames_elapsed.min(usize::MAX as u128) as usize;

        if frame_intervals_since_start > self.frame_idx {
            self.frame_idx += 1;
            true
        } else {
            false
        }
    }

    /// Returns how long until the next switch should happen.
    ///
    /// Returns [Duration::ZERO] if the timer has not started yet or if the
    /// next switch is already due.
    pub fn time_until_next_switch(&self) -> Duration {
        let Some(start_time) = self.start_time else {
            return Duration::ZERO;
        };

        let now = Instant::now();
        let elapsed_nanos = now.duration_since(start_time).as_nanos();
        let next_frame_idx = self.frame_idx.saturating_add(1) as u128;
        let next_switch_nanos = next_frame_idx
            .saturating_mul(self.target_fps.den() as u128)
            .saturating_mul(1_000_000_000u128)
            / (self.target_fps.num() as u128);
        let remaining_nanos = next_switch_nanos.saturating_sub(elapsed_nanos);

        let secs = (remaining_nanos / 1_000_000_000u128).min(u64::MAX as u128) as u64;
        let nanos = (remaining_nanos % 1_000_000_000u128) as u32;
        Duration::new(secs, nanos)
    }

    /// Resets the clock. This means the next call to [Self::is_switch_time]
    /// will *always* return `true` (as if the object had just been
    /// constructed).
    pub fn reset(&mut self) {
        *self = Self::new(self.target_fps);
    }

    /// The [Fps] this timer is targeting.
    ///
    /// See [Self::set_target_fps].
    pub fn target_fps(&self) -> Fps {
        self.target_fps
    }

    /// Change the [target FPS](Self::target_fps). If `new_target_fps` is
    /// *different* from the original [target FPS](Self::target_fps), the object
    /// will be reset (see [Self::reset]).
    pub fn set_target_fps(&mut self, new_target_fps: Fps) {
        if new_target_fps != self.target_fps {
            self.target_fps = new_target_fps;
            self.reset();
        }
    }
}
