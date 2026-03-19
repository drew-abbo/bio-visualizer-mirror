use super::editor_state_context::EditorStateContext;
use super::graph_executor_manager::GraphExecutorManager;
use super::node_graph::{NodeGraphState, NodeGraphViewer};
use super::snarl_style;
use crate::app_area::main_output::MainOutputArea;
use engine::graph_executor::NodeValue;
use engine::node::NodeLibrary;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use util::eframe;
use util::egui;
use util::ui::ErrorPopup;

const GRAPH_PREVIEW_MIN_INTERVAL: Duration = Duration::from_millis(16);

/// Manages all editor-related state: node graph, output display, and playback
pub struct EditorArea {
    /// Local node graph used when no project is open
    local_node_graph: NodeGraphState,
    error_popup_queue: VecDeque<String>,
    executor_manager: GraphExecutorManager,
    node_library: Arc<NodeLibrary>,
    editor_state_context: EditorStateContext,
    displayed_frame: Option<NodeValue>,
    last_fps_output: Option<f64>,
    last_playback_request: Instant,
    playback_accumulator: Duration,
    last_playing: bool,
    last_execute_request: Instant,
    pending_graph_preview: bool,
}

impl EditorArea {
    pub fn new() -> Self {
        let node_library = match NodeLibrary::load_all() {
            Ok(lib) => Arc::new(lib),
            Err(err) => {
                util::debug_log_error!("Failed to load node library: {:?}", err);
                Arc::new(NodeLibrary::default())
            }
        };

        Self {
            local_node_graph: NodeGraphState::new(),
            error_popup_queue: VecDeque::default(),
            executor_manager: GraphExecutorManager::new(),
            node_library,
            editor_state_context: EditorStateContext::new(),
            displayed_frame: None,
            last_fps_output: None,
            last_playback_request: Instant::now(),
            playback_accumulator: Duration::ZERO,
            last_playing: false,
            last_execute_request: Instant::now(),
            pending_graph_preview: false,
        }
    }

    /// Get the active node graph (from project if open, otherwise local)
    fn active_node_graph_mut(&mut self) -> &mut NodeGraphState {
        self.editor_state_context
            .node_graph_mut()
            .unwrap_or(&mut self.local_node_graph)
    }

    /// Access to the editor state context for project operations
    pub fn editor_state_context_mut(&mut self) -> &mut EditorStateContext {
        &mut self.editor_state_context
    }
}

impl EditorArea {
    /// Render the entire editor area
    pub fn show(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        // show the node graph and get selected nodes
        // feed selected node into output panel to update its content
        let selected_nodes = self.show_node_graph(ctx);
        let selected_snarl_node = self.update_output_selection(&selected_nodes);
        self.update_output_from_graph(ctx, frame, selected_snarl_node);

        self.show_any_error_popups(ctx);
    }

    /// Push the latest cached output data into the app-owned output area.
    pub fn sync_main_output(&mut self, frame: &eframe::Frame, main_output: &mut MainOutputArea) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        main_output.update_from_editor(
            self.displayed_frame.as_ref(),
            self.last_fps_output,
            render_state,
        );
    }

    pub fn save_state(&mut self) {
        match self.editor_state_context.save() {
            Ok(true) => {
                util::debug_log_info!("Project saved successfully");
            }
            Ok(false) => {
                util::debug_log_info!("No changes to save");
            }
            Err(e) => {
                util::debug_log_error!("Failed to save project: {}", e);
                self.error_popup_queue
                    .push_back(format!("Failed to save project: {}", e));
            }
        }
    }

    fn show_node_graph(&mut self, ctx: &egui::Context) -> Vec<egui_snarl::NodeId> {
        let mut selected_nodes = Vec::new();
        let mut pending_errors = Vec::new();

        // First, render the UI
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(16, 20, 22)))
            .show(ctx, |ui| {
                let mut viewer = NodeGraphViewer::new(self.node_library.clone());

                let snarl_widget = egui_snarl::ui::SnarlWidget::new()
                    .id(egui::Id::new("node_graph"))
                    .style(snarl_style::snarl_style());

                let node_graph = self.active_node_graph_mut();
                snarl_widget.show(&mut node_graph.snarl, &mut viewer, ui);
                selected_nodes = snarl_widget.get_selected_nodes(ui);
                pending_errors = viewer.take_pending_errors();
            });

        for error in pending_errors {
            self.error_popup_queue.push_back(error);
        }

        // Then sync to engine (after UI to avoid multiple borrows)
        // Check if we have a project graph or use local graph
        let has_project = self.editor_state_context.has_open_project();

        let graph_changed = if has_project {
            // Sync project graph to engine
            let project_graph = self.editor_state_context.node_graph_mut().unwrap();
            project_graph.sync_to_engine(
                self.executor_manager.engine_graph_mut_no_flag(),
                &self.node_library,
            )
        } else {
            // Sync local graph to engine
            self.local_node_graph.sync_to_engine(
                self.executor_manager.engine_graph_mut_no_flag(),
                &self.node_library,
            )
        };

        // Mark executor as changed if graph sync made changes
        if graph_changed {
            self.executor_manager.mark_graph_changed();
        }

        // Node position/layout edits don't always change engine graph wiring.
        // Hash-check while interacting with the graph so layout-only edits still mark dirty.
        let is_interacting_with_graph =
            ctx.input(|i| i.pointer.any_down() || i.pointer.any_released());

        // Mark project edited using state hash comparison when graph semantics changed
        // or while interacting (to catch layout-only edits like node moves).
        if has_project
            && (graph_changed || is_interacting_with_graph)
            && let Some(current_state) = self.editor_state_context.node_graph_mut()
            && let Some(current_hash) = EditorStateContext::compute_state_hash(current_state)
        {
            self.editor_state_context.check_hash_changed(current_hash);
        }

        selected_nodes
    }

    fn update_output_selection(
        &mut self,
        selected_nodes: &[egui_snarl::NodeId],
    ) -> Option<egui_snarl::NodeId> {
        let selected_snarl_node = if selected_nodes.is_empty() {
            None
        } else {
            selected_nodes.last().copied()
        };
        selected_snarl_node
    }

    fn update_output_from_graph(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        selected_snarl_node: Option<egui_snarl::NodeId>,
    ) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        // Get the snarl node's associated engine node id
        let node_graph = self.active_node_graph_mut();
        let selected_engine_node =
            selected_snarl_node.and_then(|snarl_id| node_graph.snarl[snarl_id].engine_node_id);

        let selection_changed = self
            .executor_manager
            .selection_changed(selected_engine_node);
        let graph_changed = self.executor_manager.consume_graph_changed();

        if selection_changed {
            self.executor_manager
                .set_last_selected_engine_node(selected_engine_node);
            self.pending_graph_preview = false;
            self.last_fps_output = None;
        }

        let has_nodes = !self.executor_manager.engine_graph().is_empty();
        if !has_nodes {
            self.displayed_frame = None;
            self.last_fps_output = None;
            self.playback_accumulator = Duration::ZERO;
            self.last_playing = false;
            return;
        }

        let node_to_execute =
            selected_engine_node.unwrap_or_else(|| self.executor_manager.find_display_node());

        if self.last_fps_output.is_none() || selection_changed || graph_changed {
            self.last_fps_output = self
                .executor_manager
                .get_target_fps_for_display_node(&self.node_library, node_to_execute);
        }

        self.update_playback_tick();

        let should_advance = self.should_advance_frame();

        if graph_changed {
            self.pending_graph_preview = true;
        }

        // Coalesce rapid UI drags (e.g., brightness slider) to avoid executing on every repaint.
        let graph_preview_due = self.pending_graph_preview
            && !selection_changed
            && !should_advance
            && self.last_execute_request.elapsed() >= GRAPH_PREVIEW_MIN_INTERVAL;

        let should_execute = selection_changed
            || graph_preview_due
            || self.displayed_frame.is_none()
            || should_advance;

        if !should_execute {
            if self.displayed_frame.is_some() {
                self.request_next_repaint(ctx);
            }
            return;
        }

        let context = engine::graph_executor::ExecutionContext {
            // Selection changes should not stall playback for one tick.
            advance_frame: should_advance || selection_changed,
        };

        if let Some(outputs) = self.executor_manager.execute(
            &self.node_library,
            render_state,
            Some(node_to_execute),
            context,
        ) {
            self.last_execute_request = Instant::now();
            self.pending_graph_preview = false;

            let frame_output = outputs.values().find_map(|value| {
                if let NodeValue::Frame(_) = value {
                    Some(value.clone())
                } else {
                    None
                }
            });

            if let Some(frame_output) = frame_output {
                self.displayed_frame = Some(frame_output);
            }

            if self.displayed_frame.is_some() {
                self.request_next_repaint(ctx);
            }
        }
    }

    fn should_advance_frame(&mut self) -> bool {
        let Some(fps) = self.last_fps_output.filter(|fps| *fps > 0.0) else {
            return false;
        };

        let frame_duration = Duration::from_secs_f64(1.0 / fps);
        if self.playback_accumulator >= frame_duration {
            self.playback_accumulator -= frame_duration;
            true
        } else {
            false
        }
    }

    fn request_next_repaint(&self, ctx: &egui::Context) {
        if let Some(fps) = self.last_fps_output.filter(|fps| *fps > 0.0) {
            ctx.request_repaint_after(Duration::from_secs_f64(1.0 / fps));
        } else {
            // Unknown FPS (e.g., static image chain): avoid max-rate repaint loops.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn update_playback_tick(&mut self) {
        let Some(fps) = self.last_fps_output.filter(|fps| *fps > 0.0) else {
            self.last_playing = false;
            return;
        };

        // Reset accumulator when (re)starting playback.
        if !self.last_playing {
            self.last_playback_request = Instant::now();
            self.playback_accumulator = Duration::ZERO;
        }

        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_playback_request);
        self.last_playback_request = now;
        self.playback_accumulator += dt;

        // Clamp to one frame to avoid bursty catch-up behavior under load.
        let frame_duration = Duration::from_secs_f64(1.0 / fps);
        if self.playback_accumulator > frame_duration {
            self.playback_accumulator = frame_duration;
        }

        self.last_playing = true;
    }
}

impl ErrorPopup<String> for EditorArea {
    fn error_queue_mut(&mut self) -> &mut VecDeque<String> {
        &mut self.error_popup_queue
    }
}
