use crate::view::View;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

pub struct NodeBlueprint {
    /// Current zoom level (1.0 = 100%)
    zoom: f32,
    /// Pan offset in canvas space
    pan_offset: Vec2,
    /// Is the user currently panning?
    is_panning: bool,
    /// Grid size in canvas units
    grid_size: f32,
}

impl NodeBlueprint {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            is_panning: false,
            grid_size: 50.0,
        }
    }

    /// Convert screen position to canvas position
    fn screen_to_canvas(&self, screen_pos: Pos2, canvas_rect: Rect) -> Pos2 {
        let relative_pos = screen_pos - canvas_rect.min;
        let canvas_center = canvas_rect.size() / 2.0;
        let canvas_pos = (relative_pos - canvas_center) / self.zoom - self.pan_offset;
        Pos2::new(canvas_pos.x, canvas_pos.y)
    }

    /// Convert canvas position to screen position
    fn canvas_to_screen(&self, canvas_pos: Pos2, canvas_rect: Rect) -> Pos2 {
        let canvas_center = canvas_rect.size() / 2.0;
        let relative_pos = (canvas_pos.to_vec2() + self.pan_offset) * self.zoom + canvas_center;
        canvas_rect.min + relative_pos
    }

    /// Draw the blueprint grid
    fn draw_grid(&self, ui: &mut egui::Ui, canvas_rect: Rect) {
        let painter = ui.painter_at(canvas_rect);

        // Calculate visible grid bounds
        let top_left = self.screen_to_canvas(canvas_rect.min, canvas_rect);
        let bottom_right = self.screen_to_canvas(canvas_rect.max, canvas_rect);

        let grid_spacing = self.grid_size;

        // Grid line colors
        let major_color = Color32::from_rgba_premultiplied(200, 200, 200, 255);
        let minor_color = Color32::from_rgba_premultiplied(100, 100, 100, 255); 

        // Draw vertical lines
        let start_x = (top_left.x / grid_spacing).floor() * grid_spacing;
        let mut x = start_x;
        while x <= bottom_right.x {
            let is_major = (x / grid_spacing).round() as i32 % 5 == 0;
            let color = if is_major { major_color } else { minor_color };
            let thickness = if is_major { 1.5 } else { 1.0 };

            let screen_start = self.canvas_to_screen(Pos2::new(x, top_left.y), canvas_rect);
            let screen_end = self.canvas_to_screen(Pos2::new(x, bottom_right.y), canvas_rect);

            painter.line_segment(
                [screen_start, screen_end],
                Stroke::new(thickness, color),
            );

            x += grid_spacing;
        }

        // Draw horizontal lines
        let start_y = (top_left.y / grid_spacing).floor() * grid_spacing;
        let mut y = start_y;
        while y <= bottom_right.y {
            let is_major = (y / grid_spacing).round() as i32 % 5 == 0;
            let color = if is_major { major_color } else { minor_color };
            let thickness = if is_major { 1.5 } else { 1.0 };

            let screen_start = self.canvas_to_screen(Pos2::new(top_left.x, y), canvas_rect);
            let screen_end = self.canvas_to_screen(Pos2::new(bottom_right.x, y), canvas_rect);

            painter.line_segment(
                [screen_start, screen_end],
                Stroke::new(thickness, color),
            );

            y += grid_spacing;
        }

        // Draw origin axes (highlighted)
        let origin_color = Color32::from_rgba_premultiplied(220, 220, 220, 255);
        
        // X-axis
        let x_start = self.canvas_to_screen(Pos2::new(top_left.x, 0.0), canvas_rect);
        let x_end = self.canvas_to_screen(Pos2::new(bottom_right.x, 0.0), canvas_rect);
        painter.line_segment([x_start, x_end], Stroke::new(2.0, origin_color));

        // Y-axis
        let y_start = self.canvas_to_screen(Pos2::new(0.0, top_left.y), canvas_rect);
        let y_end = self.canvas_to_screen(Pos2::new(0.0, bottom_right.y), canvas_rect);
        painter.line_segment([y_start, y_end], Stroke::new(2.0, origin_color));
    }

    /// Handle input (zoom and pan)
    fn handle_input(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) {
        let response = ui.interact(canvas_rect, ui.id(), egui::Sense::click_and_drag());

        // Handle zoom with scroll wheel
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            let zoom_delta = scroll_delta * 0.001;
            let old_zoom = self.zoom;
            self.zoom = (self.zoom * (1.0 + zoom_delta)).clamp(0.1, 10.0);

            // Zoom towards mouse position
            if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                if canvas_rect.contains(mouse_pos) {
                    let zoom_ratio = self.zoom / old_zoom;
                    
                    // Adjust pan to keep the point under the mouse stationary
                    let canvas_center = canvas_rect.size() / 2.0;
                    let relative_pos = mouse_pos - canvas_rect.min - canvas_center;
                    self.pan_offset = (self.pan_offset * zoom_ratio) 
                        + (relative_pos / old_zoom - relative_pos / self.zoom);
                }
            }
        }

        // Handle panning with middle mouse or right mouse button
        if response.dragged_by(egui::PointerButton::Middle) 
            || response.dragged_by(egui::PointerButton::Secondary) {
            self.is_panning = true;
            self.pan_offset += response.drag_delta() / self.zoom;
        } else {
            self.is_panning = false;
        }

        // Reset view with 'R' key
        if ui.input(|i| i.key_pressed(egui::Key::R)) {
            self.zoom = 1.0;
            self.pan_offset = Vec2::ZERO;
        }
    }
}

impl View for NodeBlueprint {
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Get the available space for the canvas
        let available_size = ui.available_size();
        let canvas_rect = Rect::from_min_size(ui.cursor().min, available_size);

        // Fill background with black
        ui.painter().rect_filled(
            canvas_rect,
            0.0,
            Color32::from_rgb(20, 22, 25),
        );

        // Draw the grid
        self.draw_grid(ui, canvas_rect);

        // Handle input
        self.handle_input(ui, canvas_rect);

        // Display info overlay
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(canvas_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
            |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Zoom: {:.1}%",
                            self.zoom * 100.0
                        ))
                        .color(Color32::from_rgb(200, 200, 200))
                        .size(12.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Middle/Right Mouse: Pan | Scroll: Zoom | R: Reset")
                            .color(Color32::from_rgb(150, 150, 150))
                            .size(11.0),
                    );
                });
            },
        );

        // Allocate the space so egui knows we used it
        ui.allocate_rect(canvas_rect, egui::Sense::hover());
    }
}