use super::{NodeData, NodeGraphState, VIRTUAL_OUTPUT_SINK_NAME};
use egui_snarl::{NodeId as SnarlNodeId, Snarl};
use engine::node::engine_node::{BuiltInHandler, NodeExecutionPlan};
use engine::node::{NodeInput, NodeInputKind, NodeLibrary};
use engine::node_graph::InputValue;
use media::midi::streams::list_ports;
use std::collections::HashSet;

/// Normalize all node inputs to match their current schema definitions.
/// Call on project load to populate missing inputs (schema additions) with
/// defaults and drop orphaned inputs (schema removals).
pub fn normalize_node_inputs(state: &mut NodeGraphState, node_library: &NodeLibrary) {
    let all_node_ids: Vec<SnarlNodeId> = state.snarl.node_ids().map(|(id, _)| id).collect();

    for node_id in all_node_ids {
        let node = &state.snarl[node_id];

        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            continue;
        }

        let Some(definition) = node_library.get_definition(&node.definition_name) else {
            continue;
        };

        let connected_inputs = connected_input_names(&state.snarl, node_id, definition);

        for input_def in &definition.node.inputs {
            if connected_inputs.contains(&input_def.name) {
                continue;
            }

            if state.snarl[node_id]
                .input_values
                .contains_key(&input_def.name)
            {
                continue;
            }

            if let Some(default_val) = default_input_value(input_def) {
                state.snarl[node_id]
                    .input_values
                    .insert(input_def.name.clone(), default_val);
            }
        }

        let defined_input_names: HashSet<_> = definition
            .node
            .inputs
            .iter()
            .map(|i| i.name.clone())
            .collect();

        let orphaned: Vec<_> = state.snarl[node_id]
            .input_values
            .keys()
            .filter(|key| !defined_input_names.contains(*key))
            .cloned()
            .collect();

        for key in orphaned {
            state.snarl[node_id].input_values.remove(&key);
        }
    }
}

/// Validate the node graph upstream of the selected output source.
/// Only checks whether the node being wired to the output sink can execute
/// when the engine walks backward through connected inputs.
pub fn validate_output_source(
    snarl: &Snarl<NodeData>,
    node_id: SnarlNodeId,
    node_library: &NodeLibrary,
) -> Result<(), String> {
    let node = &snarl[node_id];
    let Some(definition) = node_library.get_definition(&node.definition_name) else {
        return Err(format!(
            "Unknown node definition '{}'.",
            node.definition_name
        ));
    };

    if definition
        .node
        .inputs
        .iter()
        .any(|input| matches!(input.kind, NodeInputKind::File { .. }))
        && !node
            .input_values
            .values()
            .any(|value| matches!(value, InputValue::File(_)))
    {
        return Err(format!("'{}' requires a file input.", definition.node.name));
    }

    for input_def in &definition.node.inputs {
        if !matches!(input_def.kind, NodeInputKind::Frame) {
            continue;
        }

        let connected = snarl.wires().any(|(_, wire_to)| {
            wire_to.node == node_id
                && wire_to.input < definition.node.inputs.len()
                && definition.node.inputs[wire_to.input].name == input_def.name
        });

        if !connected {
            return Err(format!(
                "'{}' is missing required frame input '{}'.",
                definition.node.name, input_def.name
            ));
        }
    }

    for (wire_from, wire_to) in snarl.wires() {
        if wire_to.node != node_id {
            continue;
        }
        validate_output_source(snarl, wire_from.node, node_library)?;
    }

    Ok(())
}

/// Check whether each MIDI source node's configured port is still available.
/// For any node whose port has disappeared, this disconnects its outgoing wires
/// and resets the port selection to empty so the engine stops trying to use it.
///
/// Call on project load and whenever the graph topology changes.
pub fn validate_midi_ports(state: &mut NodeGraphState, node_library: &NodeLibrary) -> Vec<String> {
    let available_ports: Vec<String> = match list_ports() {
        Ok(ports) => ports.map(|port| port.port_name().to_string()).collect(),
        Err(err) => {
            util::debug_log_warning!("Failed to list MIDI ports during validation: {}", err);
            Vec::new()
        }
    };

    let mut warnings = Vec::new();
    let node_ids: Vec<SnarlNodeId> = state.snarl.node_ids().map(|(id, _)| id).collect();

    for node_id in node_ids {
        let definition_name = state.snarl[node_id].definition_name.clone();
        let Some(definition) = node_library.get_definition(&definition_name) else {
            continue;
        };

        if !matches!(
            definition.node.executor,
            NodeExecutionPlan::BuiltIn(BuiltInHandler::MidiSource)
        ) {
            continue;
        }

        // Only act on nodes with a specific port selected — "auto"/empty is fine.
        let port_query =
            state.snarl[node_id]
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

        let Some(query) = port_query else {
            continue;
        };

        let port_available = !available_ports.is_empty()
            && if let Ok(index) = query.parse::<usize>() {
                index < available_ports.len()
            } else {
                available_ports.iter().any(|port| port == &query)
            };

        if port_available {
            continue;
        }

        // Port gone — drop outgoing wires and reset selection.
        let outgoing: Vec<egui_snarl::InPinId> = state
            .snarl
            .wires()
            .filter_map(|(from, to)| (from.node == node_id).then_some(to))
            .collect();

        if !outgoing.is_empty() {
            for pin in outgoing {
                state.snarl.drop_inputs(pin);
            }
            state.snarl[node_id]
                .input_values
                .insert("Port".to_string(), InputValue::Text(String::new()));

            warnings.push(format!(
                "'{}' was disconnected because MIDI port '{}' is no longer available.",
                definition.node.name, query
            ));
        }
    }

    warnings
}

fn connected_input_names(
    snarl: &Snarl<NodeData>,
    node_id: SnarlNodeId,
    definition: &engine::node::NodeDefinition,
) -> HashSet<String> {
    if snarl[node_id].definition_name == VIRTUAL_OUTPUT_SINK_NAME {
        return HashSet::new();
    }

    snarl
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

pub(super) fn default_input_value(input_def: &NodeInput) -> Option<InputValue> {
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
        NodeInputKind::Enum { default_idx, .. } => Some(InputValue::Enum(default_idx.unwrap_or(0))),
        NodeInputKind::Text { default, .. } => Some(InputValue::Text(default.clone())),
        NodeInputKind::File { default, .. } => default.clone().map(InputValue::File),
        NodeInputKind::Frame | NodeInputKind::MidiPacket => None,
        NodeInputKind::PortSelection => Some(InputValue::Text(String::new())),
    }
}
