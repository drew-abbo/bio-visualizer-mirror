//! Node graph editor UI and synchronization with engine graph
//! This module defines the state and UI for the node graph editor, as well as the logic to sync
//! the snarl graph to the engine graph. It also includes validation logic for node connections and input values.
mod colors;
mod input_widgets;
mod sync;
mod validation;

pub use input_widgets::InputWidgetState;
pub use sync::normalize_node_inputs;

use egui;
use egui::emath::TSTransform;
use egui_snarl::ui::{PinInfo, SnarlViewer};
use egui_snarl::{InPin, NodeId as SnarlNodeId, OutPin, Snarl};
use engine::node::engine_node::{BuiltInHandler, NodeExecutionPlan, NodeOutputKind};
use engine::node::{NodeInputKind, NodeLibrary, input_kind_to_output_kind};
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use media::midi::streams::list_ports;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use util::fuzzy_search::{FuzzySearchable, FuzzySearcher};

const VIRTUAL_OUTPUT_SINK_NAME: &str = "__virtual_output_sink__";

fn are_pin_kinds_compatible(output_kind: NodeOutputKind, input_kind: &NodeInputKind) -> bool {
    let expected_output_kind = input_kind_to_output_kind(input_kind);
    output_kind == expected_output_kind
        // Numeric widening: allow Int outputs to feed Float inputs.
        || matches!((output_kind, input_kind), (NodeOutputKind::Int, NodeInputKind::Float { .. }))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct GraphViewState {
    pub scaling: f32,
    pub translation: [f32; 2],
}

impl GraphViewState {
    fn from_transform(transform: TSTransform) -> Self {
        Self {
            scaling: transform.scaling,
            translation: [transform.translation.x, transform.translation.y],
        }
    }

    fn apply_to(self, transform: &mut TSTransform) {
        transform.scaling = self.scaling;
        transform.translation = egui::vec2(self.translation[0], self.translation[1]);
    }
}

/// Data associated with each node in the snarl graph, including its definition and configured input values
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeData {
    pub definition_name: String,
    /// Configured input values for this node
    pub input_values: HashMap<String, InputValue>,

    /// Engine node ID if this node is currently in the engine graph
    #[serde(skip)]
    pub engine_node_id: Option<EngineNodeId>,
}

/// The state of the node graph editor, including the snarl graph and its synchronization with the engine graph
#[derive(Serialize, Deserialize, Clone)]
pub struct NodeGraphState {
    pub snarl: Snarl<NodeData>,
    #[serde(default)]
    pub graph_view: Option<GraphViewState>,
    pub legacy_graph_view_zoom: Option<f32>,
}

/// Needed to impl this since [Snarl<T>] doesn't implement PartialEq.
/// Project needs to be able to compare.
impl PartialEq for NodeGraphState {
    /// Compare two NodeGraphStates by serializing them to binary.
    fn eq(&self, other: &Self) -> bool {
        // Serialize both states to binary and compare the bytes
        let self_binary = postcard::to_allocvec(self).ok();
        let other_binary = postcard::to_allocvec(other).ok();

        // If either serialization fails, consider them not equal
        match (self_binary, other_binary) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }
}

impl NodeGraphState {
    pub fn new() -> Self {
        let mut state = Self {
            snarl: Snarl::new(),
            graph_view: None,
            legacy_graph_view_zoom: None,
        };

        state.ensure_output_sink();
        state
    }

    pub fn ensure_output_sink(&mut self) {
        let has_sink = self
            .snarl
            .node_ids()
            .any(|(node_id, _)| self.snarl[node_id].definition_name == VIRTUAL_OUTPUT_SINK_NAME);

        if has_sink {
            return;
        }

        self.snarl.insert_node(
            egui::pos2(880.0, 220.0),
            NodeData {
                definition_name: VIRTUAL_OUTPUT_SINK_NAME.to_string(),
                input_values: HashMap::new(),
                engine_node_id: None,
            },
        );
    }

    pub fn output_sink_node(&self) -> Option<SnarlNodeId> {
        self.snarl
            .node_ids()
            .map(|(node_id, _)| node_id)
            .find(|node_id| self.snarl[*node_id].definition_name == VIRTUAL_OUTPUT_SINK_NAME)
    }

    pub fn output_source_snarl_node(&self) -> Option<SnarlNodeId> {
        let sink = self.output_sink_node()?;
        self.snarl
            .wires()
            .find_map(|(from, to)| (to.node == sink).then_some(from.node))
    }

    /// Validate persisted runtime-sensitive inputs (for example MIDI ports) so a
    /// project can start cleanly even if external devices changed while the app
    /// was closed.
    pub fn sanitize_runtime_inputs_on_load(&mut self, node_library: &NodeLibrary) -> Vec<String> {
        let available_ports: Vec<String> = match list_ports() {
            Ok(ports) => ports.map(|port| port.port_name().to_string()).collect(),
            Err(error) => {
                util::debug_log_warning!("Failed to list MIDI input ports on load: {}", error);
                Vec::new()
            }
        };

        let mut warnings = Vec::new();
        let node_ids: Vec<SnarlNodeId> = self.snarl.node_ids().map(|(id, _)| id).collect();

        for node_id in node_ids {
            let definition_name = self.snarl[node_id].definition_name.clone();
            let Some(definition) = node_library.get_definition(&definition_name) else {
                continue;
            };

            if !matches!(
                definition.node.executor,
                NodeExecutionPlan::BuiltIn(BuiltInHandler::MidiSource)
            ) {
                continue;
            }

            let port_query = self.snarl[node_id]
                .input_values
                .get("Port")
                .and_then(|value| match value {
                    InputValue::Text(text) => {
                        let trimmed = text.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    }
                    InputValue::Enum(index) => Some(index.to_string()),
                    _ => None,
                });

            if available_ports.is_empty() {
                let outgoing_inputs: Vec<egui_snarl::InPinId> = self
                    .snarl
                    .wires()
                    .filter_map(|(from, to)| (from.node == node_id).then_some(to))
                    .collect();

                if !outgoing_inputs.is_empty() {
                    for input_pin in outgoing_inputs {
                        self.snarl.drop_inputs(input_pin);
                    }

                    warnings.push(format!(
                        "'{}' was disconnected because no MIDI input ports are currently available.",
                        definition.node.name
                    ));
                }

                continue;
            }

            let Some(query) = port_query else {
                continue;
            };

            let query_matches = if let Ok(index) = query.parse::<usize>() {
                index < available_ports.len()
            } else {
                available_ports.iter().any(|port| port == &query)
            };

            if !query_matches {
                self.snarl[node_id]
                    .input_values
                    .insert("Port".to_string(), InputValue::Text(String::new()));

                warnings.push(format!(
                    "'{}' had a missing MIDI port ('{}'); port selection was reset to Auto.",
                    definition.node.name, query
                ));
            }
        }

        warnings
    }
}

impl Default for NodeGraphState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NodeGraphViewer<'a> {
    node_library: Arc<NodeLibrary>,
    pending_errors: Vec<String>,
    input_widget_state: &'a mut input_widgets::InputWidgetState,
    node_menu_search: &'a mut String,
    node_menu_focus_requested: &'a mut bool,
    graph_menu_visible_this_frame: bool,
    initial_graph_view: Option<GraphViewState>,
    initial_graph_view_zoom: Option<f32>,
    apply_initial_graph_view: bool,
    latest_graph_view: Option<GraphViewState>,
    reset_view_requested: bool,
}

#[derive(Clone)]
struct NodeMenuSearchItem {
    definition_name: String,
    display_name: String,
    category: String,
    subcategories: Vec<String>,
    search_text: String,
}

impl FuzzySearchable for NodeMenuSearchItem {
    fn as_search_string(&self) -> &str {
        &self.search_text
    }
}

impl<'a> NodeGraphViewer<'a> {
    pub fn new(
        node_library: Arc<NodeLibrary>,
        input_widget_state: &'a mut input_widgets::InputWidgetState,
        node_menu_search: &'a mut String,
        node_menu_focus_requested: &'a mut bool,
    ) -> Self {
        Self {
            node_library,
            pending_errors: Vec::new(),
            input_widget_state,
            node_menu_search,
            node_menu_focus_requested,
            graph_menu_visible_this_frame: false,
            initial_graph_view: None,
            initial_graph_view_zoom: None,
            apply_initial_graph_view: false,
            latest_graph_view: None,
            reset_view_requested: false,
        }
    }

    pub fn set_initial_graph_view(
        &mut self,
        view: Option<GraphViewState>,
        legacy_zoom: Option<f32>,
        apply_once: bool,
    ) {
        self.initial_graph_view = view;
        self.initial_graph_view_zoom = legacy_zoom;
        self.apply_initial_graph_view = apply_once;
    }

    pub fn latest_graph_view(&self) -> Option<GraphViewState> {
        self.latest_graph_view
    }

    pub fn take_reset_view_requested(&mut self) -> bool {
        std::mem::take(&mut self.reset_view_requested)
    }

    pub fn take_pending_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_errors)
    }

    fn push_error(&mut self, msg: impl Into<String>) {
        self.pending_errors.push(msg.into());
    }

    fn insert_node_from_definition_name(
        &self,
        pos: egui::Pos2,
        definition_name: &str,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeData>,
        node_inserted: &mut bool,
    ) {
        snarl.insert_node(
            pos,
            NodeData {
                definition_name: definition_name.to_string(),
                input_values: HashMap::new(),
                engine_node_id: None,
            },
        );
        ui.close();
        *node_inserted = true;
    }

    /// Simple DFS to check if connecting would create a cycle in the graph
    fn would_create_cycle(snarl: &Snarl<NodeData>, from: SnarlNodeId, to: SnarlNodeId) -> bool {
        let mut stack = vec![to];
        let mut visited = std::collections::HashSet::new();

        while let Some(node) = stack.pop() {
            if !visited.insert(node) {
                continue;
            }
            if node == from {
                return true;
            }

            for (wire_from, wire_to) in snarl.wires() {
                if wire_from.node == node {
                    stack.push(wire_to.node);
                }
            }
        }

        false
    }

    fn midi_source_has_valid_selected_port(node: &NodeData) -> bool {
        let Some(InputValue::Text(selected_port)) = node.input_values.get("Port") else {
            return false;
        };

        let selected_port = selected_port.trim();
        if selected_port.is_empty() || selected_port.eq_ignore_ascii_case("No ports available") {
            return false;
        }

        let Ok(ports) = list_ports() else {
            return false;
        };

        let available_ports: Vec<String> = ports.map(|port| port.port_name().to_string()).collect();
        if available_ports.is_empty() {
            return false;
        }

        if let Ok(port_index) = selected_port.parse::<usize>() {
            return port_index < available_ports.len();
        }

        available_ports.iter().any(|port| port == selected_port)
    }

    fn build_node_search_items(&self) -> Vec<NodeMenuSearchItem> {
        let mut items: Vec<NodeMenuSearchItem> = self
            .node_library
            .definitions()
            .iter()
            .map(|(definition_name, definition)| {
                let mut search_tokens = vec![
                    definition.node.name.clone(),
                    definition.node.short_description.clone(),
                    definition.node.long_description.clone(),
                    definition.node.category.clone(),
                    definition.node.subcategories.join(" "),
                    definition.node.search_keywords.join(" "),
                ];
                search_tokens.retain(|s| !s.trim().is_empty());

                NodeMenuSearchItem {
                    definition_name: definition_name.clone(),
                    display_name: definition.node.name.clone(),
                    category: definition.node.category.clone(),
                    subcategories: definition.node.subcategories.clone(),
                    search_text: search_tokens.join(" "),
                }
            })
            .collect();

        items.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        items
    }

    pub fn graph_menu_visible_this_frame(&self) -> bool {
        self.graph_menu_visible_this_frame
    }
}

impl SnarlViewer<NodeData> for NodeGraphViewer<'_> {
    fn current_transform(&mut self, to_global: &mut TSTransform, _snarl: &mut Snarl<NodeData>) {
        if self.apply_initial_graph_view {
            if let Some(saved_view) = self.initial_graph_view {
                saved_view.apply_to(to_global);
            } else if let Some(saved_zoom) = self.initial_graph_view_zoom {
                to_global.scaling = saved_zoom;
            }
            self.apply_initial_graph_view = false;
        }

        self.latest_graph_view = Some(GraphViewState::from_transform(*to_global));
    }

    fn title(&mut self, node: &NodeData) -> String {
        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            return "Output".to_string();
        }

        self.node_library
            .get_definition(&node.definition_name)
            .map(|def| def.node.name.clone())
            .unwrap_or_else(|| node.definition_name.clone())
    }

    fn inputs(&mut self, node: &NodeData) -> usize {
        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            return 1;
        }

        self.node_library
            .get_definition(&node.definition_name)
            .map(|def| def.node.inputs.len())
            .unwrap_or(0)
    }

    fn outputs(&mut self, node: &NodeData) -> usize {
        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            return 0;
        }

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
        if node_name == VIRTUAL_OUTPUT_SINK_NAME {
            ui.label("Output");
            let frame_color = colors::input_kind_color(&NodeInputKind::Frame);
            return PinInfo::circle()
                .with_fill(frame_color)
                .with_wire_color(frame_color);
        }

        if let Some(def) = self.node_library.get_definition(&node_name)
            && let Some(input_def) = def.node.inputs.get(pin.id.input)
        {
            let show_pin = input_def.show_pin;

            let mut missing_file_error = None;
            ui.label(&input_def.name);

            // If the definition is file check to make sure the file exists
            if let engine::node::NodeInputKind::File { .. } = input_def.kind
                && let Some(InputValue::File(path)) =
                    snarl[pin.id.node].input_values.get(&input_def.name)
                && !std::path::Path::new(path).exists()
            {
                let missing_path = path.clone();
                let input_name = input_def.name.clone();
                // clear the file input
                snarl[pin.id.node].input_values.remove(&input_def.name);
                missing_file_error = Some(format!(
                    "Missing file for '{}' on '{}': {}",
                    input_name,
                    node_name,
                    missing_path.display()
                ));
            }

            // Show input configuration UI if no connection
            if pin.remotes.is_empty() {
                let node_data = &mut snarl[pin.id.node];
                input_widgets::show_input_widget(
                    ui,
                    &mut node_data.input_values,
                    input_def,
                    &node_name,
                    &self.node_library,
                    pin.id.node,
                    self.input_widget_state,
                );
            } else if let Some(remote) = pin.remotes.first() {
                // Show connected value
                let remote_node = &snarl[remote.node];
                ui.label(format!("Connected to {}", remote_node.definition_name));
            }

            let color = colors::input_kind_color(&input_def.kind);

            if let Some(error) = missing_file_error {
                self.push_error(error);
            }

            if !show_pin {
                return PinInfo::circle()
                    .with_fill(egui::Color32::TRANSPARENT)
                    .with_stroke(egui::Stroke::NONE)
                    .with_wire_color(egui::Color32::TRANSPARENT);
            }

            return PinInfo::circle().with_fill(color).with_wire_color(color);
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
        if node_name == VIRTUAL_OUTPUT_SINK_NAME {
            ui.label("output");
            return PinInfo::circle();
        }

        if let Some(def) = self.node_library.get_definition(node_name)
            && let Some(output_def) = def.node.outputs.get(pin.id.output)
        {
            if !output_def.show_pin {
                return PinInfo::circle()
                    .with_fill(egui::Color32::TRANSPARENT)
                    .with_stroke(egui::Stroke::NONE)
                    .with_wire_color(egui::Color32::TRANSPARENT);
            }

            ui.label(&output_def.name);
            let color = colors::output_kind_color(&output_def.kind);
            return PinInfo::circle().with_fill(color).with_wire_color(color);
        }

        ui.label("output");
        PinInfo::circle()
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<NodeData>) -> bool {
        true
    }

}

impl<'a> NodeGraphViewer<'a> {
    pub fn show_graph_menu_with_flag(&mut self, pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<NodeData>, node_inserted: &mut bool) {
        self.graph_menu_visible_this_frame = true;
        let menu_config = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks);

        let stack_info = egui::UiStackInfo::new(egui::UiKind::Menu).with_tag_value(
            egui::containers::menu::MenuConfig::MENU_CONFIG_TAG,
            menu_config,
        );

        ui.scope_builder(egui::UiBuilder::new().ui_stack_info(stack_info), |ui| {
            let palette = util::ui::app_palette();
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(22, 34, 43))
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(340.0); // Fixed menu width
                    ui.label(egui::RichText::new("Add Node").strong());
                    ui.separator();

                    let search_response = ui
                        .horizontal(|ui| {
                            let search = ui.add(
                                egui::TextEdit::singleline(self.node_menu_search)
                                    .hint_text("Search nodes...")
                                    .desired_width(220.0),
                            );

                            let clear = ui.add_enabled(
                                !self.node_menu_search.is_empty(),
                                egui::Button::new("Clear"),
                            );
                            if clear.clicked() {
                                self.node_menu_search.clear();
                            }

                            search
                        })
                        .inner;
                    if !*self.node_menu_focus_requested {
                        search_response.request_focus();
                        *self.node_menu_focus_requested = true;
                    }
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            let query = self.node_menu_search.trim();

                            if !query.is_empty() {
                                let mut searcher = FuzzySearcher::new(query);
                                let results: Vec<NodeMenuSearchItem> = searcher
                                    .search(self.build_node_search_items().into_iter())
                                    .collect();

                                if results.is_empty() {
                                    ui.label(egui::RichText::new("No matching nodes").weak());
                                    return;
                                }

                                for item in results {
                                    if ui.button(&item.display_name).clicked() {
                                        self.insert_node_from_definition_name(
                                            pos,
                                            &item.definition_name,
                                            ui,
                                            snarl,
                                            node_inserted,
                                        );
                                    }

                                    let mut location = item.category;
                                    if !item.subcategories.is_empty() {
                                        location.push_str(" / ");
                                        location.push_str(&item.subcategories.join(" / "));
                                    }

                                    if !location.trim().is_empty() {
                                        ui.label(egui::RichText::new(location).small().weak());
                                    }
                                    ui.add_space(4.0);
                                }
                                return;
                            }

                            let mut categories = self.node_library.get_all_category_info();
                            categories.sort_by(|a, b| a.name.cmp(&b.name));

                            for category in categories {
                                let mut direct_nodes = category.direct_nodes.clone();
                                direct_nodes.sort();

                                let mut subcategories = category.subcategories.clone();
                                subcategories.sort_by(|a, b| a.name.cmp(&b.name));

                                egui::CollapsingHeader::new(&category.name)
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        for node_name in &direct_nodes {
                                            if ui.button(node_name).clicked() {
                                                self.insert_node_from_definition_name(
                                                    pos, node_name, ui, snarl, node_inserted,
                                                );
                                            }
                                        }
                                        for subcategory in &subcategories {
                                            egui::CollapsingHeader::new(&subcategory.name)
                                                .default_open(false)
                                                .show(ui, |ui| {
                                                    let mut sub_nodes = subcategory.nodes.clone();
                                                    sub_nodes.sort();
                                                    for node_name in &sub_nodes {
                                                        if ui.button(node_name).clicked() {
                                                            self.insert_node_from_definition_name(
                                                                pos, node_name, ui, snarl, node_inserted,
                                                            );
                                                        }
                                                    }
                                                });
                                        }
                                    });
                            }
                        });
                });
        });
        // No return value; node_inserted is updated via mutable reference
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
        if snarl[node_id].definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            ui.label("This node is reserved as the graph output.");
            return;
        }

        if ui.button("Delete Node").clicked() {
            snarl.remove_node(node_id);
            ui.close();
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeData>) {
        // Validate connection types
        let from_node = &snarl[from.id.node];
        let to_node = &snarl[to.id.node];

        if from.id.node == to.id.node {
            // Prevent self-connection
            self.push_error("A node cannot be connected to itself.");
            return;
        }

        if Self::would_create_cycle(snarl, from.id.node, to.id.node) {
            self.push_error("Connecting these nodes would create a cycle.");
            return;
        }

        let Some(from_def) = self.node_library.get_definition(&from_node.definition_name) else {
            return;
        };

        let sink_target = to_node.definition_name == VIRTUAL_OUTPUT_SINK_NAME;
        let to_def = if sink_target {
            None
        } else {
            self.node_library.get_definition(&to_node.definition_name)
        };

        if !sink_target && to_def.is_none() {
            return;
        }

        let Some(from_output) = from_def.node.outputs.get(from.id.output) else {
            return;
        };

        let to_input_kind = if sink_target {
            if to.id.input != 0 {
                return;
            }
            NodeInputKind::Frame
        } else {
            let to_def = to_def.expect("checked above");
            let Some(to_input) = to_def.node.inputs.get(to.id.input) else {
                return;
            };
            to_input.kind.clone()
        };

        let to_input_name = if sink_target {
            "Output".to_string()
        } else {
            let to_def = to_def.expect("checked above");
            let Some(to_input) = to_def.node.inputs.get(to.id.input) else {
                return;
            };
            to_input.name.clone()
        };

        // Check if types are compatible
        if !are_pin_kinds_compatible(from_output.kind, &to_input_kind) {
            self.push_error(format!(
                "Cannot connect '{}' to '{}': incompatible pin types.",
                from_output.name, to_input_name
            ));
            return;
        }

        if matches!(
            from_def.node.executor,
            NodeExecutionPlan::BuiltIn(BuiltInHandler::MidiSource)
        ) {
            if !Self::midi_source_has_valid_selected_port(from_node) {
                self.push_error("Cannot connect MIDI node: select a valid MIDI input port first.");
                return;
            }
        }

        // Enforce one incoming connection per input pin.
        // Snarl allows multiple wires per input by default, so we replace
        // existing input wires before connecting the new source.
        snarl.drop_inputs(to.id);

        // Types match - create the connection
        snarl.connect(from.id, to.id);
    }

    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<NodeData>) {
        snarl.drop_inputs(pin.id);
    }

    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<NodeData>) {
        snarl.drop_outputs(pin.id);
    }
}

impl NodeGraphState {
    /// Sync the entire node graph to the engine
    /// Returns true if any changes were made to the engine graph
    pub fn sync_to_engine(
        &mut self,
        engine_graph: &mut NodeGraph,
        node_library: &NodeLibrary,
    ) -> bool {
        sync::sync_to_engine(self, engine_graph, node_library)
    }
}
