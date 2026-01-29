use crate::components::FrameDisplay;
use crate::view::View;
use super::playback_controls::PlaybackControls;
use egui_node_editor::NodeId;
use engine::graph_executor::OutputValue;
use media::frame::Uid;

/// Manages the output panel display state and behavior
pub struct OutputPanel {
    frame_display: FrameDisplay,
    is_open: bool,
    is_docked: bool,
    window_size: egui::Vec2,
    
    // Output tracking
    selected_node_id: Option<NodeId>,
    playback_controls: PlaybackControls,
    current_output: Option<OutputValue>,
}

impl OutputPanel {
    pub fn new() -> Self {
        Self {
            frame_display: FrameDisplay::default_config(),
            is_open: true,
            is_docked: false,
            window_size: egui::vec2(640.0, 480.0),
            selected_node_id: None,
            playback_controls: PlaybackControls::new(),
            current_output: None,
        }
    }

    /// Set whether the panel is docked (bottom panel) or floating (window)
    #[allow(dead_code)]
    pub fn set_docked(&mut self, docked: bool) {
        self.is_docked = docked;
    }

    /// Toggle between docked and floating
    #[allow(dead_code)]
    pub fn toggle_dock(&mut self) {
        self.is_docked = !self.is_docked;
    }

    /// Set whether the panel is visible
    #[allow(dead_code)]
    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    /// Toggle visibility
    #[allow(dead_code)]
    pub fn toggle_visibility(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Get reference to the underlying frame display for texture updates
    #[allow(dead_code)]
    pub fn frame_display(&self) -> &FrameDisplay {
        &self.frame_display
    }

    /// Get mutable reference to the underlying frame display
    #[allow(dead_code)]
    pub fn frame_display_mut(&mut self) -> &mut FrameDisplay {
        &mut self.frame_display
    }

    /// Check if the panel is currently visible
    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Check if the panel is docked
    #[allow(dead_code)]
    pub fn is_docked(&self) -> bool {
        self.is_docked
    }

    /// Set the window size when floating
    #[allow(dead_code)]
    pub fn set_window_size(&mut self, size: egui::Vec2) {
        self.window_size = size;
    }

    /// Set the currently selected output node
    pub fn set_selected_node(&mut self, node_id: Option<NodeId>) {
        if self.selected_node_id != node_id {
            self.selected_node_id = node_id;
            // Reset playback when switching nodes
            self.playback_controls.reset();
        }
    }

    /// Set the current output value to display
    pub fn set_output_value(&mut self, output: OutputValue) {
        self.current_output = Some(output);
    }

    /// Clear the current output value
    pub fn clear_output(&mut self) {
        self.current_output = None;
    }

    /// Get the currently selected node ID
    #[allow(dead_code)]
    pub fn selected_node(&self) -> Option<NodeId> {
        self.selected_node_id
    }

    /// Get mutable reference to playback controls
    #[allow(dead_code)]
    pub fn playback_controls_mut(&mut self) -> &mut PlaybackControls {
        &mut self.playback_controls
    }

    /// Update the displayed frame from output value
    pub fn set_output_frame(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        output: &OutputValue,
    ) {
        match output {
            OutputValue::Frame(gpu_frame) => {
                // Generate a unique ID for this frame
                let frame_id = Uid::generate_new();
                let size = [gpu_frame.size.width as usize, gpu_frame.size.height as usize];
                self.frame_display.set_wgpu_texture_if_changed(
                    render_state,
                    gpu_frame.view(),
                    size,
                    frame_id,
                );
            }
            _ => {
                // For non-frame outputs, clear the display
                self.frame_display.clear(Some(render_state));
            }
        }
    }

    /// Clear the displayed frame
    pub fn clear_frame(&mut self, render_state: Option<&egui_wgpu::RenderState>) {
        self.frame_display.clear(render_state);
    }

    /// Get reference to playback controls
    #[allow(dead_code)]
    pub fn playback_controls(&self) -> &PlaybackControls {
        &self.playback_controls
    }

    /// Render as a docked bottom panel
    fn show_as_dock(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("output_panel")
            .resizable(true)
            .default_height(300.0)
            .show(ctx, |ui| {
                self.render_panel_content(ui);
            });
    }

    /// Render as a floating window
    fn show_as_window(&mut self, ctx: &egui::Context) {
        let mut open = self.is_open;
        let mut docked = self.is_docked;

        egui::Window::new("Output")
            .open(&mut open)
            .default_size(self.window_size)
            .resizable(true)
            .show(ctx, |ui| {
                self.render_window_controls(ui, &mut docked);
                ui.separator();
                self.render_panel_content(ui);
            });

        self.is_open = open;
        if docked {
            self.is_docked = true;
        }
    }

    /// Render the dock/float toggle and other controls
    fn render_window_controls(&self, ui: &mut egui::Ui, docked: &mut bool) {
        ui.horizontal(|ui| {
            if ui.button("📌 Dock").clicked() {
                *docked = true;
            }
            if ui.button("🗖 Undock").clicked() {
                *docked = false;
            }
            
            // Show selected node info
            if let Some(node_id) = self.selected_node_id {
                ui.separator();
                ui.label(format!("Node: {:?}", node_id));
            } else {
                ui.separator();
                ui.label("No node selected");
            }
        });
    }

    /// Render the main panel content with playback controls
    fn render_panel_content(&mut self, ui: &mut egui::Ui) {
        // Playback controls
        self.playback_controls.ui(ui);
        ui.separator();
        
        // Display output value
        if let Some(ref output) = self.current_output {
            match output {
                OutputValue::Frame(gpu_frame) => {
                    ui.label(format!("Frame: {}x{}", gpu_frame.size.width, gpu_frame.size.height));
                    // Frame display
                    egui::Frame::NONE.show(ui, |ui| {
                        self.frame_display.render_content(ui);
                    });
                }
                OutputValue::Bool(val) => {
                    ui.label(format!("Bool: {}", val));
                }
                OutputValue::Int(val) => {
                    ui.label(format!("Int: {}", val));
                }
                OutputValue::Float(val) => {
                    ui.label(format!("Float: {}", val));
                }
                OutputValue::Dimensions(w, h) => {
                    ui.label(format!("Dimensions: {}x{}", w, h));
                }
                OutputValue::Pixel(rgba) => {
                    ui.label(format!("Pixel: RGBA({}, {}, {}, {})", rgba[0], rgba[1], rgba[2], rgba[3]));
                }
                OutputValue::Text(text) => {
                    ui.label(format!("Text: {}", text));
                }
            }
        } else {
            ui.label("No output available");
        }
    }
}

impl View for OutputPanel {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.frame_display.ui(ui);
    }
}

impl OutputPanel {
    /// Show the output panel - handles both docked and floating modes
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        if self.is_docked {
            self.show_as_dock(ctx);
        } else {
            self.show_as_window(ctx);
        }
    }
}
