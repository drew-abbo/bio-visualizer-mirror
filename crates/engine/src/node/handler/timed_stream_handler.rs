use std::collections::HashSet;

use media::fps::Fps;

use crate::node_graph::EngineNodeId;

pub trait TimedStreamHandler {
    type Stream;

    fn for_each_stream_mut<F>(&mut self, f: F)
    where
        F: FnMut(EngineNodeId, &mut Self::Stream);

    fn set_paused_state(&mut self, paused: bool);
    fn is_paused_state(&self) -> bool;
    fn clear_stream_cache(&mut self);

    fn stream_pause(stream: &mut Self::Stream);
    fn stream_play(stream: &mut Self::Stream);
    fn stream_set_target_fps(stream: &mut Self::Stream, target_fps: Fps);

    fn pause_all_streams(&mut self) {
        self.set_paused_state(true);
        self.for_each_stream_mut(|_, stream| {
            Self::stream_pause(stream);
        });
    }

    fn play_all_streams(&mut self) {
        self.set_paused_state(false);
    }

    fn clear_cache(&mut self) {
        self.clear_stream_cache();
    }

    fn set_target_fps_all(&mut self, target_fps: Fps) {
        self.for_each_stream_mut(|_, stream| {
            Self::stream_set_target_fps(stream, target_fps);
        });
    }

    fn set_target_fps_for_nodes(&mut self, target_fps: Fps, active_nodes: &HashSet<EngineNodeId>) {
        self.for_each_stream_mut(|node_id, stream| {
            if active_nodes.contains(&node_id) {
                Self::stream_set_target_fps(stream, target_fps);
            }
        });
    }

    fn set_playback_for_nodes(&mut self, active_nodes: &HashSet<EngineNodeId>) {
        let paused = self.is_paused_state();

        self.for_each_stream_mut(|node_id, stream| {
            let should_play = !paused && active_nodes.contains(&node_id);
            if should_play {
                Self::stream_play(stream);
            } else {
                Self::stream_pause(stream);
            }
        });
    }
}
