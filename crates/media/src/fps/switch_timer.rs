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
        let frames_elapsed = elapsed_nanos.saturating_mul(self.target_fps.num() as u128)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    use crate::fps::consts;

    // --- is_switch_time decisions ---

    #[test]
    fn first_call_always_returns_true() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        assert!(timer.is_switch_time());
    }

    #[test]
    fn second_call_without_time_elapsed_returns_false() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        timer.is_switch_time(); // first = true
        assert!(!timer.is_switch_time()); // no time passed
    }

    #[test]
    fn returns_true_when_frame_advanced() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        timer.is_switch_time(); // initialize

        // sleep enough for at least one frame (~16ms for 60 FPS)
        sleep(std::time::Duration::from_millis(20));

        assert!(timer.is_switch_time());
    }

    // --- time_until_next_switch decisions ---

    #[test]
    fn time_until_next_switch_before_start_is_zero() {
        let timer = SwitchTimer::new(consts::FPS_60);

        assert_eq!(timer.time_until_next_switch(), Duration::ZERO);
    }

    #[test]
    fn time_until_next_switch_after_start_non_zero() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        timer.is_switch_time(); // start clock

        let remaining = timer.time_until_next_switch();

        // should be >= 0, usually > 0
        assert!(remaining >= Duration::ZERO);
    }

    #[test]
    fn time_until_next_switch_zero_when_behind() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        timer.is_switch_time();

        // wait long enough to exceed next frame
        sleep(std::time::Duration::from_millis(50));

        let remaining = timer.time_until_next_switch();

        assert_eq!(remaining, Duration::ZERO);
    }

    // --- reset behavior ---

    #[test]
    fn reset_restores_initial_behavior() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        timer.is_switch_time();
        timer.is_switch_time();

        timer.reset();

        // after reset, first call should again be true
        assert!(timer.is_switch_time());
    }

    // --- set_target_fps decisions ---

    #[test]
    fn set_target_fps_same_value_does_not_reset() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        timer.is_switch_time(); // start

        timer.set_target_fps(consts::FPS_60);

        // should NOT behave like reset
        assert!(!timer.is_switch_time());
    }

    #[test]
    fn set_target_fps_different_value_resets() {
        let mut timer = SwitchTimer::new(consts::FPS_60);

        timer.is_switch_time(); // start

        timer.set_target_fps(consts::FPS_30);

        // should behave like first call again
        assert!(timer.is_switch_time());
    }

    // --- target_fps getter ---

    #[test]
    fn target_fps_returns_current_value() {
        let timer = SwitchTimer::new(consts::FPS_60);

        assert_eq!(timer.target_fps(), consts::FPS_60);
    }
}
