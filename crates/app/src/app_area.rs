<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
mod editor;
mod title_bar;

use editor::EditorArea;
use util::eframe;
use util::egui;

/// This is the main area of the app.
/// Anything you add to this please make sure it is contained within an _area file
/// The app struct should handle as little logic as possible, and should just be responsible for rendering the different areas of the app and passing data between them
pub struct AppArea {
    title_bar: title_bar::TitleBarArea,
    editor_area: EditorArea,
}

impl AppArea {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        Self {
            title_bar: title_bar::TitleBarArea::new(),
            editor_area: EditorArea::new(),
        }
    }

    fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(0x3E, 0x3A, 0x3C))    // #3E3A3C
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ctx, |ui| {
                self.title_bar.ui(ui);
            });
    }
}

impl eframe::App for AppArea {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.show_top_bar(ctx);
        self.editor_area.show(ctx, frame);
    }
=======
=======
mod menu_bar;
>>>>>>> 6d842de (ui in good state)
=======
mod title_bar;
pub use title_bar::TitleBar;
>>>>>>> 59a6b68 (started adding some basic node and video stuff)
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
<<<<<<< HEAD
<<<<<<< HEAD
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
    visuals.override_text_color = Some(Color32::from_rgb(102, 255, 51));

    // Reduce spacing globally
    let mut style = egui::Style::default();
    style.spacing.item_spacing = egui::vec2(4.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.visuals = visuals;

    ctx.set_style(style);
>>>>>>> 2069524 (trying to get the UI looking right)
}
=======
}
>>>>>>> 6d842de (ui in good state)
=======
}
>>>>>>> 59a6b68 (started adding some basic node and video stuff)
