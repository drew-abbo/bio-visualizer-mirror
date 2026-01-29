use std::borrow::Cow;
use std::collections::HashMap;

use crate::view::View;
use egui::Color32;
use egui_node_editor::*;
use engine::node::NodeDefinition;
use slotmap::Key;
use engine::node::node::{NodeInputKind, NodeOutputKind};

#[derive(Clone, Debug, Default)]
struct UserState {
    node_definitions: HashMap<String, NodeDefinition>,
}

#[derive(Clone, Debug)]
struct NodeData {
    #[allow(dead_code)]
    definition_name: String,
}

// Map engine types to editor data types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DataType {
    Frame,
    Midi,
    Bool,
    Int,
    Float,
    Dimensions,
    Pixel,
    Text,
    File,
}

impl DataTypeTrait<UserState> for DataType {
    fn data_type_color(&self, _user_state: &mut UserState) -> Color32 {
        match self {
            DataType::Frame => Color32::from_rgb(120, 200, 255),
            DataType::Midi => Color32::from_rgb(255, 120, 200),
            DataType::Bool => Color32::from_rgb(200, 100, 100),
            DataType::Int => Color32::from_rgb(100, 200, 100),
            DataType::Float => Color32::from_rgb(100, 150, 200),
            DataType::Dimensions => Color32::from_rgb(200, 200, 100),
            DataType::Pixel => Color32::from_rgb(255, 150, 50),
            DataType::Text => Color32::from_rgb(150, 255, 150),
            DataType::File => Color32::from_rgb(200, 150, 200),
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            DataType::Frame => Cow::Borrowed("Frame"),
            DataType::Midi => Cow::Borrowed("Midi"),
            DataType::Bool => Cow::Borrowed("Bool"),
            DataType::Int => Cow::Borrowed("Int"),
            DataType::Float => Cow::Borrowed("Float"),
            DataType::Dimensions => Cow::Borrowed("Dimensions"),
            DataType::Pixel => Cow::Borrowed("Pixel"),
            DataType::Text => Cow::Borrowed("Text"),
            DataType::File => Cow::Borrowed("File"),
        }
    }
}

impl DataType {
    fn from_input_kind(kind: &NodeInputKind) -> Self {
        match kind {
            NodeInputKind::Frame => DataType::Frame,
            NodeInputKind::Midi => DataType::Midi,
            NodeInputKind::Bool { .. } => DataType::Bool,
            NodeInputKind::Int { .. } => DataType::Int,
            NodeInputKind::Float { .. } => DataType::Float,
            NodeInputKind::Dimensions { .. } => DataType::Dimensions,
            NodeInputKind::Pixel { .. } => DataType::Pixel,
            NodeInputKind::Text { .. } => DataType::Text,
            NodeInputKind::File { .. } => DataType::File,
            NodeInputKind::Enum { .. } => DataType::Text, // Treat enum as text for now
        }
    }

    fn from_output_kind(kind: &NodeOutputKind) -> Self {
        match kind {
            NodeOutputKind::Frame => DataType::Frame,
            NodeOutputKind::Midi => DataType::Midi,
            NodeOutputKind::Bool => DataType::Bool,
            NodeOutputKind::Int => DataType::Int,
            NodeOutputKind::Float => DataType::Float,
            NodeOutputKind::Dimensions => DataType::Dimensions,
            NodeOutputKind::Pixel => DataType::Pixel,
            NodeOutputKind::Text => DataType::Text,
        }
    }
}

// Value types for node inputs with UI widgets
#[derive(Clone, Debug, PartialEq)]
enum ValueType {
    Frame,
    Midi,
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions(u32, u32),
    Pixel([f32; 4]),
    Text(String),
    File(String),
    Enum { choices: Vec<String>, selected: usize },
}

impl Default for ValueType {
    fn default() -> Self {
        ValueType::Float(0.0)
    }
}

impl ValueType {
    fn from_input_kind(kind: &NodeInputKind) -> Self {
        match kind {
            NodeInputKind::Frame => ValueType::Frame,
            NodeInputKind::Midi => ValueType::Midi,
            NodeInputKind::Bool { default } => ValueType::Bool(*default),
            NodeInputKind::Int { default, .. } => ValueType::Int(*default),
            NodeInputKind::Float { default, .. } => ValueType::Float(*default),
            NodeInputKind::Dimensions { default } => ValueType::Dimensions(default.0, default.1),
            NodeInputKind::Pixel { default, .. } => ValueType::Pixel(*default),
            NodeInputKind::Text { default, .. } => ValueType::Text(default.clone()),
            NodeInputKind::File { default, .. } => {
                let path_str: &str = default.as_ref().and_then(|p| p.to_str()).unwrap_or("");
                ValueType::File(path_str.to_string())
            }
            NodeInputKind::Enum { choices, default_idx } => ValueType::Enum {
                choices: choices.clone(),
                selected: default_idx.unwrap_or(0),
            },
        }
    }
}

#[derive(Clone, Debug)]
struct MyResponse;

impl UserResponseTrait for MyResponse {}

impl WidgetValueTrait for ValueType {
    type Response = MyResponse;
    type UserState = UserState;
    type NodeData = NodeData;

    fn value_widget(
        &mut self,
        param_name: &str,
        _node_id: NodeId,
        ui: &mut egui::Ui,
        _user_state: &mut UserState,
        _node_data: &NodeData,
    ) -> Vec<MyResponse> {
        match self {
            ValueType::Frame | ValueType::Midi => {
                ui.label(format!("{}: (connected)", param_name));
            }
            ValueType::Bool(val) => {
                ui.checkbox(val, param_name);
            }
            ValueType::Int(val) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(egui::DragValue::new(val));
                });
            }
            ValueType::Float(val) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(egui::DragValue::new(val).speed(0.1));
                });
            }
            ValueType::Dimensions(w, h) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(egui::DragValue::new(w).prefix("W: "));
                    ui.add(egui::DragValue::new(h).prefix("H: "));
                });
            }
            ValueType::Pixel(color) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.color_edit_button_rgba_unmultiplied(color);
                });
            }
            ValueType::Text(text) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.text_edit_singleline(text);
                });
            }
            ValueType::File(path) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.text_edit_singleline(path);
                    if ui.button("📁").clicked() {
                        if let Some(file) = rfd::FileDialog::new()
                            .add_filter("Video", &["mp4", "mov", "mkv"])
                            .pick_file()
                        {
                            *path = file.to_string_lossy().to_string();
                        }
                    }
                });
            }
            ValueType::Enum { choices, selected } => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    egui::ComboBox::from_id_salt(format!("enum_{}", param_name))
                        .selected_text(&choices[*selected])
                        .show_ui(ui, |ui| {
                            for (i, choice) in choices.iter().enumerate() {
                                ui.selectable_value(selected, i, choice);
                            }
                        });
                });
            }
        }
        Vec::new()
    }
}

impl NodeDataTrait for NodeData {
    type Response = MyResponse;
    type UserState = UserState;
    type DataType = DataType;
    type ValueType = ValueType;

    fn bottom_ui(
        &self,
        _ui: &mut egui::Ui,
        _node_id: NodeId,
        _graph: &Graph<NodeData, DataType, ValueType>,
        _user_state: &mut UserState,
    ) -> Vec<NodeResponse<MyResponse, NodeData>>
    where
        MyResponse: UserResponseTrait,
    {
        Vec::new()
    }
}

// Dynamic node template based on loaded node definitions
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct NodeTemplate {
    definition_name: String,
}

impl NodeTemplateTrait for NodeTemplate {
    type NodeData = NodeData;
    type DataType = DataType;
    type ValueType = ValueType;
    type UserState = UserState;
    type CategoryType = String;

    fn node_finder_label(&self, _user_state: &mut Self::UserState) -> Cow<'_, str> {
        Cow::Owned(self.definition_name.clone())
    }

    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<Self::CategoryType> {
        vec!["Nodes".to_string()] // TODO: Use sub_folders from NodeDefinition
    }

    fn node_graph_label(&self, _user_state: &mut Self::UserState) -> String {
        self.definition_name.clone()
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        NodeData {
            definition_name: self.definition_name.clone(),
        }
    }

    fn build_node(
        &self,
        graph: &mut Graph<Self::NodeData, Self::DataType, Self::ValueType>,
        user_state: &mut Self::UserState,
        node_id: NodeId,
    ) {
        // Build node from definition in user_state
        if let Some(def) = user_state.node_definitions.get(&self.definition_name) {
            // Add inputs
            for input in &def.node.inputs {
                let data_type = DataType::from_input_kind(&input.kind);
                let value_type = ValueType::from_input_kind(&input.kind);
                
                // Determine if this input can be constant or must be connected
                let input_kind = match &input.kind {
                    NodeInputKind::Frame | NodeInputKind::Midi => InputParamKind::ConnectionOnly,
                    _ => InputParamKind::ConnectionOrConstant,
                };

                graph.add_input_param(
                    node_id,
                    input.name.clone(),
                    data_type,
                    value_type,
                    input_kind,
                    true,
                );
            }

            // Add outputs
            for output in &def.node.outputs {
                let data_type = DataType::from_output_kind(&output.kind);
                graph.add_output_param(node_id, output.name.clone(), data_type);
            }
        }
    }
}

struct DynamicNodeTemplates {
    templates: Vec<NodeTemplate>,
}

impl NodeTemplateIter for DynamicNodeTemplates {
    type Item = NodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        self.templates.clone()
    }
}

type GraphState = GraphEditorState<NodeData, DataType, ValueType, NodeTemplate, UserState>;

pub struct NodeGraphView {
    state: GraphState,
    user_state: UserState,
    selected_node_id: Option<NodeId>,
    // Map input/output parameter IDs to their names
    input_param_names: HashMap<InputId, String>,
    output_param_names: HashMap<OutputId, String>,
}

impl NodeGraphView {
    pub fn new() -> Self {
        Self {
            state: GraphState::default(),
            user_state: UserState::default(),
            selected_node_id: None,
            input_param_names: HashMap::new(),
            output_param_names: HashMap::new(),
        }
    }

    pub fn set_node_library(&mut self, definitions: HashMap<String, NodeDefinition>) {
        self.user_state.node_definitions = definitions;
    }

    /// Get the currently selected node ID, if any
    pub fn selected_node_id(&self) -> Option<NodeId> {
        self.selected_node_id
    }

    /// Get the engine node ID for an editor node ID
    pub fn get_engine_node_id(&self, editor_node_id: NodeId) -> Option<engine::node_graph::NodeId> {
        // Convert egui NodeId to engine NodeId using index()
        Some(engine::node_graph::NodeId::new(editor_node_id.data().as_ffi() as u32))
    }

    /// Build an engine NodeGraph from the current editor state
    pub fn build_engine_graph(&mut self) -> engine::node_graph::NodeGraph {
        use engine::node_graph::NodeGraph as EngineNodeGraph;
        
        let mut engine_graph = EngineNodeGraph::new();

        // Clear parameter name mappings
        self.input_param_names.clear();
        self.output_param_names.clear();

        // Step 1: Add all nodes to the engine graph and build parameter name mappings
        for editor_node_id in self.state.graph.iter_nodes() {
            let editor_node = &self.state.graph[editor_node_id];
            let engine_node_id = engine::node_graph::NodeId::new(editor_node_id.data().as_ffi() as u32);
            // Use the same NodeId directly
            engine_graph.add_instance_with_id(engine_node_id, editor_node.user_data.definition_name.clone());
            
            // Build parameter name mappings from the definition
            if let Some(def) = self.user_state.node_definitions.get(&editor_node.user_data.definition_name) {
                // Map input IDs to names - inputs are added in order
                let input_ids: Vec<_> = editor_node.input_ids().collect();
                for (idx, input_def) in def.node.inputs.iter().enumerate() {
                    if idx < input_ids.len() {
                        self.input_param_names.insert(input_ids[idx], input_def.name.clone());
                    }
                }
                
                // Map output IDs to names - outputs are added in order
                let output_ids: Vec<_> = editor_node.output_ids().collect();
                for (idx, output_def) in def.node.outputs.iter().enumerate() {
                    if idx < output_ids.len() {
                        self.output_param_names.insert(output_ids[idx], output_def.name.clone());
                    }
                }
            }
        }

        // Step 2: Extract input values from node parameters
        for editor_node_id in self.state.graph.iter_nodes() {
            let editor_node = &self.state.graph[editor_node_id];
            let engine_node_id = engine::node_graph::NodeId::new(editor_node_id.data().as_ffi() as u32);
            
            // Iterate through input parameters of this node
            for input_id in editor_node.input_ids() {
                let input_param = &self.state.graph[input_id];
                
                // Check if this input is connected
                let is_connected = self.state.graph.connection(input_id).is_some();
                
                // If not connected, use the constant value from the input
                if !is_connected {
                    if let Some(input_name) = self.input_param_names.get(&input_id) {
                        let input_value = Self::convert_value_type_to_input_value(&input_param.value);
                        let _ = engine_graph.set_input_value(engine_node_id, input_name.clone(), input_value);
                    }
                }
            }
        }

        // Step 3: Extract connections between nodes
        for (target_input_id, source_output_id) in self.state.graph.iter_connections() {
            // Get the input parameter to find which node it belongs to
            let target_input = &self.state.graph[target_input_id];
            let target_node_id = engine::node_graph::NodeId::new(target_input.node.data().as_ffi() as u32);
            
            // Get the output parameter to find which node it belongs to
            let source_output = &self.state.graph[source_output_id];
            let source_node_id = engine::node_graph::NodeId::new(source_output.node.data().as_ffi() as u32);
            
            // Get the names from our mappings
            if let (Some(target_input_name), Some(source_output_name)) = (
                self.input_param_names.get(&target_input_id),
                self.output_param_names.get(&source_output_id),
            ) {
                // Create the connection in the engine graph
                let _ = engine_graph.connect(
                    source_node_id,
                    source_output_name.clone(),
                    target_node_id,
                    target_input_name.clone(),
                );
            }
        }

        engine_graph
    }

    /// Convert a ValueType from the UI to an engine InputValue
    fn convert_value_type_to_input_value(value: &ValueType) -> engine::node_graph::InputValue {
        use engine::node_graph::InputValue;
        
        match value {
            ValueType::Frame => InputValue::Frame,
            ValueType::Midi => InputValue::Frame, // Placeholder - no Midi type in InputValue yet
            ValueType::Bool(b) => InputValue::Bool(*b),
            ValueType::Int(i) => InputValue::Int(*i),
            ValueType::Float(f) => InputValue::Float(*f),
            ValueType::Dimensions(w, h) => InputValue::Dimensions { width: *w, height: *h },
            ValueType::Pixel(colors) => InputValue::Pixel {
                r: colors[0],
                g: colors[1],
                b: colors[2],
                a: colors[3],
            },
            ValueType::Text(s) => InputValue::Text(s.clone()),
            ValueType::File(path) => InputValue::File(std::path::PathBuf::from(path)),
            ValueType::Enum { selected, .. } => InputValue::Enum(*selected),
        }
    }
}

impl View for NodeGraphView {
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Build node templates from library
        let templates = DynamicNodeTemplates {
            templates: self.user_state.node_definitions.keys().map(|name| NodeTemplate {
                definition_name: name.clone(),
            }).collect(),
        };

        // Draw the graph editor
        let graph_response = self.state.draw_graph_editor(
            ui,
            templates,
            &mut self.user_state,
            Vec::default(),
        );

        // Track node selection from responses
        for response in &graph_response.node_responses {
            if let NodeResponse::SelectNode(node_id) = response {
                self.selected_node_id = Some(*node_id);
            }
        }
    }
}

