use egui_snarl::ui::{PinInfo, SnarlViewer};
use egui_snarl::{InPin, NodeId as SnarlNodeId, OutPin, Snarl};
use engine::node::node::{NodeInput, NodeOutputKind};
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
                            // We can restrict the kind of file based on the node name.
                            // since those nodes are not user defined.

                            let current_value = node_data.input_values.get(&input_def.name);
                            let display_text = if let Some(InputValue::File(path)) = current_value {
                                path.to_string_lossy()
                            } else {
                                "Select file...".into()
                            };

                            if ui.button(display_text).clicked() {
                                // Create file dialog with appropriate filters based on node name
                                // Not sure if everything is supported but for now I will just add this

                                let mut dialog = rfd::FileDialog::new();

                                if let Some(def) = self.node_library.get_definition(&node_name) {
                                    match def.node.name.as_str() {
                                        "Video" => {
                                            dialog = dialog.add_filter(
                                                "Video Files",
                                                &[
                                                    "mp4", "avi", "mov", "mkv", "webm", "flv",
                                                    "wmv", "m4v", "mpg", "mpeg",
                                                ],
                                            );
                                        }
                                        "Image" => {
                                            dialog = dialog.add_filter(
                                                "Image Files",
                                                &[
                                                    "png", "jpg", "jpeg", "bmp", "gif", "tiff",
                                                    "tif", "webp", "ico",
                                                ],
                                            );
                                        }
                                        _ => {
                                            // Default: all files
                                        }
                                    }
                                }

                                if let Some(path) = dialog.pick_file() {
                                    node_data
                                        .input_values
                                        .insert(input_def.name.clone(), InputValue::File(path));
                                }
                            }
                        }
                        NodeInputKind::Bool { default } => {
                            let mut value = if let Some(InputValue::Bool(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *v
                            } else {
                                *default
                            };

                            if ui.checkbox(&mut value, "").changed() {
                                node_data
                                    .input_values
                                    .insert(input_def.name.clone(), InputValue::Bool(value));
                            }
                        }
                        NodeInputKind::Int {
                            default, min, max, ..
                        } => {
                            let mut value = if let Some(InputValue::Int(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *v
                            } else {
                                *default
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
                        NodeInputKind::Float {
                            default, min, max, ..
                        } => {
                            let mut value = if let Some(InputValue::Float(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *v
                            } else {
                                *default
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
                        NodeInputKind::Text { default, .. } => {
                            let mut value = if let Some(InputValue::Text(v)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                v.clone()
                            } else {
                                default.clone()
                            };

                            if ui.text_edit_singleline(&mut value).changed() {
                                node_data
                                    .input_values
                                    .insert(input_def.name.clone(), InputValue::Text(value));
                            }
                        }
                        NodeInputKind::Dimensions { default } => {
                            let (mut width, mut height) =
                                if let Some(InputValue::Dimensions { width, height }) =
                                    node_data.input_values.get(&input_def.name)
                                {
                                    (*width, *height)
                                } else {
                                    *default
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
                        NodeInputKind::Pixel { default, .. } => {
                            let (r, g, b, a) = if let Some(InputValue::Pixel { r, g, b, a }) =
                                node_data.input_values.get(&input_def.name)
                            {
                                (*r, *g, *b, *a)
                            } else {
                                (default[0], default[1], default[2], default[3])
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
                        NodeInputKind::Enum {
                            choices,
                            default_idx,
                            ..
                        } => {
                            let mut selected_idx = if let Some(InputValue::Enum(idx)) =
                                node_data.input_values.get(&input_def.name)
                            {
                                *idx
                            } else {
                                default_idx.unwrap_or(0)
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

    fn has_node_menu(&mut self, _node: &NodeData) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        node_id: SnarlNodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeData>,
    ) {
        if ui.button("Delete Node").clicked() {
            snarl.remove_node(node_id);
            ui.close();
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeData>) {
        // Validate connection types
        let from_node = &snarl[from.id.node];
        let to_node = &snarl[to.id.node];

        let Some(from_def) = self.node_library.get_definition(&from_node.definition_name) else {
            return;
        };
        let Some(to_def) = self.node_library.get_definition(&to_node.definition_name) else {
            return;
        };

        let Some(from_output) = from_def.node.outputs.get(from.id.output) else {
            return;
        };
        let Some(to_input) = to_def.node.inputs.get(to.id.input) else {
            return;
        };

        // Check if types are compatible
        let output_kind = &from_output.kind;
        let expected_output_kind = input_kind_to_output_kind(&to_input.kind);

        // Only allow connection if types match
        if output_kind != &expected_output_kind {
            return;
        }

        // Types match - create the connection
        snarl.connect(from.id, to.id);
    }

    fn drop_inputs(&mut self, _pin: &InPin, _snarl: &mut Snarl<NodeData>) {
        // Allow dropping all input connections (default behavior)
    }

    fn drop_outputs(&mut self, _pin: &OutPin, _snarl: &mut Snarl<NodeData>) {
        // Allow dropping all output connections (default behavior)
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
    fn are_inputs_satisfied(&self, node_id: SnarlNodeId, node_library: &NodeLibrary) -> bool {
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

        // Collect all engine node IDs that are still in the snarl
        let snarl_engine_ids: HashSet<EngineNodeId> = all_node_ids
            .iter()
            .filter_map(|node_id| self.snarl[*node_id].engine_node_id)
            .collect();

        // Remove engine nodes that are no longer in the snarl (were deleted)
        let all_engine_ids: Vec<EngineNodeId> =
            engine_graph.instances().iter().map(|(id, _)| *id).collect();

        for engine_id in all_engine_ids {
            if !snarl_engine_ids.contains(&engine_id) {
                engine_graph.remove_instance(engine_id);
            }
        }

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
                    let Some(definition) = node_library.get_definition(&node.definition_name)
                    else {
                        continue;
                    };

                    let connected_inputs = self.connected_input_names(*node_id, definition);

                    let mut merged_inputs = instance.input_values.clone();

                    // Apply defaults for non-connected inputs if missing
                    for input_def in &definition.node.inputs {
                        if connected_inputs.contains(&input_def.name) {
                            continue;
                        }
                        if !merged_inputs.contains_key(&input_def.name) {
                            if let Some(default_val) = Self::default_input_value(input_def) {
                                merged_inputs.insert(input_def.name.clone(), default_val);
                            }
                        }
                    }

                    // Apply user-provided values for non-connected inputs
                    for (key, value) in &node.input_values {
                        if connected_inputs.contains(key) {
                            continue;
                        }
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

        // Copy input values to engine instance (with defaults for non-connected inputs)
        if let Some(instance) = engine_graph.get_instance_mut(engine_id) {
            let connected_inputs = self.connected_input_names(node_id, definition);
            let mut merged_inputs = HashMap::new();

            for input_def in &definition.node.inputs {
                if connected_inputs.contains(&input_def.name) {
                    continue;
                }
                if let Some(default_val) = Self::default_input_value(input_def) {
                    merged_inputs.insert(input_def.name.clone(), default_val);
                }
            }

            for (key, value) in input_values {
                if connected_inputs.contains(&key) {
                    continue;
                }
                merged_inputs.insert(key, value);
            }

            instance.input_values = merged_inputs;
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
            if let Err(err) = engine_graph.connect(
                from_engine_id,
                output_def.name.clone(),
                to_engine_id,
                input_def.name.clone(),
            ) {
                util::debug_log_error!(
                    "Failed to connect {} output '{}' to {} input '{}': {}",
                    from_engine_id,
                    output_def.name,
                    to_engine_id,
                    input_def.name,
                    err
                );
            }
        }
    }

    fn connected_input_names(
        &self,
        node_id: SnarlNodeId,
        definition: &engine::node::NodeDefinition,
    ) -> HashSet<String> {
        self.snarl
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
            .collect()
    }

    fn default_input_value(input_def: &NodeInput) -> Option<InputValue> {
        match &input_def.kind {
            NodeInputKind::Bool { default } => Some(InputValue::Bool(*default)),
            NodeInputKind::Int { default, .. } => Some(InputValue::Int(*default)),
            NodeInputKind::Float { default, .. } => Some(InputValue::Float(*default)),
            NodeInputKind::Dimensions { default } => Some(InputValue::Dimensions {
                width: default.0,
                height: default.1,
            }),
            NodeInputKind::Pixel { default, .. } => Some(InputValue::Pixel {
                r: default[0],
                g: default[1],
                b: default[2],
                a: default[3],
            }),
            NodeInputKind::Enum { default_idx, .. } => {
                Some(InputValue::Enum(default_idx.unwrap_or(0)))
            }
            NodeInputKind::Text { default, .. } => Some(InputValue::Text(default.clone())),
            NodeInputKind::File { default, .. } => default.clone().map(InputValue::File),
            NodeInputKind::Frame | NodeInputKind::Midi => None,
        }
    }
}
