use egui_snarl::{NodeId as SnarlNodeId, Snarl};
use engine::node::NodeInputKind;
use engine::node::NodeLibrary;
use engine::node::engine_node::{BuiltInHandler, NodeExecutionPlan};
use engine::node_graph::InputValue;
use media::midi::streams::list_ports;
use std::collections::HashSet;

use super::NodeData;

const VIRTUAL_OUTPUT_SINK_NAME: &str = "__virtual_output_sink__";

/// Check if a node has all its required inputs satisfied (connected or with defaults)
/// This is RECURSIVE - it checks that source nodes are also satisfied
pub fn are_inputs_satisfied(
    snarl: &Snarl<NodeData>,
    node_id: SnarlNodeId,
    node_library: &NodeLibrary,
) -> bool {
    let node = &snarl[node_id];
    if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
        return snarl
            .wires()
            .any(|(_, wire_to)| wire_to.node == node_id && wire_to.input == 0);
    }

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

    // Check if this is a MIDI source node - only allow if MIDI ports are available
    if matches!(
        definition.node.executor,
        NodeExecutionPlan::BuiltIn(BuiltInHandler::MidiSource)
    ) {
        let has_midi_ports = match list_ports() {
            Ok(ports) => ports.count() > 0,
            Err(_) => false,
        };
        if !has_midi_ports {
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
