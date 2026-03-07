//! Exports [BufferingSuggestor].

use std::time::{Duration, Instant};

use crate::fps::Fps;

/// Used to suggest how many items a data producer should be buffering
/// (queueing) ahead of time to keep a safe but not excessive lead on a data
/// consumer based on a target consumption speed (`dest_fps`) and time samples.
#[derive(Debug)]
pub struct BufferingSuggestor {
    consumer_interval: f64,
    count: u64,
    mean_production_time: f64,
    production_time_variance: f64,
    emergency_multipler: f64,
}

impl BufferingSuggestor {
    /// Create a [BufferingSuggestor].
    ///
    /// `dest_fps` is the number of times per second data will be fetched from
    /// the queue.
    ///
    /// As a conservative starting guess, this function assumes it takes 80% as
    /// long to produce data as it does to consume it. Also see
    /// [Self::with_time_guess].
    pub fn new(dest_fps: Fps) -> Self {
        // We're going with a very conservative guess that it will take 80% as
        // long to produce data as to consume it.
        let time_guess = Duration::from_secs_f64(dest_fps.inverse().as_float() * 0.8);

        Self::with_time_guess(dest_fps, time_guess)
    }

    /// The same as [Self::new] except you can provide a guess as to how long it
    /// will take to produce a piece of data.
    pub const fn with_time_guess(dest_fps: Fps, time_guess: Duration) -> Self {
        Self {
            consumer_interval: dest_fps.inverse().as_float(),
            count: 0,
            mean_production_time: time_guess.as_secs_f64(),
            production_time_variance: 0.0,
            emergency_multipler: 1.0,
        }
    }

    /// Suggest how many data items to buffer ahead of time based on past time
    /// samples and the consumer interval (target frame rate).
    pub fn buffering_suggestion(&self) -> usize {
        let base_items = self.mean_production_time / self.consumer_interval;

        const SAFETY_FACTOR: f64 = 3.0;
        let safety_items = SAFETY_FACTOR * self.time_std_dev() / self.consumer_interval;

        ((base_items + safety_items) * self.emergency_multipler).ceil() as usize
    }

    /// Update the suggestor with the amount of time it took to produce the last
    /// piece of data.
    pub fn add_time_sample(&mut self, production_time: Duration) {
        self.count += 1;

        let production_time = production_time.as_secs_f64();

        if self.count == 1 {
            self.mean_production_time = production_time;
            self.production_time_variance = 0.0;
        } else {
            // Rolling average and variance.
            let delta = production_time - self.mean_production_time;
            self.mean_production_time += delta / (self.count as f64);
            self.production_time_variance += delta * (production_time - self.mean_production_time);
        }

        // If we're suddenly producing data nearly slower than we're consuming
        // it we'll immediately spike the buffer size to avoid waiting on the
        // queue at all.
        if production_time * 1.1 > self.consumer_interval {
            const EMERGENCY_SPIKE: f64 = 4.0;
            self.emergency_multipler = EMERGENCY_SPIKE;
        } else if self.emergency_multipler > 1.0 {
            // Decay back towards 1 when we aren't in an emergency.
            self.emergency_multipler = (self.emergency_multipler * 0.9).max(1.0);
        }
    }

    /// Times the execution of `f` and updates the suggestor with the amount of
    /// time it took to run (see [Self::add_time_sample]).
    #[inline(always)]
    pub fn run_timed_and_sampled<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let (elapsed, ret) = Self::run_timed(f);
        self.add_time_sample(elapsed);
        ret
    }

    /// Update the number of times per second data will be fetched from the
    /// queue.
    pub fn set_dest_fps(&mut self, dest_fps: Fps) {
        self.consumer_interval = dest_fps.inverse().as_float();
    }

    /// Times `f` and returns how long it took.
    #[inline(always)]
    pub fn run_timed<F, R>(f: F) -> (Duration, R)
    where
        F: FnOnce() -> R,
    {
        let start_time = Instant::now();
        let ret = f();
        (start_time.elapsed(), ret)
    }

    fn time_std_dev(&self) -> f64 {
        if self.count > 1 {
            (self.production_time_variance / ((self.count - 1) as f64)).sqrt()
        } else {
            0.0
        }
    }
}
