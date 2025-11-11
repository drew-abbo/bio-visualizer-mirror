use crate::video_frame::{VideoFrame, View};
use engine::VideoPlayer;
use engine::renderer::{Renderer, FrameRenderer};

pub struct BioVisualizerMainWindow {
    output_frame: VideoFrame,
    player: Option<VideoPlayer>,
    renderer: Option<Renderer>,
}

impl Default for BioVisualizerMainWindow {
    fn default() -> Self {
        Self {
            output_frame: VideoFrame::default(),
            player: None,
            renderer: None,
        }
    }
}

impl BioVisualizerMainWindow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize renderer with wgpu from eframe
        let wgpu_render_state = cc.wgpu_render_state.as_ref().unwrap();
        let renderer = Renderer::new(
            &wgpu_render_state.device,
            wgpu_render_state.target_format,
        );
        
        // if let Err(e) = &renderer {
        //     eprintln!("Failed to create renderer: {}", e);
        // }
        
        let app = Self {
            output_frame: VideoFrame::default(),
            player: None,
            renderer: renderer.ok(),
        };
        
        app
    }
    
    pub fn load_video(&mut self, producer: media::frame::Producer) {
        self.player = Some(VideoPlayer::new(producer));
    }
    
    fn try_load_video(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Create a video stream from the file
        let stream = media::frame::streams::Video::new(path)?;
        
        // Create a producer with looping enabled
        let producer = media::frame::Producer::new(
            stream,
            media::frame::streams::OnStreamEnd::Loop
        )?;
        
        self.load_video(producer);
        Ok(())
    }
}

impl eframe::App for BioVisualizerMainWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Load Video").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("video", &["mp4", "mov", "avi", "mkv"])
                            .pick_file()
                        {
                            if let Err(e) = self.try_load_video(path.to_str().unwrap()) {
                                eprintln!("Failed to load video: {}", e);
                            } else if let Some(player) = &mut self.player {
                                player.play();
                            }
                        }
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                if let Some(player) = &mut self.player {
                    ui.separator();
                    if ui.button(if player.is_playing() { "⏸ Pause" } else { "▶ Play" }).clicked() {
                        player.toggle_play_pause();
                    }
                    if ui.button("⏭ Step").clicked() {
                        player.step_forward();
                    }
                    
                    ui.separator();
                    ui.label(format!("Time: {:.2}s", player.current_time().as_secs_f64()));
                    ui.label(format!("FPS: {:.1}", player.fps()));
                    
                    let stats = player.stats();
                    ui.label(format!("{}x{}", stats.dimensions.width(), stats.dimensions.height()));
                }
                
                ui.add_space(16.0);
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bio Visualizer");
            ui.separator();
            
            // Debug info
            ui.label(format!("Has player: {}", self.player.is_some()));
            ui.label(format!("Has renderer: {}", self.renderer.is_some()));
            
            if let Some(player) = &self.player {
                ui.label(format!("Is playing: {}", player.is_playing()));
                ui.label(format!("Has frame: {}", player.current_frame().is_some()));
            }
            
            ui.separator();
            
            // Update player and render frame
            if let (Some(player), Some(renderer)) = (&mut self.player, &mut self.renderer) {
                let did_update = player.update();
                let has_frame = player.current_frame().is_some();
                
                if did_update || has_frame {
                    if let Some(current_frame) = player.current_frame() {
                        let wgpu_render_state = frame.wgpu_render_state().unwrap();
                        
                        let texture_view = renderer.render_frame(
                            current_frame,
                            &wgpu_render_state.device,
                            &wgpu_render_state.queue,
                        );
                        
                        let dims = current_frame.dimensions();
                        self.output_frame.set_wgpu_texture(
                            wgpu_render_state,
                            &texture_view,
                            [dims.width() as usize, dims.height() as usize],
                        );
                    }
                }
                
                self.output_frame.ui(ui);
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("No Video Loaded");
                    ui.label("Click 'File → Load Video' to get started");
                });
            }
        });

        // Request continuous repaints for smooth playback
        if self.player.as_ref().map_or(false, |p| p.is_playing()) {
            ctx.request_repaint();
        }
    }
}