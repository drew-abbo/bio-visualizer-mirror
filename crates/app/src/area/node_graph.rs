use egui_snarl::ui::{PinInfo, SnarlViewer};
use egui_snarl::{InPin, NodeId as SnarlNodeId, OutPin, Snarl};
use engine::node::node::NodeOutputKind;
use engine::node::{NodeInputKind, NodeLibrary, input_kind_to_output_kind};
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use util::egui;

#[derive(Clone, Debug)]
pub struct NodeData {
    pub definition_name: String,
    /// Configured input values for this node (not including connected inputs)
    pub input_values: HashMap<String, InputValue>,
    /// Engine node ID if this node is currently in the engine graph
    pub engine_node_id: Option<EngineNodeId>,
}

pub struct NodeGraphState {
    pub snarl: Snarl<NodeData>,
}

impl NodeGraphState {
    pub fn new() -> Self {
        Self {
            snarl: Snarl::new(),
        }
    }
}

pub struct NodeGraphViewer {
    node_library: Arc<NodeLibrary>,
}

impl NodeGraphViewer {
    pub fn new(node_library: Arc<NodeLibrary>) -> Self {
        Self { node_library }
    }
}

impl SnarlViewer<NodeData> for NodeGraphViewer {
    fn title(&mut self, node: &NodeData) -> String {
        self.node_library
            .get_definition(&node.definition_name)
            .map(|def| def.node.name.clone())
            .unwrap_or_else(|| node.definition_name.clone())
    }

    fn inputs(&mut self, node: &NodeData) -> usize {
        self.node_library
            .get_definition(&node.definition_name)
            .map(|def| def.node.inputs.len())
            .unwrap_or(0)
    }

    fn outputs(&mut self, node: &NodeData) -> usize {
        self.node_library
            .get_definition(&node.definition_name)
            .map(|def| def.node.outputs.len())
            .unwrap_or(0)
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeData>,
    ) -> impl egui_snarl::ui::SnarlPin + 'static {
        let node_name = snarl[pin.id.node].definition_name.clone();
        if let Some(def) = self.node_library.get_definition(&node_name) {
            if let Some(input_def) = def.node.inputs.get(pin.id.input) {
                ui.label(&input_def.name);

                // Show input configuration UI if no connection
                if pin.remotes.is_empty() {
                    let node_data = &mut snarl[pin.id.node];

                    match &input_def.kind {
                        NodeInputKind::File { .. } => {
                            let current_value = node_data.input_values.get(&input_def.name);
                            let display_text = if let Some(InputValue::File(path)) = current_value {
                                path.to_string_lossy()
                            } else {
                                "Select file...".into()
                            };

                            if ui.button(display_text).clicked() {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    node_data
                                        .input_values
                                        .insert(input_def.name.clone(), InputValue::File(path));
                                }
                            }
                        }
                        NodeInputKind::Bool { .. } => {
                            let mut value = if let Some(InputValue::Bool(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *v
                            } else {
                                false
                            };

                            if ui.checkbox(&mut value, "").changed() {
                                node_data
                                    .input_values
                                    .insert(input_def.name.clone(), InputValue::Bool(value));
                            }
                        }
                        NodeInputKind::Int { min, max, .. } => {
                            let mut value = if let Some(InputValue::Int(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *v
                            } else {
                                0
                            };

                            let changed = if let (Some(min_val), Some(max_val)) = (min, max) {
                                ui.add(egui::Slider::new(&mut value, *min_val..=*max_val))
                                    .changed()
                            } else {
                                ui.add(egui::DragValue::new(&mut value)).changed()
                            };

                            if changed {
                                node_data
                                    .input_values
                                    .insert(input_def.name.clone(), InputValue::Int(value));
                            }
                        }
                        NodeInputKind::Float { min, max, .. } => {
                            let mut value = if let Some(InputValue::Float(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *v
                            } else {
                                0.0
                            };

                            let changed = if let (Some(min_val), Some(max_val)) = (min, max) {
                                ui.add(egui::Slider::new(&mut value, *min_val..=*max_val))
                                    .changed()
                            } else {
                                ui.add(egui::DragValue::new(&mut value).speed(0.1))
                                    .changed()
                            };

                            if changed {
                                node_data
                                    .input_values
                                    .insert(input_def.name.clone(), InputValue::Float(value));
                            }
                        }
                        NodeInputKind::Text { .. } => {
                            let mut value = if let Some(InputValue::Text(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                v.clone()
                            } else {
                                String::new()
                            };

                            if ui.text_edit_singleline(&mut value).changed() {
                                node_data
                                    .input_values
                                    .insert(input_def.name.clone(), InputValue::Text(value));
                            }
                        }
                        NodeInputKind::Dimensions { .. } => {
                            let (mut width, mut height) =
                                if let Some(InputValue::Dimensions { width, height }) =
                                    node_data.input_values.get(&input_def.name)
                                {
                                    (*width, *height)
                                } else {
                                    (1920, 1080)
                                };

                            let mut changed = ui
                                .add(egui::DragValue::new(&mut width).prefix("W: "))
                                .changed();
                            changed |= ui
                                .add(egui::DragValue::new(&mut height).prefix("H: "))
                                .changed();

                            if changed {
                                node_data.input_values.insert(
                                    input_def.name.clone(),
                                    InputValue::Dimensions { width, height },
                                );
                            }
                        }
                        NodeInputKind::Pixel { .. } => {
                            let (r, g, b, a) = if let Some(InputValue::Pixel { r, g, b, a }) =
                                node_data.input_values.get(&input_def.name)
                            {
                                (*r, *g, *b, *a)
                            } else {
                                (0.0, 0.0, 0.0, 1.0)
                            };

                            let mut color = egui::Color32::from_rgba_premultiplied(
                                (r * 255.0) as u8,
                                (g * 255.0) as u8,
                                (b * 255.0) as u8,
                                (a * 255.0) as u8,
                            );

                            if ui.color_edit_button_srgba(&mut color).changed() {
                                let [r_u8, g_u8, b_u8, a_u8] = color.to_array();
                                node_data.input_values.insert(
                                    input_def.name.clone(),
                                    InputValue::Pixel {
                                        r: r_u8 as f32 / 255.0,
                                        g: g_u8 as f32 / 255.0,
                                        b: b_u8 as f32 / 255.0,
                                        a: a_u8 as f32 / 255.0,
                                    },
                                );
                            }
                        }
                        NodeInputKind::Frame | NodeInputKind::Midi => {
                            ui.label("(must be connected)");
                        }
                        NodeInputKind::Enum { choices, .. } => {
                            let mut selected_idx = if let Some(InputValue::Enum(idx)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *idx
                            } else {
                                0
                            };

                            egui::ComboBox::from_id_salt(&input_def.name)
                                .selected_text(
                                    choices.get(selected_idx).unwrap_or(&"None".to_string()),
                                )
                                .show_ui(ui, |ui| {
                                    for (idx, option) in choices.iter().enumerate() {
                                        if ui
                                            .selectable_value(&mut selected_idx, idx, option)
                                            .changed()
                                        {
                                            node_data.input_values.insert(
                                                input_def.name.clone(),
                                                InputValue::Enum(idx),
                                            );
                                        }
                                    }
                                });
                        }
                    }
                } else if let Some(remote) = pin.remotes.first() {
                    // Show connected value
                    let remote_node = &snarl[remote.node];
                    ui.label(format!("Connected to {}", remote_node.definition_name));
                }

                let color = input_kind_color(&input_def.kind);
                return PinInfo::circle().with_fill(color);
            }
        }

        ui.label("input");
        PinInfo::circle()
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeData>,
    ) -> impl egui_snarl::ui::SnarlPin + 'static {
        let node_name = &snarl[pin.id.node].definition_name;
        if let Some(def) = self.node_library.get_definition(node_name) {
            if let Some(output_def) = def.node.outputs.get(pin.id.output) {
                ui.label(&output_def.name);
                let color = output_kind_color(&output_def.kind);
                return PinInfo::circle().with_fill(color);
            }
        }

        ui.label("output");
        PinInfo::circle()
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<NodeData>) -> bool {
        true
    }

    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<NodeData>) {
        ui.label("Add Node");
        ui.separator();

        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                let mut definitions: Vec<_> = self.node_library.definitions().iter().collect();
                definitions.sort_by(|(_, a), (_, b)| a.node.name.cmp(&b.node.name));

                for (definition_name, definition) in definitions {
                    if ui.button(&definition.node.name).clicked() {
                        snarl.insert_node(
                            pos,
                            NodeData {
                                definition_name: definition_name.clone(),
                                input_values: HashMap::new(),
                                engine_node_id: None,
                            },
                        );
                        ui.close();
                    }
                }
            });
    }
}

fn input_kind_color(kind: &NodeInputKind) -> egui::Color32 {
    output_kind_color(&input_kind_to_output_kind(kind))
}

fn output_kind_color(kind: &NodeOutputKind) -> egui::Color32 {
    match kind {
        NodeOutputKind::Bool => egui::Color32::from_rgb(200, 100, 100),
        NodeOutputKind::Int => egui::Color32::from_rgb(100, 200, 100),
        NodeOutputKind::Float => egui::Color32::from_rgb(100, 100, 200),
        NodeOutputKind::Frame => egui::Color32::from_rgb(200, 200, 100),
        NodeOutputKind::Midi => egui::Color32::from_rgb(100, 200, 200),
        NodeOutputKind::Dimensions => egui::Color32::from_rgb(200, 100, 200),
        NodeOutputKind::Pixel => egui::Color32::from_rgb(150, 150, 150),
        NodeOutputKind::Text => egui::Color32::from_rgb(255, 165, 0),
    }
}

impl NodeGraphState {
    /// Check if a node has all its required inputs satisfied (connected or with defaults)
    /// This is RECURSIVE - it checks that source nodes are also satisfied
    fn are_inputs_satisfied(
        &self,
        node_id: SnarlNodeId,
        node_library: &NodeLibrary,
    ) -> bool {
        let node = &self.snarl[node_id];
        let Some(definition) = node_library.get_definition(&node.definition_name) else {
            return false;
        };

        // Check if this is a source node (has File input)
        let is_source = definition
            .node
            .inputs
            .iter()
            .any(|input| matches!(input.kind, NodeInputKind::File { .. }));

        // Source nodes must have a file configured
        if is_source {
            let has_file = node
                .input_values
                .values()
                .any(|v| matches!(v, InputValue::File(_)));
            if !has_file {
                return false;
            }
        }

        // Build a set of connected input names for this node
        let connected_inputs: std::collections::HashSet<String> = self
            .snarl
            .wires()
            .filter_map(|(_, wire_to)| {
                if wire_to.node == node_id {
                    definition
                        .node
                        .inputs
                        .get(wire_to.input)
                        .map(|inp| inp.name.clone())
                } else {
                    None
                }
            })
            .collect();

        // Check all Frame inputs are either connected or have defaults
        for input_def in &definition.node.inputs {
            if matches!(input_def.kind, NodeInputKind::Frame) {
                // Frame inputs have no defaults, so must be connected
                if !connected_inputs.contains(&input_def.name) {
                    return false;
                }
            }
        }

        // RECURSIVE CHECK: For each connected input, verify the source node is also satisfied
        for (wire_from, wire_to) in self.snarl.wires() {
            if wire_to.node == node_id {
                let source_node = wire_from.node;
                // Recursively check that the source node is also satisfied
                if !self.are_inputs_satisfied(source_node, node_library) {
                    return false;
                }
            }
        }

        true
    }

    /// Sync the entire node graph to the engine
    pub fn sync_to_engine(&mut self, engine_graph: &mut NodeGraph, node_library: &NodeLibrary) {
        // Collect all snarl node IDs (must do this before mutating)
        let all_node_ids: Vec<_> = self.snarl.node_ids().map(|(id, _)| id).collect();

        // First, identify which nodes should be in the engine
        let mut to_add = Vec::new();
        let mut to_remove = Vec::new();

        for node_id in &all_node_ids {
            let node = &self.snarl[*node_id];
            let is_in_engine = node.engine_node_id.is_some();

            // Check if this node should be in the engine
            let definition_name = &node.definition_name;
            let Some(definition) = node_library.get_definition(definition_name) else {
                continue;
            };
            
            // Check if this node is a source (has File input) by checking the definition
            let is_source = definition
                .node
                .inputs
                .iter()
                .any(|input| matches!(input.kind, NodeInputKind::File { .. }));
            
            let has_file = node
                .input_values
                .values()
                .any(|v| matches!(v, InputValue::File(_)));
            let inputs_satisfied = self.are_inputs_satisfied(*node_id, node_library);
            let should_be_in_engine = if is_source {
                has_file && inputs_satisfied // Source nodes need configured file
            } else {
                // Non-source nodes: ALL Frame inputs must be connected AND satisfied recursively
                inputs_satisfied
            };

            if should_be_in_engine && !is_in_engine {
                to_add.push(*node_id);
            } else if !should_be_in_engine && is_in_engine {
                to_remove.push(*node_id);
            }
        }

        // Remove disconnected nodes
        for node_id in to_remove {
            self.remove_node_from_engine(node_id, engine_graph);
        }

        // Add newly connected nodes
        for node_id in to_add {
            self.sync_node_to_engine(node_id, engine_graph, node_library);
        }

        // Update input values for nodes already in engine
        // Preserve existing connection inputs so we don't clobber them
        for node_id in &all_node_ids {
            let node = &self.snarl[*node_id];
            if let Some(engine_id) = node.engine_node_id {
                if let Some(instance) = engine_graph.get_instance_mut(engine_id) {
                    let mut merged_inputs = instance.input_values.clone();
                    for (key, value) in &node.input_values {
                        merged_inputs.insert(key.clone(), value.clone());
                    }
                    instance.input_values = merged_inputs;
                }
            }
        }

        // Sync all wires
        for node_id in &all_node_ids {
            if self.snarl[*node_id].engine_node_id.is_some() {
                self.sync_wires_for_node(*node_id, engine_graph, node_library);
            }
        }
    }

    /// Check if a node is a source node (video/image) with a configured file
    fn is_active_source(&self, node_id: SnarlNodeId, node_library: &NodeLibrary) -> bool {
        let node = &self.snarl[node_id];
        let Some(definition) = node_library.get_definition(&node.definition_name) else {
            return false;
        };

        let is_source = definition
            .node
            .inputs
            .iter()
            .any(|input| matches!(input.kind, NodeInputKind::File { .. }));

        // Check if file input is configured
        is_source
            && node
                .input_values
                .values()
                .any(|v| matches!(v, InputValue::File(_)))
    }

    /// Check if a node is connected (directly or indirectly) to an active source
    fn is_connected_to_source(&self, node_id: SnarlNodeId, node_library: &NodeLibrary) -> bool {
        // If it's an active source itself, return true
        if self.is_active_source(node_id, node_library) {
            return true;
        }

        // Use BFS to check if any input leads to an active source
        let mut visited = HashSet::new();
        let mut queue = vec![node_id];

        while let Some(current_id) = queue.pop() {
            if visited.contains(&current_id) {
                continue;
            }
            visited.insert(current_id);

            // Check all input connections
            for (wire_from, wire_to) in self.snarl.wires() {
                if wire_to.node == current_id {
                    // This node receives input from wire_from.node
                    let source_node = wire_from.node;
                    if self.is_active_source(source_node, node_library) {
                        return true;
                    }
                    queue.push(source_node);
                }
            }
        }

        false
    }

    /// Sync a node to the engine if it should be active
    pub fn sync_node_to_engine(
        &mut self,
        node_id: SnarlNodeId,
        engine_graph: &mut NodeGraph,
        node_library: &NodeLibrary,
    ) {
        // Check if already in engine
        if self.snarl[node_id].engine_node_id.is_some() {
            return;
        }

        // Check conditions before borrowing mutably
        let definition_name = self.snarl[node_id].definition_name.clone();
        let input_values = self.snarl[node_id].input_values.clone();
        let Some(definition) = node_library.get_definition(&definition_name) else {
            return;
        };
        let is_source = definition
            .node
            .inputs
            .iter()
            .any(|input| matches!(input.kind, NodeInputKind::File { .. }));
        let has_file = input_values
            .values()
            .any(|v| matches!(v, InputValue::File(_)));
        let should_add = if is_source {
            has_file
        } else {
            self.is_connected_to_source(node_id, node_library)
        };

        if !should_add {
            return;
        }

        // Add to engine
        let engine_id = engine_graph.add_instance(definition_name);

        // Copy input values to engine instance
        if let Some(instance) = engine_graph.get_instance_mut(engine_id) {
            instance.input_values = input_values;
        }

        // Update node with engine ID
        self.snarl[node_id].engine_node_id = Some(engine_id);

        // Sync all wire connections for this node
        self.sync_wires_for_node(node_id, engine_graph, node_library);
    }

    /// Remove a node from the engine
    pub fn remove_node_from_engine(&mut self, node_id: SnarlNodeId, engine_graph: &mut NodeGraph) {
        let node = &mut self.snarl[node_id];
        if let Some(engine_id) = node.engine_node_id.take() {
            engine_graph.remove_instance(engine_id);
        }
    }

    /// Sync wire connections for a specific node
    fn sync_wires_for_node(
        &mut self,
        node_id: SnarlNodeId,
        engine_graph: &mut NodeGraph,
        node_library: &NodeLibrary,
    ) {
        let node = &self.snarl[node_id];
        let Some(to_engine_id) = node.engine_node_id else {
            return;
        };

        // Get node definition to map input indices to names
        let Some(definition) = node_library.get_definition(&node.definition_name) else {
            return;
        };

        // Clear existing engine connections for this node's inputs
        for input_def in &definition.node.inputs {
            engine_graph.disconnect(to_engine_id, &input_def.name);
        }

        // Connect all inputs
        for (wire_from, wire_to) in self.snarl.wires() {
            if wire_to.node != node_id {
                continue;
            }

            let from_node = &self.snarl[wire_from.node];
            let Some(from_engine_id) = from_node.engine_node_id else {
                continue;
            };

            // Get output and input names
            let Some(from_definition) = node_library.get_definition(&from_node.definition_name)
            else {
                continue;
            };

            let Some(output_def) = from_definition.node.outputs.get(wire_from.output) else {
                continue;
            };
            let Some(input_def) = definition.node.inputs.get(wire_to.input) else {
                continue;
            };

            // Connect in engine graph
            let _ = engine_graph.connect(
                from_engine_id,
                output_def.name.clone(),
                to_engine_id,
                input_def.name.clone(),
            );
        }
    }
}
