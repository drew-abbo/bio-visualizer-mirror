use super::editor_state_context::EditorStateContext;
use super::graph_executor_manager::GraphExecutorManager;
use super::node_graph::{NodeGraphState, NodeGraphViewer};
use super::snarl_style;
use crate::app_area::main_output::MainOutputArea;
use eframe;
use egui;
use engine::graph_executor::NodeValue;
use engine::node::NodeLibrary;
use media::fps::{Fps, SwitchTimer};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use util::ui::ErrorPopup;

// While dragging controls, coalesce graph re-exec requests to roughly one UI frame.
const GRAPH_INTERACTION_MIN_INTERVAL: Duration = Duration::from_millis(16);

/// Manages all editor-related state: node graph, output display, and playback
pub struct EditorArea {
    /// Local node graph used when no project is open
    local_node_graph: NodeGraphState,
    error_popup_queue: VecDeque<String>,
    executor_manager: GraphExecutorManager,
    node_library: Arc<NodeLibrary>,
    editor_state_context: EditorStateContext,
    displayed_frame: Option<NodeValue>,
    last_fps_output: Option<Fps>,
    playback_timer: Option<SwitchTimer>,
    // Last time we ran a graph execute triggered by graph edits.
    last_graph_execute_request: Instant,
    // Set when graph topology/parameters changed and a preview execute is pending.
    pending_graph_execute: bool,
    playback_enabled: bool,
    fps_override: Option<Fps>,
    last_warned_disconnected_selected_node: Option<engine::node_graph::EngineNodeId>,
    snarl_view_generation: u64,
    apply_saved_graph_zoom_once: bool,
    last_synced_content_hash: Option<u64>,
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
            playback_timer: None,
            last_graph_execute_request: Instant::now(),
            pending_graph_execute: false,
            playback_enabled: true,
            fps_override: None,
            last_warned_disconnected_selected_node: None,
            snarl_view_generation: 0,
            apply_saved_graph_zoom_once: true,
            last_synced_content_hash: None,
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

    fn set_playback_enabled(&mut self, enabled: bool) {
        if self.playback_enabled != enabled {
            self.playback_timer = None;
            if enabled {
                self.executor_manager.play_streams();
            } else {
                self.executor_manager.pause_streams();
            }
        }
        self.playback_enabled = enabled;
    }

    /// Returns true when the playback clock reached its next frame deadline.
    /// Also advances the internal deadline to the next frame boundary.
    fn playback_due(&mut self) -> bool {
        if !self.playback_enabled {
            self.playback_timer = None;
            return false;
        }

        let Some(target_fps) = self.fps_override.or(self.last_fps_output) else {
            self.playback_timer = None;
            return false;
        };

        let timer = self
            .playback_timer
            .get_or_insert_with(|| SwitchTimer::new(target_fps));
        timer.set_target_fps(target_fps);
        timer.is_switch_time()
    }

    fn schedule_next_playback_repaint(&self, ctx: &egui::Context) {
        if !self.playback_enabled {
            return;
        }

        let Some(timer) = &self.playback_timer else {
            return;
        };

        let wait = timer.time_until_next_switch();
        if wait.is_zero() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(wait);
        }
    }
}

impl EditorArea {
    /// Render the editor and synchronize the shared main output area.
    pub fn show_with_main_output(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        main_output: &mut MainOutputArea,
    ) {
        self.set_playback_enabled(main_output.playback_enabled());
        self.fps_override = main_output.fps_override();
        self.show(ctx, frame, main_output.preview_selected_node_enabled());
        self.sync_main_output(frame, main_output);
    }

    /// Render the entire editor area
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        preview_selected_node_enabled: bool,
    ) {
        // Render graph UI, then update preview/output from current selection.
        let selected_nodes = self.show_node_graph(ctx);
        let selected_snarl_node = self.update_output_selection(&selected_nodes);
        self.update_output_from_graph(
            ctx,
            frame,
            selected_snarl_node,
            preview_selected_node_enabled,
        );

        self.show_any_error_popups(ctx);
    }

    /// Push the latest cached output data into the app-owned output area.
    fn sync_main_output(&mut self, frame: &eframe::Frame, main_output: &mut MainOutputArea) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        main_output.update_from_editor(
            self.displayed_frame.as_ref(),
            self.fps_override.or(self.last_fps_output),
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
                    .id(egui::Id::new(("node_graph", self.snarl_view_generation)))
                    .style(snarl_style::snarl_style());

                let apply_saved_graph_zoom_once = self.apply_saved_graph_zoom_once;
                let mut reset_view_requested = false;
                {
                    let node_graph = self.active_node_graph_mut();
                    node_graph.ensure_output_sink();
                    viewer.set_initial_graph_view(
                        node_graph.graph_view,
                        node_graph.legacy_graph_view_zoom,
                        apply_saved_graph_zoom_once,
                    );
                    snarl_widget.show(&mut node_graph.snarl, &mut viewer, ui);
                    node_graph.graph_view = viewer.latest_graph_view();
                    node_graph.legacy_graph_view_zoom = None;

                    if viewer.take_reset_view_requested() {
                        node_graph.graph_view = None;
                        node_graph.legacy_graph_view_zoom = None;
                        reset_view_requested = true;
                    }
                }

                self.apply_saved_graph_zoom_once = false;
                if reset_view_requested {
                    self.snarl_view_generation = self.snarl_view_generation.wrapping_add(1);
                    self.apply_saved_graph_zoom_once = true;
                    self.editor_state_context.mark_edited();
                }

                selected_nodes = snarl_widget.get_selected_nodes(ui);
                pending_errors = viewer.take_pending_errors();
            });

        for error in pending_errors {
            self.error_popup_queue.push_back(error);
        }

        // Sync to engine only when graph content has changed.
        let has_project = self.editor_state_context.has_open_project();
        let current_content_hash = if has_project {
            self.editor_state_context
                .node_graph()
                .and_then(EditorStateContext::compute_content_hash)
        } else {
            EditorStateContext::compute_content_hash(&self.local_node_graph)
        };

        let should_sync = self.last_synced_content_hash != current_content_hash;

        // Then sync to engine (after UI to avoid multiple borrows)
        // Check if we have a project graph or use local graph
        let graph_changed = if should_sync {
            if has_project {
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
            }
        } else {
            false
        };
        self.last_synced_content_hash = current_content_hash;

        // Mark executor as changed if graph sync made changes
        if graph_changed {
            self.executor_manager.mark_graph_changed();
        }

        // Node position/layout edits don't always change engine graph wiring.
        // Hash-check while interacting with the graph so layout-only edits still mark dirty.
        let is_interacting_with_graph = ctx.input(|i| {
            i.pointer.any_down()
                || i.pointer.any_released()
                || i.raw_scroll_delta != egui::Vec2::ZERO
                || (i.zoom_delta() - 1.0).abs() > f32::EPSILON
        });

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
        if selected_nodes.is_empty() {
            None
        } else {
            selected_nodes.last().copied()
        }
    }

    fn update_output_from_graph(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        selected_snarl_node: Option<egui_snarl::NodeId>,
        preview_selected_node_enabled: bool,
    ) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        // Get the snarl node's associated engine node id
        let (selected_engine_node, output_source_engine_node) = {
            let node_graph = self.active_node_graph_mut();
            let selected_engine_node =
                selected_snarl_node.and_then(|snarl_id| node_graph.snarl[snarl_id].engine_node_id);
            let output_source_engine_node = node_graph
                .output_source_snarl_node()
                .and_then(|snarl_id| node_graph.snarl[snarl_id].engine_node_id);

            (selected_engine_node, output_source_engine_node)
        };

        self.executor_manager
            .set_output_source_engine_node(output_source_engine_node);

        if !preview_selected_node_enabled {
            if let (Some(selected), Some(output)) =
                (selected_engine_node, output_source_engine_node)
            {
                if !self
                    .executor_manager
                    .node_in_output_subgraph(selected, output)
                {
                    if self.last_warned_disconnected_selected_node != Some(selected) {
                        self.error_popup_queue.push_back(
                            "Selected node is outside the output-connected graph. Enable 'Preview Selected Node' to view it."
                                .to_string(),
                        );
                        self.last_warned_disconnected_selected_node = Some(selected);
                    }
                } else {
                    self.last_warned_disconnected_selected_node = None;
                }
            } else {
                self.last_warned_disconnected_selected_node = None;
            }
        } else {
            self.last_warned_disconnected_selected_node = None;
        }

        let preview_target_node = if preview_selected_node_enabled {
            selected_engine_node
        } else {
            None
        };

        let selection_changed = self.executor_manager.selection_changed(preview_target_node);
        let graph_changed = self.executor_manager.consume_graph_changed();

        if selection_changed {
            self.executor_manager
                .set_last_selected_engine_node(preview_target_node);
            self.last_fps_output = None;
            self.pending_graph_execute = false;
        }

        let has_nodes = !self.executor_manager.engine_graph().is_empty();
        if !has_nodes {
            self.displayed_frame = None;
            self.last_fps_output = None;
            self.playback_timer = None;
            self.pending_graph_execute = false;
            return;
        }

        let node_to_execute = preview_target_node.or(output_source_engine_node);

        let Some(node_to_execute) = node_to_execute else {
            self.displayed_frame = None;
            self.last_fps_output = None;
            self.playback_timer = None;
            self.pending_graph_execute = false;
            return;
        };

        if self.last_fps_output.is_none() || selection_changed || graph_changed {
            self.last_fps_output = self
                .executor_manager
                .get_target_fps_for_display_node(&self.node_library, node_to_execute);
        }

        if let Some(global_fps) = self.fps_override.or(self.last_fps_output) {
            self.executor_manager
                .set_global_stream_target_fps(global_fps);
        }

        if graph_changed {
            self.pending_graph_execute = true;
        }

        // Pointer-down usually means slider drags or active graph interaction.
        let is_dragging_graph = ctx.input(|i| i.pointer.any_down());

        // Graph edits execute immediately when interaction ends, but are throttled
        // while dragging to keep UI input responsive.
        let graph_execute_due = if selection_changed {
            true
        } else if self.pending_graph_execute {
            if !is_dragging_graph {
                true
            } else {
                self.last_graph_execute_request.elapsed() >= GRAPH_INTERACTION_MIN_INTERVAL
            }
        } else {
            false
        };

        let should_advance = self.playback_due();
        let should_execute = graph_execute_due || self.displayed_frame.is_none() || should_advance;

        if should_execute {
            match self.executor_manager.execute(
                &self.node_library,
                render_state,
                Some(node_to_execute),
            ) {
                Ok(Some(frame_output)) => {
                    self.displayed_frame = Some(frame_output);
                }
                Ok(None) => {
                    // No frame output available
                }
                Err(engine::graph_executor::ExecutionError::VideoStreamNotReady(_)) => {
                    // Video still warming up, expected case
                }
                Err(err) => {
                    util::debug_log_error!("Graph execution error: {}", err);
                }
            }

            if graph_execute_due {
                // Mark the queued graph update as handled even if execution failed,
                // otherwise we can get stuck in a tight retry/logging loop.
                self.pending_graph_execute = false;
                self.last_graph_execute_request = Instant::now();
            }
        }

        if self.pending_graph_execute {
            if is_dragging_graph {
                let elapsed = self.last_graph_execute_request.elapsed();
                let wait = GRAPH_INTERACTION_MIN_INTERVAL.saturating_sub(elapsed);
                if wait.is_zero() {
                    ctx.request_repaint();
                } else {
                    ctx.request_repaint_after(wait);
                }
            } else {
                // Apply queued graph changes immediately once dragging stops.
                ctx.request_repaint();
            }
        }

        if self.playback_enabled {
            self.schedule_next_playback_repaint(ctx);
        }
    }
}

impl ErrorPopup<String> for EditorArea {
    fn error_queue_mut(&mut self) -> &mut VecDeque<String> {
        &mut self.error_popup_queue
    }
}
