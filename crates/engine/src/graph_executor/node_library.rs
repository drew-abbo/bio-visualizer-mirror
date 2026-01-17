use crate::graph_executor::enums::{NodeExecutionPlan, NodeInputKind, NodeOutputKind};
use std::collections::HashMap;

// Placeholder types (these come from the node definition system)
pub struct NodeLibrary {
    definitions: HashMap<String, Node>,
}

impl NodeLibrary {
    pub fn get_definition(&self, name: &str) -> Option<&Node> {
        self.definitions.get(name)
    }
}

pub struct Node {
    pub name: String,
    pub inputs: Vec<NodeInput>,
    pub outputs: Vec<NodeOutput>,
    pub executor: NodeExecutionPlan,
}

pub struct NodeInput {
    pub name: String,
    pub kind: NodeInputKind,
}

pub struct NodeOutput {
    pub name: String,
    pub kind: NodeOutputKind,
}
