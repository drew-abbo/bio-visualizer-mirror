//! Exports utilities for determining when you should switch frames.

use std::time::Instant;

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
        let Some(start_time) = self.start_time else {
            self.start_time = Some(Instant::now());
            return true;
        };

        let time_since_start = Instant::now().duration_since(start_time).as_secs_f64();
        let frame_interval = self.target_fps.interval_float();
        let frame_intervals_since_start = (time_since_start / frame_interval) as usize;

        if frame_intervals_since_start > self.frame_idx {
            self.frame_idx += 1;
            true
        } else {
            false
        }
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
