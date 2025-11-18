use crate::components::View;
use crate::components::video_frame::VideoFrame;
use engine::core::Engine;
use media::video_player::VideoPlayer;

#[derive(Default)]
pub struct BioVisualizerMainWindow {
    video_frame: VideoFrame,
    player: Option<VideoPlayer>,
    engine: Option<Engine>,
}

impl BioVisualizerMainWindow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wgpu_render_state = cc.wgpu_render_state.as_ref().unwrap();

        let engine = Engine::new(wgpu_render_state.target_format);

        // Handle engine initialization errors later

        Self {
            video_frame: VideoFrame::default(),
            player: None,
            engine: Some(engine),
        }
    }

    pub fn load_video(&mut self, producer: media::frame::Producer) {
        self.player = Some(VideoPlayer::new(producer));
    }

    fn try_load_video(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Create a video stream from the file
        let stream = media::frame::streams::Video::new(path)?;

        // Create a producer with looping enabled
        let producer =
            media::frame::Producer::new(stream, media::frame::streams::OnStreamEnd::Loop)?;

        self.load_video(producer);
        Ok(())
    }
}

impl eframe::App for BioVisualizerMainWindow {
    fn update(&mut self, ctx: &egui::Context, eframe: &mut eframe::Frame) {
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
                    if ui
                        .button(if player.is_playing() { "⏸ Pause" } else { "▶ Play" })
                        .clicked()
                    {
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

            // Main rendering loop with synchronous engine
            if let (Some(player), Some(engine)) = (&mut self.player, &mut self.engine) {
                // Grab wgpu state once
                let wgpu_render_state = eframe.wgpu_render_state().unwrap();
                let device = &wgpu_render_state.device;
                let queue = &wgpu_render_state.queue;

                // 1. Fetch next frame from player (decoder runs in producer thread)
                if player.update() {
                    if let Some(frame) = player.take_current_frame() {
                        let dims = frame.dimensions();
                        let width = dims.width();
                        let height = dims.height();

                        // Ensure UI texture exists and matches frame size
                        let recreate = match self.video_frame.texture() {
                            Some(tex) => {
                                let [w, h] = self.video_frame.texture_size();
                                w as u32 != width || h as u32 != height
                            }
                            None => true,
                        };

                        if recreate {
                            let texture = create_ui_texture(
                                device,
                                width,
                                height,
                                wgpu_render_state.target_format,
                            );
                            self.video_frame.set_texture(texture);
                        }

                        // We cloned earlier UI-owned texture; get a handle to pass to engine
                        let ui_texture = self.video_frame.texture().unwrap().clone();

                        // Synchronously render into UI texture using engine
                        let result = engine.render_frame(&frame, device, queue, &ui_texture);

                        // Recycle the frame back to the producer (important!)
                        player.recycle_frame(frame);

                        // Handle the engine result
                        match result {
                            engine::core::EngineResult::FrameRendered { width, height, texture } => {
                                // Register and show the texture
                                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                                self.video_frame.set_wgpu_texture(
                                    wgpu_render_state,
                                    &view,
                                    [width as usize, height as usize],
                                );
                            }
                            engine::core::EngineResult::Error { message } => {
                                eprintln!("Engine error: {}", message);
                                ui.colored_label(egui::Color32::RED, format!("Engine error: {}", message));
                            }
                            _ => { /* other results not used in this path */ }
                        }
                    }
                }
            } else if self.engine.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("Engine Failed to Initialize");
                    ui.label("Check console for errors");
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("No Video Loaded");
                    ui.label("Click 'File → Load Video' to get started");
                });
            }
        });

        // Request continuous repaints for smooth playback
        if self.player.as_ref().is_some_and(|p| p.is_playing()) {
            ctx.request_repaint();
        }
    }
}

fn create_ui_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ui_display_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}
