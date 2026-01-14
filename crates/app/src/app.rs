use crate::areas::node_blueprint::NodeBlueprint;
use crate::components::menu_bar::MenuAction;
use crate::components::menu_bar::MenuBar;
use crate::video::VideoContext;
use crate::view::View;

pub struct App {
    menu_bar: MenuBar,
    node_blueprint: NodeBlueprint,
    video_context: VideoContext,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_styles(&cc.egui_ctx);

        let wgpu_render_state = cc.wgpu_render_state.as_ref().unwrap();
        let video_context = VideoContext::new(wgpu_render_state.target_format).unwrap();

        Self {
            menu_bar: MenuBar::new(),
            node_blueprint: NodeBlueprint::new(),
            video_context,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu_bar.ui(ui);
        });

        // Add a left side panel
        egui::SidePanel::left("left_panel")
            .default_width(400.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Node Library");
            });

        // Blueprint takes the remaining space
        egui::CentralPanel::default().show(ctx, |ui| {
            self.node_blueprint.ui(ui);
        });

        for action in self.menu_bar.drain_actions() {
            match action {
                MenuAction::ImportVideo(path) => {
                    if let Err(e) = self.video_context.load_video(path) {
                        eprintln!("Failed to load video: {e}");
                    }
                    else {
                        self.video_context.toggle_playback();
                    }
                }
                _ => {}
            }
        }

        if self.video_context.video_loaded() {
            // Delta time for this frame
            let dt = ctx.input(|i| i.unstable_dt);

            // Update the video controller / player and GPU texture
            self.video_context
                .update_and_render(frame.wgpu_render_state().unwrap(), dt);

            // Render the floating frame in egui
            egui::Area::new(egui::Id::new("video_frame_context"))
                .movable(true)
                .show(ctx, |ui| {
                    self.video_context.ui(ui);
                });
        }

        if self.video_context.is_playing() {
            ctx.request_repaint();
        }
        
    }
}

fn configure_styles(ctx: &egui::Context) {
    use egui::{Color32, Visuals};

    let mut visuals = Visuals::dark();

    // Main background
    visuals.panel_fill = Color32::from_rgb(24, 29, 31);

    // Menu bar background (darker)
    visuals.window_fill = Color32::from_rgb(20, 24, 27);

    // Text edits, scroll bars
    visuals.extreme_bg_color = Color32::from_rgb(33, 54, 33);

    // Menu button styling
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(40, 44, 47);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(50, 54, 57);
    visuals.override_text_color = Some(Color32::from_rgb(0x9D, 0xF2, 0x9F));

    // Reduce spacing globally
    let mut style = egui::Style::default();
    style.spacing.item_spacing = egui::vec2(4.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.visuals = visuals;

    ctx.set_style(style);
}
