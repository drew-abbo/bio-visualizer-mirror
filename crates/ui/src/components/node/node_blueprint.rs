use egui::{Pos2, Rect, Vec2};

use crate::components::{
    View,
    node::{NodeItem, placed_node::PlacedNode},
};

pub struct NodeBlueprint {
    placed_nodes: Vec<PlacedNode>,
    connections: Vec<Connection>,
    dragging_node: Option<usize>,
    drag_offset: Vec2,
    active_connection: Option<ActiveConnection>,

    selected_node: Option<usize>,

    // pan/zoom
    pan_offset: Vec2,                 // How far we have panned the view
    zoom: f32,                        // How zoomed in we are
    is_panning: bool,                 // Whether we are currently panning
    last_pan_pos: Option<egui::Pos2>, // Last mouse position during panning, to calculate delta
}

/// Represents a connection between two nodes
#[derive(Clone, Debug)]
pub struct Connection {
    pub from_node_idx: usize,
    pub to_node_idx: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum PortType {
    Input,
    Output,
}

/// Tracks the connection being actively created by the user
struct ActiveConnection {
    from_node_idx: usize,
    from_port: PortType,
    current_pos: Pos2, // Where the mouse is
}

/// Blueprint area where nodes can be placed, connected, and manipulated
/// Canvas space is the virtual coordinate system for the nodes
/// Screen space is the actual pixel coordinates on the UI, what the user sees
impl NodeBlueprint {
    pub fn new() -> Self {
        Self {
            placed_nodes: Vec::new(),
            connections: Vec::new(),
            dragging_node: None,
            drag_offset: Vec2::ZERO,
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            is_panning: false,
            last_pan_pos: None,
            active_connection: None,
            selected_node: None,
        }
    }

    /// Convert screen space to canvas space
    /// Converts mouse click position on screen to the actual position on the canvas
    fn screen_to_canvas(&self, screen_pos: egui::Pos2, rect: Rect) -> egui::Pos2 {
        let relative = screen_pos - rect.min;
        let canvas_pos = (relative - self.pan_offset) / self.zoom;
        egui::pos2(canvas_pos.x, canvas_pos.y)
    }

    /// Convert canvas space to screen space
    /// Takes a node's actual canvas position and figures out where to draw it on screen
    fn canvas_to_screen(&self, canvas_pos: egui::Pos2, rect: Rect) -> egui::Pos2 {
        let scaled = canvas_pos.to_vec2() * self.zoom;
        rect.min + scaled + self.pan_offset
    }

    fn draw_grid(&self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter();
        let grid_size = 20.0 * self.zoom;
        let color = ui.visuals().weak_text_color().linear_multiply(0.1);

        // Calculate visible grid range
        let start_x = ((rect.min.x - self.pan_offset.x) / grid_size).floor() * grid_size;
        let start_y = ((rect.min.y - self.pan_offset.y) / grid_size).floor() * grid_size;

        //TODO: Optimize more, I noticed that the vertical lines are not showing up sometimes

        // Vertical lines
        let mut x = start_x;
        while x < rect.max.x {
            let screen_x = x + self.pan_offset.x;
            // Only draw lines within the visible rect
            if screen_x >= rect.min.x && screen_x <= rect.max.x {
                painter.line_segment(
                    [
                        egui::pos2(screen_x, rect.min.y),
                        egui::pos2(screen_x, rect.max.y),
                    ],
                    egui::Stroke::new(1.0, color),
                );
            }
            x += grid_size;
        }

        // Horizontal lines
        let mut y = start_y;
        while y < rect.max.y {
            let screen_y = y + self.pan_offset.y;
            if screen_y >= rect.min.y && screen_y <= rect.max.y {
                painter.line_segment(
                    [
                        egui::pos2(rect.min.x, screen_y),
                        egui::pos2(rect.max.x, screen_y),
                    ],
                    egui::Stroke::new(1.0, color),
                );
            }
            y += grid_size;
        }
    }

    fn draw_node(
        &self,
        ui: &mut egui::Ui,
        node: &PlacedNode,
        is_dragging: bool,
        canvas_rect: Rect,
    ) {
        let painter = ui.painter();

        // Transform node position to screen space
        let screen_pos = self.canvas_to_screen(node.position, canvas_rect);
        let scaled_size = node.size * self.zoom;
        let rect = Rect::from_min_size(screen_pos, scaled_size);

        let base_color = node.color;
        let fill_color = if is_dragging {
            base_color.linear_multiply(0.3)
        } else {
            base_color.linear_multiply(0.2)
        };

        // Draw node box
        painter.rect(
            rect,
            8.0 * self.zoom,
            fill_color,
            egui::Stroke::new(2.0 * self.zoom, base_color),
            egui::StrokeKind::Inside,
        );

        // Draw name
        let name_pos = rect.center();
        painter.text(
            name_pos,
            egui::Align2::CENTER_CENTER,
            node.name(),
            egui::FontId::proportional(14.0 * self.zoom),
            egui::Color32::WHITE,
        );

        // Draw input/output ports
        let port_radius = 6.0 * self.zoom;
        let port_color = egui::Color32::from_rgb(200, 200, 200);

        // Input port (left)
        let input_pos = egui::pos2(rect.min.x, rect.center().y);

        if node.has_input {
            painter.circle(
                input_pos,
                port_radius,
                port_color,
                egui::Stroke::new(2.0 * self.zoom, base_color),
            );
        }

        // Output port (right)
        let output_pos = egui::pos2(rect.max.x, rect.center().y);

        if node.has_output {
            painter.circle(
                output_pos,
                port_radius,
                port_color,
                egui::Stroke::new(2.0 * self.zoom, base_color),
            );
        }

        // Highlight ports during connection dragging
        if node.has_input {
            let highlight_input = self.active_connection.is_some();

            let input_color = if highlight_input {
                egui::Color32::from_rgb(100, 255, 100) // highlight color
            } else {
                port_color
            };

            painter.circle(
                input_pos,
                port_radius,
                input_color,
                egui::Stroke::new(2.0 * self.zoom, base_color),
            );
        }
    }

    fn handle_drop(&mut self, ui: &mut egui::Ui, response: &egui::Response, rect: Rect) {
        if let Some(node_item) = response.dnd_release_payload::<NodeItem>() {
            if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                if rect.contains(pointer_pos) {
                    let canvas_pos = self.screen_to_canvas(pointer_pos, rect);
                    let node = PlacedNode::from_item(&node_item, canvas_pos);
                    self.placed_nodes.push(node);
                }
            }
        }
    }

    fn handle_pan_and_zoom(&mut self, ui: &mut egui::Ui, response: &egui::Response, rect: Rect) {
        // Handle zoom with scroll wheel
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y); // The movement of the scroll wheel
        if scroll_delta != 0.0 && response.hovered() {
            let zoom_delta = scroll_delta * 0.001; // Converts to zoom amount
            let old_zoom = self.zoom;
            self.zoom = (self.zoom + zoom_delta).clamp(0.1, 5.0); // Limit zoom levels

            // Some trickery to keep zoom centered on mouse pointer
            if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
                if rect.contains(pointer_pos) {
                    let pointer_offset = pointer_pos - rect.min;
                    self.pan_offset = pointer_offset
                        - (pointer_offset - self.pan_offset) * (self.zoom / old_zoom);
                }
            }
        }

        // Handle panning with middle mouse button, could add more
        let is_middle_down = ui.input(|i| i.pointer.middle_down());
        let should_pan = is_middle_down;

        if should_pan && response.hovered() {
            if !self.is_panning {
                self.is_panning = true;
                self.last_pan_pos = ui.ctx().pointer_interact_pos();
            }

            if let Some(current_pos) = ui.ctx().pointer_interact_pos() {
                if let Some(last_pos) = self.last_pan_pos {
                    let delta = current_pos - last_pos;
                    self.pan_offset += delta;
                }
                self.last_pan_pos = Some(current_pos);
            }
        } else {
            self.is_panning = false;
            self.last_pan_pos = None;
        }
    }

    fn handle_node_dragging(&mut self, ui: &mut egui::Ui, rect: Rect) {
        // Don't drag nodes while panning
        if self.is_panning {
            return;
        }

        let pointer_pos = ui.ctx().pointer_interact_pos();

        if ui.input(|i| i.pointer.primary_pressed()) {
            if let Some(pos) = pointer_pos {
                // Only clicks inside the blueprint canvas can affect selection
                if rect.contains(pos) {
                    let canvas_pos = self.screen_to_canvas(pos, rect);

                    // Clear selection only when clicking empty canvas area
                    let mut clicked_node = None;

                    for (i, node) in self.placed_nodes.iter().enumerate().rev() {
                        if node.contains(canvas_pos) {
                            clicked_node = Some(i);
                            break;
                        }
                    }

                    match clicked_node {
                        Some(i) => {
                            self.selected_node = Some(i);
                            self.dragging_node = Some(i);
                            self.drag_offset = canvas_pos - self.placed_nodes[i].position;
                        }
                        None => {
                            // Now it's safe to clear selection
                            self.selected_node = None;
                        }
                    }
                }
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            self.dragging_node = None;
        }

        // Update dragging node position
        if let Some(idx) = self.dragging_node {
            if let Some(pos) = pointer_pos {
                let canvas_pos = self.screen_to_canvas(pos, rect);
                let drag_offset = self.drag_offset;
                if let Some(node) = self.placed_nodes.get_mut(idx) {
                    node.position = canvas_pos - drag_offset;
                }
            }
        }
    }

    fn handle_connection_interaction(&mut self, ui: &mut egui::Ui, rect: Rect) {
        // Don't handle connections while panning or dragging nodes
        if self.is_panning || self.dragging_node.is_some() {
            return;
        }

        let pointer_pos = ui.ctx().pointer_interact_pos();

        // Start connection drag
        if ui.input(|i| i.pointer.primary_pressed()) {
            if let Some(pos) = pointer_pos {
                // Check if clicking on an output port
                for (idx, node) in self.placed_nodes.iter().enumerate() {
                    let port_pos = self.get_port_screen_pos(node, PortType::Output, rect);
                    if self.port_hit_test(pos, port_pos) {
                        self.active_connection = Some(ActiveConnection {
                            from_node_idx: idx,
                            from_port: PortType::Output,
                            current_pos: pos,
                        });
                        break;
                    }
                }
            }
        }

        // Remove connection on right-click of input port
        if ui.input(|i| i.pointer.secondary_pressed()) {
            if let Some(pos) = pointer_pos {
                for (idx, node) in self.placed_nodes.iter().enumerate() {
                    let port_pos = self.get_port_screen_pos(node, PortType::Input, rect);
                    if self.port_hit_test(pos, port_pos) {
                        // Find and remove connection to this input port
                        if let Some(conn_idx) =
                            self.connections.iter().position(|c| c.to_node_idx == idx)
                        {
                            self.connections.remove(conn_idx);
                        }
                        break;
                    }
                }
            }
        }

        // Update active connection position
        if let Some(active) = &mut self.active_connection {
            if let Some(pos) = pointer_pos {
                active.current_pos = pos;
            }
        }

        // Complete connection
        if ui.input(|i| i.pointer.primary_released()) {
            if let Some(active) = self.active_connection.take() {
                if let Some(pos) = pointer_pos {
                    // Check if released on an input port
                    for (idx, node) in self.placed_nodes.iter().enumerate() {
                        // Can't connect to self
                        if idx == active.from_node_idx {
                            continue;
                        }

                        let port_pos = self.get_port_screen_pos(node, PortType::Input, rect);
                        if self.port_hit_test(pos, port_pos) {
                            // Check if connection already exists
                            let conn_exists = self.connections.iter().any(|c| {
                                c.from_node_idx == active.from_node_idx && c.to_node_idx == idx
                            });

                            if !conn_exists {
                                self.connections.push(Connection {
                                    from_node_idx: active.from_node_idx,
                                    to_node_idx: idx,
                                });
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    fn draw_connections(&self, ui: &mut egui::Ui, canvas_rect: Rect) {
        let painter = ui.painter();

        // Draw established connections
        for conn in &self.connections {
            if let (Some(from_node), Some(to_node)) = (
                self.placed_nodes.get(conn.from_node_idx),
                self.placed_nodes.get(conn.to_node_idx),
            ) {
                let from_pos = self.get_port_screen_pos(from_node, PortType::Output, canvas_rect);
                let to_pos = self.get_port_screen_pos(to_node, PortType::Input, canvas_rect);

                self.draw_connection_curve(painter, from_pos, to_pos, ui.visuals().text_color());
            }
        }

        // Draw active connection being dragged
        if let Some(active) = &self.active_connection {
            if let Some(from_node) = self.placed_nodes.get(active.from_node_idx) {
                let from_pos = self.get_port_screen_pos(from_node, active.from_port, canvas_rect);
                self.draw_connection_curve(
                    painter,
                    from_pos,
                    active.current_pos,
                    egui::Color32::from_rgb(100, 150, 255),
                );
            }
        }
    }

    /// Draw a bezier curve between two points
    fn draw_connection_curve(
        &self,
        painter: &egui::Painter,
        from: Pos2,
        to: Pos2,
        color: egui::Color32,
    ) {
        let distance = (to.x - from.x).abs();
        let control_offset = (distance * 0.5).min(100.0 * self.zoom);

        let control1 = egui::pos2(from.x + control_offset, from.y);
        let control2 = egui::pos2(to.x - control_offset, to.y);

        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            [from, control1, control2, to],
            false,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(2.0 * self.zoom, color),
        ));
    }

    fn get_port_screen_pos(
        &self,
        node: &PlacedNode,
        port_type: PortType,
        canvas_rect: Rect,
    ) -> Pos2 {
        let screen_pos = self.canvas_to_screen(node.position, canvas_rect);
        let scaled_size = node.size * self.zoom;
        let rect = Rect::from_min_size(screen_pos, scaled_size);

        match port_type {
            PortType::Input => egui::pos2(rect.min.x, rect.center().y),
            PortType::Output => egui::pos2(rect.max.x, rect.center().y),
        }
    }

    pub fn selected_node_mut(&mut self) -> Option<&mut PlacedNode> {
        if let Some(idx) = self.selected_node {
            self.placed_nodes.get_mut(idx)
        } else {
            None
        }
    }

    /// Check if a screen position is hovering over a port
    fn port_hit_test(&self, screen_pos: Pos2, port_pos: Pos2) -> bool {
        let port_radius = 8.0 * self.zoom; // Slightly larger than visual for easier clicking
        screen_pos.distance(port_pos) <= port_radius
    }

    pub fn placed_nodes(&self) -> &[PlacedNode] {
        &self.placed_nodes
    }

    pub fn clear_nodes(&mut self) {
        self.placed_nodes.clear();
    }

    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    pub fn remove_connection(&mut self, from_idx: usize, to_idx: usize) {
        self.connections
            .retain(|c| !(c.from_node_idx == from_idx && c.to_node_idx == to_idx));
    }

    pub fn clear_connections(&mut self) {
        self.connections.clear();
    }

    /// Returns true if graph structure changed (for triggering recompilation)
    pub fn has_changes(&self) -> bool {
        // For now, always return false
        // Later: track dirty flag when connections/nodes change
        false
    }
}

impl View for NodeBlueprint {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let available_size = ui.available_size();
        let (rect, response) =
            ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);

        self.handle_pan_and_zoom(ui, &response, rect);
        self.draw_grid(ui, rect);

        // Draw connections before nodes so are on top
        self.draw_connections(ui, rect);

        self.handle_drop(ui, &response, rect);
        self.handle_node_dragging(ui, rect);

        //Handle connection interaction
        self.handle_connection_interaction(ui, rect);

        // Draw all nodes
        for (i, node) in self.placed_nodes.iter().enumerate() {
            let is_dragging = self.dragging_node == Some(i);
            self.draw_node(ui, node, is_dragging, rect);
        }

        if self.placed_nodes.is_empty() {
            let center = rect.center();
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                "Drag nodes from the left panel\nMiddle-click to pan, scroll to zoom",
                egui::FontId::proportional(18.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
}
