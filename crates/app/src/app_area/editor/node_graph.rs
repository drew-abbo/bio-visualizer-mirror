mod colors;
mod input_widgets;
mod sync;
mod validation;

use egui_snarl::ui::{PinInfo, SnarlViewer};
use egui_snarl::{InPin, NodeId as SnarlNodeId, OutPin, Snarl};
use engine::node::{NodeLibrary, input_kind_to_output_kind};
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use std::collections::HashMap;
use std::sync::Arc;
use util::egui;

/// Data associated with each node in the snarl graph, including its definition and configured input values
#[derive(Clone, Debug)]
pub struct NodeData {
    pub definition_name: String,
    /// Configured input values for this node (not including connected inputs)
    pub input_values: HashMap<String, InputValue>,
    /// Engine node ID if this node is currently in the engine graph
    pub engine_node_id: Option<EngineNodeId>,
}

/// The state of the node graph editor, including the snarl graph and its synchronization with the engine graph
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
        if let Some(def) = self.node_library.get_definition(&node_name)
            && let Some(input_def) = def.node.inputs.get(pin.id.input)
        {
            ui.label(&input_def.name);

            // Show input configuration UI if no connection
            if pin.remotes.is_empty() {
                let node_data = &mut snarl[pin.id.node];
                input_widgets::show_input_widget(
                    ui,
                    &mut node_data.input_values,
                    input_def,
                    &node_name,
                    &self.node_library,
                );
            } else if let Some(remote) = pin.remotes.first() {
                // Show connected value
                let remote_node = &snarl[remote.node];
                ui.label(format!("Connected to {}", remote_node.definition_name));
            }

            let color = colors::input_kind_color(&input_def.kind);
            return PinInfo::circle().with_fill(color);
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
        if let Some(def) = self.node_library.get_definition(node_name)
            && let Some(output_def) = def.node.outputs.get(pin.id.output)
        {
            ui.label(&output_def.name);
            let color = colors::output_kind_color(&output_def.kind);
            return PinInfo::circle().with_fill(color);
        }

>>>>>>> a665ac9 (commit now so I don't screw something up)
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

impl NodeGraphState {
    /// Sync the entire node graph to the engine
    pub fn sync_to_engine(&mut self, engine_graph: &mut NodeGraph, node_library: &NodeLibrary) {
        sync::sync_to_engine(self, engine_graph, node_library);
    }
}
