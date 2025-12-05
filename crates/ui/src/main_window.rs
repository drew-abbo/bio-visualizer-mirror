use crate::components::effects::node_blueprint::NodeBlueprint;
use crate::components::effects::node_select_list::NodeSelectList;
use crate::components::{PlaybackControls, VideoController, VideoFrame, View};

pub struct BioVisualizerMainWindow {
    video_frame: VideoFrame,
    video_controller: Option<VideoController>,
    effects_panel: NodeSelectList,
    node_blueprint: NodeBlueprint,
}

impl BioVisualizerMainWindow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wgpu_render_state = cc.wgpu_render_state.as_ref().unwrap();

        let video_controller =
            VideoController::new(&wgpu_render_state.device, wgpu_render_state.target_format).ok();

        Self {
            video_frame: VideoFrame::default(),
            video_controller,
            effects_panel: NodeSelectList::new(),
            node_blueprint: NodeBlueprint::new(),
        }
    }

    fn load_video_file(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let controller = self
            .video_controller
            .as_mut()
            .ok_or("Video controller not initialized")?;

        controller.load_video(path)?;

        // Auto-play after loading
        if let Some(player) = controller.player_mut() {
            player.play();
        }

        Ok(())
    }

    fn show_menu_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            // File menu
            ui.menu_button("File", |ui| {
                if ui.button("Load Video").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("video", &["mp4", "mov", "avi", "mkv"])
                        .pick_file()
                    {
                        if let Err(e) = self.load_video_file(path.to_str().unwrap()) {
                            eprintln!("Failed to load video: {}", e);
                        }
                    }
                }
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            if let Some(controller) = &mut self.video_controller {
                if let Some(player) = controller.player_mut() {
                    PlaybackControls::show(ui, player);
                }
            }

            ui.add_space(16.0);
            egui::widgets::global_theme_preference_buttons(ui);
        });
    }

    fn update_video_frame(&mut self, frame: &mut eframe::Frame) {
        let Some(controller) = &mut self.video_controller else {
            return;
        };

        let wgpu_render_state = frame.wgpu_render_state().unwrap();

        match controller.update_and_render(&wgpu_render_state.device, &wgpu_render_state.queue) {
            Ok(Some(result)) => {
                self.video_frame.set_wgpu_texture(
                    wgpu_render_state,
                    &result.texture_view,
                    result.size,
                );
            }
            Ok(None) => {
                // No frame available yet
            }
            Err(e) => {
                //should probably show an error in the UI instead and/or crash
                eprintln!("Failed to render frame: {}", e);
            }
        }
    }
}

impl eframe::App for BioVisualizerMainWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.show_menu_bar(ctx, ui);
        });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            self.node_blueprint.ui(ui);
        });


        // Might have to put these technically in the CentralPanel to get proper layering
        egui::SidePanel::left("left_panel")
            .default_width(300.0)
            .show(ctx, |ui| {
                self.effects_panel.ui(ui);
            });

        egui::SidePanel::right("right_panel")
            .default_width(400.0)
            .show(ctx, |ui| {
                self.update_video_frame(frame);
                if self
                    .video_controller
                    .as_ref()
                    .is_some_and(|c| c.has_video())
                {
                    self.video_frame.ui(ui);
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(100.0);
                        ui.heading("No Video Loaded");
                        ui.label("Click 'File → Load Video' to get started");
                    });
                }
            });

        // Request continuous repaints for smooth playback
        if self
            .video_controller
            .as_ref()
            .is_some_and(|c| c.is_playing())
        {
            ctx.request_repaint();
        }
    }
}
