use engine::graph_executor::{ExecutionContext, NodeValue};
use engine::node::NodeLibrary;
use engine::node_graph::EngineNodeId;
use std::collections::HashMap;

use super::graph_executor_manager::GraphExecutorManager;
use super::output_panel::OutputPanel;
use super::playback_controls::PlaybackControls;
use super::playback_state::PlaybackState;

pub struct OutputController;

impl OutputController {
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        ctx: &util::egui::Context,
        output_panel: &mut OutputPanel,
        playback_controls: &PlaybackControls,
        playback_state: &mut PlaybackState,
        executor_manager: &mut GraphExecutorManager,
        node_library: &NodeLibrary,
        render_state: &egui_wgpu::RenderState,
        selected_engine_node: Option<EngineNodeId>,
    ) {
        let is_playing = playback_controls.is_playing();
        let selection_changed = executor_manager.selection_changed(selected_engine_node);
<<<<<<< HEAD
<<<<<<< HEAD
        let graph_changed = executor_manager.consume_graph_changed();
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
=======
        let graph_changed = executor_manager.consume_graph_changed();
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)

        // Reset playback state on selection change
        if selection_changed {
            playback_state.reset();
            executor_manager.set_last_selected_engine_node(selected_engine_node);
        }

        if is_playing {
            ctx.request_repaint();
        }

        // Resolve sampling rate from controls
        let sampling_rate_hz = playback_controls.sampling_rate().resolve(30.0); // Default FPS, will be updated from output

        playback_state.update_tick(is_playing, sampling_rate_hz);

        let has_nodes = !executor_manager.engine_graph().is_empty();

        if !has_nodes {
            output_panel.reset();
        }

        let should_advance = if is_playing {
            playback_state.should_advance_frame(sampling_rate_hz)
        } else {
            false
        };

        // Determine if we should execute the graph
<<<<<<< HEAD
<<<<<<< HEAD
        let should_execute = has_nodes && (selection_changed || should_advance || graph_changed);
=======
        let should_execute = has_nodes && (selection_changed || should_advance);
>>>>>>> a665ac9 (commit now so I don't screw something up)
=======
        let should_execute = has_nodes && (selection_changed || should_advance || graph_changed);
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)

        let context = ExecutionContext {
            timeline_time_secs: playback_state.timeline_time_secs(sampling_rate_hz),
            sampling_rate_hz,
            advance_frame: should_advance,
        };

        // Execute and get outputs if needed
        let outputs = if should_execute {
            ctx.request_repaint();
            Self::execute_and_get_outputs(
                executor_manager,
                node_library,
                render_state,
                selected_engine_node,
                context,
            )
        } else {
            None
        };

        // Display output if we have it, otherwise keep showing the last output
        if let Some(outputs) = outputs {
            Self::apply_outputs(output_panel, render_state, &outputs);
        }
        // Note: We don't clear output when paused - keeps the last frame visible
    }

    fn execute_and_get_outputs(
        executor_manager: &mut GraphExecutorManager,
        node_library: &NodeLibrary,
        render_state: &egui_wgpu::RenderState,
        selected_engine_node: Option<EngineNodeId>,
        context: ExecutionContext,
    ) -> Option<HashMap<String, NodeValue>> {
        let node_to_execute =
            selected_engine_node.unwrap_or_else(|| executor_manager.find_display_node());

        executor_manager.execute(node_library, render_state, Some(node_to_execute), context)
    }

    fn apply_outputs(
        output_panel: &mut OutputPanel,
        render_state: &egui_wgpu::RenderState,
        outputs: &HashMap<String, NodeValue>,
    ) {
        // Find the Frame output to display
        let frame_output = outputs
            .iter()
            .find(|(_, value)| matches!(value, NodeValue::Frame(_)))
            .map(|(_, value)| value.clone());

        if let Some(output_value) = frame_output {
            output_panel.set_output_value(output_value.clone());
            output_panel.set_output_frame(render_state, &output_value);
        }

        // Find FPS output - look for Float outputs that might be FPS
        // Check output names that suggest FPS (case-insensitive)
        let fps_output = outputs
            .iter()
            .find(|(name, value)| {
                matches!(value, NodeValue::Float(_)) && name.to_lowercase().contains("fps")
            })
            .and_then(|(_, value)| {
                if let NodeValue::Float(fps) = value {
                    Some(*fps)
                } else {
                    None
                }
            });

        if let Some(fps) = fps_output {
            output_panel.set_playback_fps(fps as f64);
        }
    }
}
