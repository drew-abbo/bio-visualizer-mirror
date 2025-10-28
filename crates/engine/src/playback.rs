use crate::frame_store::FrameStore;
use std::time::{Duration, Instant};

pub struct Playback {
    cursor: Option<usize>,
    playing: bool,
    
    // time sync
    wall_start: Option<Instant>,
    pts_start: Option<Duration>,
    speed: f32,
}

impl Playback {
    pub fn new() -> Self {
        Self {
            cursor: None,
            playing: true,
            wall_start: None,
            pts_start: None,
            speed: 1.0,
        }
    }

    pub fn set_playing(&mut self, play: bool, store: &FrameStore) {
        self.playing = play;
        if play && self.cursor.is_none() {
            self.cursor = store.newest_index();
        }
        if play {
            self.wall_start = Some(Instant::now());
            self.pts_start = self.current_pts(store);
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn on_new_frame(&mut self, store: &FrameStore) {
        if self.playing {
            self.cursor = store.newest_index();
        } else if self.cursor.is_none() {
            self.cursor = store.newest_index();
        }
    }

    pub fn step_back(&mut self) {
        if let Some(i) = self.cursor {
            self.cursor = Some(i.saturating_sub(1));
        }
    }

    pub fn step_fwd(&mut self, store: &FrameStore) {
        if let Some(i) = self.cursor {
            let n = store.len();
            if i + 1 < n {
                self.cursor = Some(i + 1);
            }
        }
    }

    pub fn tick(&mut self, store: &FrameStore) {
        if !self.playing {
            return;
        }

        // Advance cursor based on wall clock and pts
        // future: handle speed changes
    }

    pub fn current_index(&self) -> Option<usize> {
        self.cursor
    }
    fn current_pts(&self, store: &FrameStore) -> Option<Duration> {
        self.cursor.and_then(|i| store.get(i)).and_then(|f| f.pts)
    }
}
