use egui_node_graph2::{DataTypeTrait, Graph, NodeId, NodeTemplateTrait, InputParamKind};
use engine::graph_executor::Value;
use engine::node::{NodeDefinition, NodeInputKind, input_kind_to_output_kind, default_value_for_input_kind};
use engine::node::node::NodeOutputKind;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct NodeTemplate {
    pub definition_name: String,
}

#[derive(Clone, Debug)]
pub struct NodeData {
    pub definition_name: String,
}

#[derive(Default)]
// #[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeGraphState {
    // Keep track of the currently active node
    pub active_node: Option<NodeId>,
    // Node definitions for building nodes
    pub node_definitions: HashMap<String, NodeDefinition>,
}

impl DataTypeTrait<NodeGraphState> for NodeOutputKind {
    fn data_type_color(&self, _user_state: &mut NodeGraphState) -> ecolor::Color32 {
        match self {
            NodeOutputKind::Bool => ecolor::Color32::from_rgb(200, 100, 100),
            NodeOutputKind::Int => ecolor::Color32::from_rgb(100, 200, 100),
            NodeOutputKind::Float => ecolor::Color32::from_rgb(100, 100, 200),
            NodeOutputKind::Frame => ecolor::Color32::from_rgb(200, 200, 100),
            NodeOutputKind::Midi => ecolor::Color32::from_rgb(100, 200, 200),
            NodeOutputKind::Dimensions => ecolor::Color32::from_rgb(200, 100, 200),
            NodeOutputKind::Pixel => ecolor::Color32::from_rgb(150, 150, 150),
            NodeOutputKind::Text => ecolor::Color32::from_rgb(255, 165, 0),
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            NodeOutputKind::Bool => Cow::Borrowed("Bool"),
            NodeOutputKind::Int => Cow::Borrowed("Int"),
            NodeOutputKind::Float => Cow::Borrowed("Float"),
            NodeOutputKind::Frame => Cow::Borrowed("Frame"),
            NodeOutputKind::Midi => Cow::Borrowed("Midi"),
            NodeOutputKind::Dimensions => Cow::Borrowed("Dimensions"),
            NodeOutputKind::Pixel => Cow::Borrowed("Pixel"),
            NodeOutputKind::Text => Cow::Borrowed("Text"),
        }
    }
}

// A trait for the node kinds, which tells the library how to build new nodes
// from the templates in the node finder
impl NodeTemplateTrait for NodeTemplate {
    // Define associated types
    type NodeData = NodeData; // Data stored with each node
    type DataType = NodeOutputKind; // Your data types for connections
    type ValueType = Value; // Values for input parameters
    type UserState = NodeGraphState; // Your user state
    type CategoryType = String; // Categories for node finder

    fn node_finder_label(&self, _user_state: &mut Self::UserState) -> Cow<'_, str> {
        Cow::Owned(self.definition_name.clone())
    }

    fn node_finder_categories(&self, user_state: &mut Self::UserState) -> Vec<Self::CategoryType> {
        // Use sub_folders from node definition as categories
        if let Some(def) = user_state.node_definitions.get(&self.definition_name) {
            if !def.node.sub_folders.is_empty() {
                return def.node.sub_folders.clone();
            }
        }

        // Fallback if no sub_folders are defined
        vec!["Other".to_string()]
    }

    fn node_graph_label(&self, user_state: &mut Self::UserState) -> String {
        // It's okay to delegate this to node_finder_label if you don't want to
        // show different names in the node finder and the node itself.
        self.node_finder_label(user_state).into()
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
        if let Some(def) = user_state.node_definitions.get(&self.definition_name) {
            // Add inputs
            for input in &def.node.inputs {
                // Map NodeInputKind to NodeOutputKind for connection type
                let data_type = input_kind_to_output_kind(&input.kind);
                
                // Create a default value for this input
                let value_type = default_value_for_input_kind(&input.kind);
                
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
                graph.add_output_param(node_id, output.name.clone(), output.kind.clone());
            }
        }
    }
}
