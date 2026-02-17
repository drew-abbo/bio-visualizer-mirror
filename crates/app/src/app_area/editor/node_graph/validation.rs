use egui_snarl::{NodeId as SnarlNodeId, Snarl};
use engine::node::NodeLibrary;
use engine::node::NodeInputKind;
use engine::node_graph::InputValue;
use std::collections::HashSet;

use super::NodeData;

/// Check if a node has all its required inputs satisfied (connected or with defaults)
/// This is RECURSIVE - it checks that source nodes are also satisfied
pub fn are_inputs_satisfied(
    snarl: &Snarl<NodeData>,
    node_id: SnarlNodeId,
    node_library: &NodeLibrary,
) -> bool {
    let node = &snarl[node_id];
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
    let connected_inputs: HashSet<String> = snarl
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
    for (wire_from, wire_to) in snarl.wires() {
        if wire_to.node == node_id {
            let source_node = wire_from.node;
            // Recursively check that the source node is also satisfied
            if !are_inputs_satisfied(snarl, source_node, node_library) {
                return false;
            }
        }
    }

    true
}

/// Check if a node is a source node (video/image) with a configured file
pub fn is_active_source(
    snarl: &Snarl<NodeData>,
    node_id: SnarlNodeId,
    node_library: &NodeLibrary,
) -> bool {
    let node = &snarl[node_id];
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
pub fn is_connected_to_source(
    snarl: &Snarl<NodeData>,
    node_id: SnarlNodeId,
    node_library: &NodeLibrary,
) -> bool {
    // If it's an active source itself, return true
    if is_active_source(snarl, node_id, node_library) {
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
        for (wire_from, wire_to) in snarl.wires() {
            if wire_to.node == current_id {
                // This node receives input from wire_from.node
                let source_node = wire_from.node;
                if is_active_source(snarl, source_node, node_library) {
                    return true;
                }
                queue.push(source_node);
            }
        }
    }

    false
}
