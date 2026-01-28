use crate::app_area::menu_bar::MenuBar;
use crate::view::View;
use egui_phosphor::regular;

pub struct TitleBar {
    menu_bar: MenuBar,
}

impl TitleBar {
    pub fn new() -> Self {
        Self {
            menu_bar: MenuBar::new(),
        }
    }
}

impl View for TitleBar {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        let title_bar_response = ui.interact(
            ui.max_rect(),
            egui::Id::new("title_bar"),
            egui::Sense::click_and_drag(),
        );

        if title_bar_response.drag_started_by(egui::PointerButton::Primary) {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.visuals_mut().button_frame = false;

            // Window control buttons
            let button_size = 18.0;
            let button_color = egui::Color32::from_rgb(220, 220, 220);

            let close_response = ui
                .add(egui::Button::new(
                    egui::RichText::new(regular::X)
                        .size(button_size)
                        .color(button_color),
                ))
                .on_hover_text("Close");
            if close_response.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }

            let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            if is_maximized {
                let maximize_response = ui
                    .add(egui::Button::new(
                        egui::RichText::new(regular::CORNERS_IN)
                            .size(button_size)
                            .color(button_color),
                    ))
                    .on_hover_text("Restore");
                if maximize_response.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                }
            } else {
                let maximize_response = ui
                    .add(egui::Button::new(
                        egui::RichText::new(regular::CORNERS_OUT)
                            .size(button_size)
                            .color(button_color),
                    ))
                    .on_hover_text("Maximize");
                if maximize_response.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                }
            }

            let minimize_response = ui
                .add(egui::Button::new(
                    egui::RichText::new(regular::MINUS)
                        .size(button_size)
                        .color(button_color),
                ))
                .on_hover_text("Minimize");
            if minimize_response.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }

            ui.add_space(12.0);

            // Reset button frame for menu buttons
            ui.visuals_mut().button_frame = true;

            self.menu_bar.ui(ui);
        });
    }
}
