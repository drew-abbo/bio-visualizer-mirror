use egui::{Color32, Pos2, Vec2};

#[derive(Debug, Clone, Copy)]
pub enum ContextMenuAction {
    AddNode,
}

pub struct ContextMenu {
    /// Position where the menu should appear (in screen space)
    position: Pos2,
    /// Is the menu currently open?
    is_open: bool,
}

impl ContextMenu {
    pub fn new() -> Self {
        Self {
            position: Pos2::ZERO,
            is_open: false,
        }
    }

    pub fn open_at(&mut self, position: Pos2) {
        self.position = position;
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Render the context menu and return the selected action (if any)
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<ContextMenuAction> {
        if !self.is_open {
            return None;
        }

        let mut action = None;
        let menu_width = 120.0;
        let menu_height = 30.0;

        // Draw menu background
        let menu_rect = egui::Rect::from_min_size(self.position, Vec2::new(menu_width, menu_height));
        let painter = ui.painter();

        painter.rect_filled(menu_rect, 4.0, Color32::from_rgb(40, 42, 45));
        
        // Draw border using line segments
        let stroke = egui::Stroke::new(1.0, Color32::from_rgb(100, 100, 100));
        painter.line_segment([menu_rect.left_top(), menu_rect.right_top()], stroke);
        painter.line_segment([menu_rect.right_top(), menu_rect.right_bottom()], stroke);
        painter.line_segment([menu_rect.right_bottom(), menu_rect.left_bottom()], stroke);
        painter.line_segment([menu_rect.left_bottom(), menu_rect.left_top()], stroke);

        // Create a response for the menu area
        let response = ui.interact(menu_rect, ui.id().with("context_menu"), egui::Sense::click());

        // Draw menu items
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(menu_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
            |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    if ui.button("Add Node").clicked() {
                        action = Some(ContextMenuAction::AddNode);
                        self.is_open = false;
                    }
                });
            },
        );

        // Close menu if clicked outside
        if ui.input(|i| i.pointer.any_pressed()) && !response.hovered() {
            if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                if !menu_rect.contains(pos) {
                    self.is_open = false;
                }
            }
        }

        action
    }
}
