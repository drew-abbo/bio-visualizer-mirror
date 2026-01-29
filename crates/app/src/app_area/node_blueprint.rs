use crate::{components::Node, view::View};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

mod context_menu;
mod node_item;

pub use context_menu::ContextMenu;
pub use node_item::NodeItem;

#[derive(Clone, Copy, Debug)]
struct NodeBlueprintStyle {
    background: Color32,
    grid: Color32,
    info_text: Color32,
    hint_text: Color32,
    grid_thickness: f32,
}

pub struct NodeBlueprint {
    /// Current zoom level (1.0 = 100%)
    zoom: f32,
    /// Pan offset in canvas space
    pan_offset: Vec2,
    /// Is the user currently panning?
    is_panning: bool,
    /// Grid size in canvas units
    grid_size: f32,
    /// Nodes on the blueprint
    nodes: Vec<NodeItem>,
    /// Context menu for right-click actions
    context_menu: ContextMenu,
    /// Next node ID to assign
    next_node_id: usize,
    /// ID of the node currently being dragged
    dragging_node: Option<usize>,
}

impl NodeBlueprint {
    fn style() -> NodeBlueprintStyle {
        NodeBlueprintStyle {
            background: Color32::from_rgb(20, 22, 25),
            grid: Color32::from_rgb(20, 51, 10),
            info_text: Color32::from_rgba_premultiplied(200, 220, 235, 200),
            hint_text: Color32::from_rgba_premultiplied(160, 190, 210, 160),
            grid_thickness: 1.0,
        }
    }

    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            is_panning: false,
            grid_size: 50.0,
            nodes: Vec::new(),
            context_menu: ContextMenu::new(),
            next_node_id: 0,
            dragging_node: None,
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
        let style = Self::style();

        // Calculate visible grid bounds
        let top_left = self.screen_to_canvas(canvas_rect.min, canvas_rect);
        let bottom_right = self.screen_to_canvas(canvas_rect.max, canvas_rect);

        let grid_spacing = self.grid_size;

        // Draw vertical lines
        let start_x = (top_left.x / grid_spacing).floor() * grid_spacing;
        let mut x = start_x;
        while x <= bottom_right.x {
            let color = style.grid;
            let thickness = style.grid_thickness;

            let screen_start = self.canvas_to_screen(Pos2::new(x, top_left.y), canvas_rect);
            let screen_end = self.canvas_to_screen(Pos2::new(x, bottom_right.y), canvas_rect);

            painter.line_segment([screen_start, screen_end], Stroke::new(thickness, color));

            x += grid_spacing;
        }

        // Draw horizontal lines
        let start_y = (top_left.y / grid_spacing).floor() * grid_spacing;
        let mut y = start_y;
        while y <= bottom_right.y {
            let color = style.grid;
            let thickness = style.grid_thickness;

            let screen_start = self.canvas_to_screen(Pos2::new(top_left.x, y), canvas_rect);
            let screen_end = self.canvas_to_screen(Pos2::new(bottom_right.x, y), canvas_rect);

            painter.line_segment([screen_start, screen_end], Stroke::new(thickness, color));

            y += grid_spacing;
        }
    }

    /// Handle input (zoom and pan)
    fn handle_input(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) {
        let response = ui.interact(canvas_rect, ui.id(), egui::Sense::click_and_drag());

        let zoom_delta = ui.input(|i| i.zoom_delta());
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
        let scroll_is_zoom = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);

        // Trackpad pinch zoom (and Ctrl/Cmd + scroll wheel zoom)
        if zoom_delta != 1.0 || (scroll_is_zoom && scroll_delta.y != 0.0) {
            let delta = if zoom_delta != 1.0 {
                zoom_delta
            } else {
                1.0 + scroll_delta.y * 0.001
            };

            let old_zoom = self.zoom;
            self.zoom = (self.zoom * delta).clamp(0.1, 10.0);

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
        } else if scroll_delta != Vec2::ZERO {
            // Trackpad two-finger pan
            self.pan_offset += scroll_delta / self.zoom;
        }

        // Handle panning with middle mouse or right mouse button
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            self.is_panning = true;
            self.pan_offset += response.drag_delta() / self.zoom;
        } else {
            self.is_panning = false;
        }
    }

    /// Handle right-click context menu
    fn handle_context_menu(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) {
        let response = ui.interact(canvas_rect, ui.id().with("blueprint_bg"), egui::Sense::click());

        // Right-click to open context menu
        if response.secondary_clicked() {
            if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
                if canvas_rect.contains(mouse_pos) {
                    self.context_menu.open_at(mouse_pos);
                }
            }
        }

        // Handle context menu actions
        if let Some(action) = self.context_menu.ui(ui) {
            match action {
                context_menu::ContextMenuAction::AddNode => {
                    // Get the canvas position where the right-click occurred
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
                        let canvas_pos = self.screen_to_canvas(mouse_pos, canvas_rect);
                        self.add_node_at(canvas_pos);
                    }
                }
            }
        }
    }

    /// Add a new node at the specified canvas position
    fn add_node_at(&mut self, canvas_pos: Pos2) {
        use crate::components::BaseNode;

        let label = format!("Node {}", self.next_node_id);
        let node = BaseNode::new(self.next_node_id, label);
        
        // Find a position that doesn't overlap with existing nodes
        let final_pos = self.find_non_overlapping_position(canvas_pos);
        let item = NodeItem::new(node, final_pos);

        self.nodes.push(item);
        self.next_node_id += 1;
    }

    /// Find a position close to the requested position that doesn't overlap with existing nodes
    fn find_non_overlapping_position(&self, desired_pos: Pos2) -> Pos2 {
        const MIN_SPACING: f32 = 20.0;
        const MAX_SEARCH_DISTANCE: f32 = 200.0;
        const SEARCH_STEP: f32 = 10.0;

        // Check if the desired position is clear
        if !self.has_overlap_at(desired_pos) {
            return desired_pos;
        }

        // Spiral search around the desired position
        let mut search_distance = SEARCH_STEP;
        while search_distance <= MAX_SEARCH_DISTANCE {
            for angle_idx in 0..8 {
                let angle = (angle_idx as f32 / 8.0) * std::f32::consts::TAU;
                let offset = Vec2::new(angle.cos(), angle.sin()) * search_distance;
                let test_pos = desired_pos + offset;

                if !self.has_overlap_at(test_pos) {
                    return test_pos;
                }
            }
            search_distance += SEARCH_STEP;
        }

        // If we couldn't find a clear spot, just offset from desired
        desired_pos + Vec2::new(MIN_SPACING, MIN_SPACING)
    }

    /// Check if there's a node overlap at the given position
    fn has_overlap_at(&self, pos: Pos2) -> bool {
        let test_rect = Rect::from_min_size(pos, Vec2::new(200.0, 150.0));
        const PADDING: f32 = 10.0;

        self.nodes.iter().any(|node| {
            let padded_bounds = node.bounds().expand(PADDING);
            test_rect.intersects(padded_bounds)
        })
    }
}

impl View for NodeBlueprint {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let style = Self::style();
        // Use the full panel rect to avoid borders/padding
        let canvas_rect = ui.max_rect();

        // Fill background with black
        ui.painter().rect_filled(canvas_rect, 0.0, style.background);

        // Draw the grid
        self.draw_grid(ui, canvas_rect);

        // Handle input FIRST (before rendering nodes so input isn't blocked)
        self.handle_input(ui, canvas_rect);

        // Handle right-click context menu BEFORE rendering nodes
        self.handle_context_menu(ui, canvas_rect);

        // Render nodes with canvas coordinate conversion
        let zoom = self.zoom;
        let pan_offset = self.pan_offset;
        let canvas_center = canvas_rect.size() / 2.0;
        let canvas_min = canvas_rect.min;
        
        let canvas_to_screen = |pos: Pos2| {
            let relative_pos = (pos.to_vec2() + pan_offset) * zoom + canvas_center;
            canvas_min + relative_pos
        };
        
        let screen_to_canvas = |screen_pos: Pos2| {
            let relative_pos = screen_pos - canvas_min;
            let canvas_pos = (relative_pos - canvas_center) / zoom - pan_offset;
            Pos2::new(canvas_pos.x, canvas_pos.y)
        };

        for node_item in &mut self.nodes {
            node_item.ui(ui, &mut self.dragging_node, canvas_to_screen, screen_to_canvas, zoom);
        }

        // Stop dragging if mouse released
        if !ui.input(|i| i.pointer.any_down()) {
            self.dragging_node = None;
        }
        
        // Check for and prevent collisions with dragged node
        if let Some(dragging_id) = self.dragging_node {
            // Find the dragged node index
            if let Some(dragged_idx) = self.nodes.iter().position(|n| n.node.id() == dragging_id) {
                // Collect collision data before mutating
                let mut collision_adjustments = Vec::new();
                
                {
                    let dragged_bounds = self.nodes[dragged_idx].bounds();
                    let dragged_size = self.nodes[dragged_idx].size;
                    
                    // Check against all other nodes
                    for (other_idx, other_node) in self.nodes.iter().enumerate() {
                        if other_idx != dragged_idx {
                            const PADDING: f32 = 10.0;
                            let padded_bounds = other_node.bounds().expand(PADDING);
                            
                            if dragged_bounds.intersects(padded_bounds) {
                                // Collision detected - calculate push away
                                let other_center = other_node.bounds().center();
                                let this_center = dragged_bounds.center();
                                let push_dir = (this_center - other_center).normalized();
                                
                                let min_dist = (dragged_size.length() + other_node.size.length()) / 2.0 + PADDING;
                                let current_dist = this_center.distance(other_center);
                                let push_amount = (min_dist - current_dist).max(0.0);
                                
                                collision_adjustments.push(push_dir * push_amount);
                            }
                        }
                    }
                }
                
                // Apply adjustments
                for adjustment in collision_adjustments {
                    self.nodes[dragged_idx].position += adjustment;
                }
            }
        }

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
                        .color(style.info_text)
                        .size(12.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "Two-finger: Pan | Pinch/Ctrl+Scroll: Zoom | Middle/Right: Pan | R: Reset | Right-click: Add Node",
                        )
                            .color(style.hint_text)
                            .size(11.0),
                    );
                });
            },
        );

        // Allocate the space so egui knows we used it
        ui.allocate_rect(canvas_rect, egui::Sense::hover());
    }
}
