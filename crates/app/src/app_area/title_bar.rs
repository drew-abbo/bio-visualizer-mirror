mod toolbar;
use crate::app_area::title_bar::toolbar::ToolBar;
use crate::view::View;
use egui_phosphor::regular;

pub struct TitleBar {
    tool_bar: ToolBar,
    dragging: bool,
}

impl TitleBar {
    pub fn new() -> Self {
        Self {
            tool_bar: ToolBar::new(),
            dragging: false,
        }
    }
}

impl View for TitleBar {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        // Render toolbar on the left, and window controls on the right.
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.visuals_mut().button_frame = false; // Don't show frames for toolbar buttons
            self.tool_bar.ui(ui);

            // Fill the middle space so controls stay at the far right.
            // Make the middle area a draggable spacer so clicks there start window drag
            // without interfering with toolbar or control widgets.
            let avail = ui.available_size_before_wrap();
            let button_size = 22.0;
            let controls_width = button_size * 3.0 + 24.0;
            let spacer_w = (avail.x - controls_width).max(8.0);

            let (_, rect) = ui.allocate_space(egui::Vec2::new(spacer_w, avail.y));
            let drag_resp = ui
                .interact(
                    rect,
                    egui::Id::new("title_spacer"),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Default);

            if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                self.dragging = true;
            }

            // Clear dragging when pointer is released
            if !ui.input(|i| i.pointer.any_down()) {
                self.dragging = false;
            }

            // Force cursor icon during drag to prevent text cursor
            if self.dragging {
                ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
            }

            // Right: window controls
            let button_color = ui.visuals().text_color();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                let close_response = ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(regular::X)
                                .size(button_size)
                                .color(button_color),
                        )
                        .frame(false)
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    )
                    .on_hover_text("Close");
                if close_response.hovered() {
                    ui.painter().rect_filled(
                        close_response.rect,
                        4.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15),
                    );
                }
                if close_response.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                if is_maximized {
                    let maximize_response = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(regular::CORNERS_IN)
                                    .size(button_size)
                                    .color(button_color),
                            )
                            .frame(false)
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        )
                        .on_hover_text("Restore");
                    if maximize_response.hovered() {
                        ui.painter().rect_filled(
                            maximize_response.rect,
                            4.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15),
                        );
                    }
                    if maximize_response.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    }
                } else {
                    let maximize_response = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(regular::CORNERS_OUT)
                                    .size(button_size)
                                    .color(button_color),
                            )
                            .frame(false)
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        )
                        .on_hover_text("Maximize");
                    if maximize_response.hovered() {
                        ui.painter().rect_filled(
                            maximize_response.rect,
                            4.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15),
                        );
                    }
                    if maximize_response.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                    }
                }

                let minimize_response = ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(regular::MINUS)
                                .size(button_size)
                                .color(button_color),
                        )
                        .frame(false)
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    )
                    .on_hover_text("Minimize");
                if minimize_response.hovered() {
                    ui.painter().rect_filled(
                        minimize_response.rect,
                        4.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15),
                    );
                }
                if minimize_response.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });
    }
}
