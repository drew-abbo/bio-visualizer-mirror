use egui_snarl::InPinId;
use egui_snarl::{NodeId as SnarlNodeId, Snarl};
use engine::node::engine_node::{NodeInput, NodeOutputKind};
use engine::node::{NodeInputKind, NodeLibrary, input_kind_to_output_kind};
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use std::collections::{HashMap, HashSet};

use super::{NodeData, NodeGraphState, validation};
const VIRTUAL_OUTPUT_SINK_NAME: &str = "__virtual_output_sink__";

fn are_pin_kinds_compatible(output_kind: NodeOutputKind, input_kind: &NodeInputKind) -> bool {
    let expected_kind = input_kind_to_output_kind(input_kind);
    output_kind == expected_kind
        // Numeric widening: allow Int outputs to feed Float inputs.
        || matches!((output_kind, input_kind), (NodeOutputKind::Int, NodeInputKind::Float { .. }))
}

/// Sync the entire node graph to the engine
/// Returns true if any changes were made to the engine graph
pub fn sync_to_engine(
    state: &mut NodeGraphState,
    engine_graph: &mut NodeGraph,
    node_library: &NodeLibrary,
) -> bool {
    // Remove wires that reference pins which no longer exist in node definitions
    // (for example after output pin removals in updated node schemas).
    let mut changes_made = prune_invalid_wires(state, node_library);

    // Collect all snarl node IDs (must do this before mutating)
    let all_node_ids: Vec<SnarlNodeId> = state.snarl.node_ids().map(|(id, _)| id).collect();

    // Collect all engine node IDs that are still in the snarl
    let snarl_engine_ids: HashSet<EngineNodeId> = all_node_ids
        .iter()
        .filter_map(|node_id| state.snarl[*node_id].engine_node_id)
        .collect();

    // Remove engine nodes that are no longer in the snarl (were deleted)
    let all_engine_ids: Vec<EngineNodeId> = engine_graph.instances().keys().copied().collect();

    for engine_id in all_engine_ids {
        if !snarl_engine_ids.contains(&engine_id) {
            engine_graph.remove_instance(engine_id);
            changes_made = true;
        }
    }

    // First, identify which nodes should be in the engine
    let mut to_add = Vec::new();
    let mut to_remove = Vec::new();

    for node_id in &all_node_ids {
        let node = &state.snarl[*node_id];
        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            continue;
        }
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

        let inputs_satisfied =
            validation::are_inputs_satisfied(&state.snarl, *node_id, node_library);

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
        remove_node_from_engine(state, node_id, engine_graph);
        changes_made = true;
    }

    // Add newly connected nodes
    for node_id in to_add {
        sync_node_to_engine(state, node_id, engine_graph, node_library);
        changes_made = true;
    }

    // Update input values for nodes already in engine
    // Preserve existing connection inputs so we don't clobber them
    for node_id in &all_node_ids {
        let node = &state.snarl[*node_id];
        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            continue;
        }
        if let Some(engine_id) = node.engine_node_id
            && let Some(instance) = engine_graph.get_instance_mut(engine_id)
        {
            let Some(definition) = node_library.get_definition(&node.definition_name) else {
                continue;
            };

            let connected_inputs = connected_input_names(&state.snarl, *node_id, definition);

            let mut merged_inputs = instance.input_values.clone();

            // Apply defaults for non-connected inputs if missing
            for input_def in &definition.node.inputs {
                if connected_inputs.contains(&input_def.name) {
                    continue;
                }
                if !merged_inputs.contains_key(&input_def.name)
                    && let Some(default_val) = default_input_value(input_def)
                {
                    merged_inputs.insert(input_def.name.clone(), default_val);
                }
            }

            // Apply user-provided values for non-connected inputs
            for (key, value) in &node.input_values {
                if connected_inputs.contains(key) {
                    continue;
                }
                merged_inputs.insert(key.clone(), value.clone());
            }

            // Only update if values actually changed
            if merged_inputs != instance.input_values {
                instance.input_values = merged_inputs;
                changes_made = true;
            }
        }
    }

    // Sync all wires
    for node_id in &all_node_ids {
        if state.snarl[*node_id].engine_node_id.is_some()
            && sync_wires_for_node(state, *node_id, engine_graph, node_library)
        {
            changes_made = true;
        }
    }

    changes_made
}

fn prune_invalid_wires(state: &mut NodeGraphState, node_library: &NodeLibrary) -> bool {
    let mut invalid_inputs: Vec<InPinId> = Vec::new();

    for (wire_from, wire_to) in state.snarl.wires() {
        if state.snarl[wire_from.node].definition_name == VIRTUAL_OUTPUT_SINK_NAME
            || state.snarl[wire_to.node].definition_name == VIRTUAL_OUTPUT_SINK_NAME
        {
            continue;
        }

        let from_node = &state.snarl[wire_from.node];
        let to_node = &state.snarl[wire_to.node];

        let Some(from_def) = node_library.get_definition(&from_node.definition_name) else {
            invalid_inputs.push(wire_to);
            continue;
        };
        let Some(to_def) = node_library.get_definition(&to_node.definition_name) else {
            invalid_inputs.push(wire_to);
            continue;
        };

        let Some(from_output) = from_def.node.outputs.get(wire_from.output) else {
            invalid_inputs.push(wire_to);
            continue;
        };
        let Some(to_input) = to_def.node.inputs.get(wire_to.input) else {
            invalid_inputs.push(wire_to);
            continue;
        };

        if !are_pin_kinds_compatible(from_output.kind, &to_input.kind) {
            invalid_inputs.push(wire_to);
        }
    }

    if invalid_inputs.is_empty() {
        return false;
    }

    invalid_inputs.sort_by_key(|pin| (pin.node.0, pin.input));
    invalid_inputs.dedup();

    for input_pin in invalid_inputs {
        state.snarl.drop_inputs(input_pin);
    }

    true
}

/// Sync a node to the engine if it should be active
pub fn sync_node_to_engine(
    state: &mut NodeGraphState,
    node_id: SnarlNodeId,
    engine_graph: &mut NodeGraph,
    node_library: &NodeLibrary,
) {
    // Check if already in engine
    if state.snarl[node_id].engine_node_id.is_some() {
        return;
    }

    // Check conditions before borrowing mutably
    let definition_name = state.snarl[node_id].definition_name.clone();
    let input_values = state.snarl[node_id].input_values.clone();

    let Some(definition) = node_library.get_definition(&definition_name) else {
        return;
    };

    // Use the same admission rule as the full-graph sync path so scalar
    // generators like noise are not incorrectly excluded.
    let should_add = validation::are_inputs_satisfied(&state.snarl, node_id, node_library);

    if !should_add {
        return;
    }

    // Add to engine
    let engine_id = engine_graph.add_instance(definition_name);

    // Copy input values to engine instance (with defaults for non-connected inputs)
    if let Some(instance) = engine_graph.get_instance_mut(engine_id) {
        let connected_inputs = connected_input_names(&state.snarl, node_id, definition);
        let mut merged_inputs = HashMap::new();

        for input_def in &definition.node.inputs {
            if connected_inputs.contains(&input_def.name) {
                continue;
            }
            if let Some(default_val) = default_input_value(input_def) {
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
    state.snarl[node_id].engine_node_id = Some(engine_id);

    // Sync all wire connections for this node
    sync_wires_for_node(state, node_id, engine_graph, node_library);
}

/// Remove a node from the engine
pub fn remove_node_from_engine(
    state: &mut NodeGraphState,
    node_id: SnarlNodeId,
    engine_graph: &mut NodeGraph,
) {
    let node = &mut state.snarl[node_id];
    if let Some(engine_id) = node.engine_node_id.take() {
        engine_graph.remove_instance(engine_id);
    }
}

/// Sync wire connections for a specific node
fn sync_wires_for_node(
    state: &mut NodeGraphState,
    node_id: SnarlNodeId,
    engine_graph: &mut NodeGraph,
    node_library: &NodeLibrary,
) -> bool {
    let mut changes_made = false;

    let node = &state.snarl[node_id];
    if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
        return false;
    }
    let Some(to_engine_id) = node.engine_node_id else {
        return false;
    };

    // Get node definition to map input indices to names
    let Some(definition) = node_library.get_definition(&node.definition_name) else {
        return false;
    };

    // Build desired incoming connections from snarl wires:
    // input_name -> (from_engine_id, output_name)
    let mut desired_connections: HashMap<String, (EngineNodeId, String)> = HashMap::new();
    for (wire_from, wire_to) in state.snarl.wires() {
        if wire_to.node != node_id {
            continue;
        }

        if state.snarl[wire_from.node].definition_name == VIRTUAL_OUTPUT_SINK_NAME
            || state.snarl[wire_to.node].definition_name == VIRTUAL_OUTPUT_SINK_NAME
        {
            continue;
        }

        let from_node = &state.snarl[wire_from.node];
        let Some(from_engine_id) = from_node.engine_node_id else {
            continue;
        };

        let Some(from_definition) = node_library.get_definition(&from_node.definition_name) else {
            continue;
        };
        let Some(output_def) = from_definition.node.outputs.get(wire_from.output) else {
            continue;
        };
        let Some(input_def) = definition.node.inputs.get(wire_to.input) else {
            continue;
        };

        desired_connections.insert(
            input_def.name.clone(),
            (from_engine_id, output_def.name.clone()),
        );
    }

    // Snapshot current engine connections for this node:
    // input_name -> (from_node, output_name)
    let current_connections: HashMap<String, (EngineNodeId, String)> = engine_graph
        .get_instance(to_engine_id)
        .map(|instance| {
            instance
                .input_values
                .iter()
                .filter_map(|(name, value)| {
                    if let InputValue::Connection {
                        from_node,
                        output_name,
                    } = value
                    {
                        Some((name.clone(), (*from_node, output_name.clone())))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Disconnect stale or changed connections only.
    for input_def in &definition.node.inputs {
        let input_name = &input_def.name;
        let current = current_connections.get(input_name);
        let desired = desired_connections.get(input_name);

        let needs_disconnect = match (current, desired) {
            (Some(curr), Some(want)) => curr != want,
            (Some(_), None) => true,
            _ => false,
        };

        if needs_disconnect && engine_graph.disconnect(to_engine_id, input_name) {
            changes_made = true;
        }
    }

    // Connect new or changed connections only.
    for (input_name, (from_engine_id, output_name)) in desired_connections {
        let already_connected = current_connections
            .get(&input_name)
            .is_some_and(|curr| curr.0 == from_engine_id && curr.1 == output_name);

        if already_connected {
            continue;
        }

        match engine_graph.connect(
            from_engine_id,
            output_name.clone(),
            to_engine_id,
            input_name.clone(),
        ) {
            Ok(_) => {
                changes_made = true;
            }
            Err(err) => {
                util::debug_log_error!(
                    "Failed to connect {} output '{}' to {} input '{}': {}",
                    from_engine_id,
                    output_name,
                    to_engine_id,
                    input_name,
                    err
                );
            }
        }
    }

    changes_made
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
        NodeInputKind::Enum { default_idx, .. } => Some(InputValue::Enum(default_idx.unwrap_or(0))),
        NodeInputKind::Text { default, .. } => Some(InputValue::Text(default.clone())),
        NodeInputKind::File { default, .. } => default.clone().map(InputValue::File),
        NodeInputKind::Frame | NodeInputKind::MidiPacket => None,
        NodeInputKind::PortSelection => Some(InputValue::Text(String::new())),
    }
}

/// Normalize all node inputs to match their current schema definitions.
/// This ensures that when a project is loaded, any missing inputs (from schema changes) are
/// populated with defaults, and any orphaned inputs (no longer in schema) are removed.
pub fn normalize_node_inputs(state: &mut NodeGraphState, node_library: &NodeLibrary) {
    let all_node_ids: Vec<SnarlNodeId> = state.snarl.node_ids().map(|(id, _)| id).collect();

    for node_id in all_node_ids {
        let node = &state.snarl[node_id];

        // Skip the virtual output sink
        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            continue;
        }

        let Some(definition) = node_library.get_definition(&node.definition_name) else {
            continue;
        };

        // Get the list of connected inputs for this node
        let connected_inputs = connected_input_names(&state.snarl, node_id, definition);

        // Ensure all inputs from the definition are present in input_values
        for input_def in &definition.node.inputs {
            // Skip connected inputs (those are managed by wires)
            if connected_inputs.contains(&input_def.name) {
                continue;
            }

            // Skip inputs that are already set
            if state.snarl[node_id].input_values.contains_key(&input_def.name) {
                continue;
            }

            // Add default value if available
            if let Some(default_val) = default_input_value(input_def) {
                state.snarl[node_id]
                    .input_values
                    .insert(input_def.name.clone(), default_val);
            }
        }

        // Remove inputs that are no longer in the schema
        let defined_input_names: std::collections::HashSet<_> =
            definition.node.inputs.iter().map(|i| i.name.clone()).collect();

        let orphaned_inputs: Vec<_> = state.snarl[node_id]
            .input_values
            .keys()
            .filter(|key| !defined_input_names.contains(*key))
            .cloned()
            .collect();

        for orphaned_key in orphaned_inputs {
            state.snarl[node_id].input_values.remove(&orphaned_key);
        }
    }
}
