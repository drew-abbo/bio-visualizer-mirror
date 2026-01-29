mod title_bar;
pub use title_bar::TitleBar;
mod node_blueprint;
use crate::engine_controller::EngineController;
use crate::view::View;
use node_blueprint::NodeBlueprint;

pub struct App {
    title_bar: TitleBar,
    node_blueprint: NodeBlueprint,
    engine_controller: EngineController,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        // TODO: Handle error properly
        let engine_controller = EngineController::new().unwrap();

        Self {
            title_bar: TitleBar::new(),
            node_blueprint: NodeBlueprint::new(),
            engine_controller: engine_controller,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(24, 29, 31))
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ctx, |ui| {
                self.title_bar.ui(ui);
            });

        // Blueprint takes the remaining space
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.node_blueprint.ui(ui);
            });

        // for action in self.menu_bar.drain_actions() {
        //     match action {
        //         MenuAction::ImportVideo(path) => {
        //             // if let Err(e) = self.video_context.load_video(path) {
        //             //     eprintln!("Failed to load video: {e}");
        //             // }
        //             // else {
        //             //     self.video_context.toggle_playback();
        //             // }
        //         }
        //     }
        // }

        // if self.video_context.video_loaded() {
        //     // Delta time for this frame
        //     let dt = ctx.input(|i| i.unstable_dt);

        //     // Update the video controller / player and GPU texture
        //     self.video_context
        //         .update_and_render(frame.wgpu_render_state().unwrap(), dt);

        //     // Render the floating frame in egui
        //     egui::Area::new(egui::Id::new("video_frame_context"))
        //         .movable(true)
        //         .show(ctx, |ui| {
        //             self.video_context.ui(ui);
        //         });
        // }

        // if self.video_context.is_playing() {
        //     ctx.request_repaint();
        // }
    }
}
