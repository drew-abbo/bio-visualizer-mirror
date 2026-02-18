use egui_snarl::{NodeId as SnarlNodeId, Snarl};
<<<<<<< HEAD
<<<<<<< HEAD
use engine::node::engine_node::NodeInput;
=======
use engine::node::node::NodeInput;
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
use engine::node::engine_node::NodeInput;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
use engine::node::{NodeInputKind, NodeLibrary};
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use std::collections::{HashMap, HashSet};

use super::{NodeData, NodeGraphState, validation};

/// Sync the entire node graph to the engine
/// Returns true if any changes were made to the engine graph
pub fn sync_to_engine(
    state: &mut NodeGraphState,
    engine_graph: &mut NodeGraph,
    node_library: &NodeLibrary,
<<<<<<< HEAD
<<<<<<< HEAD
) -> bool {
    let mut changes_made = false;

    // Collect all snarl node IDs (must do this before mutating)
    let all_node_ids: Vec<SnarlNodeId> = state.snarl.node_ids().map(|(id, _)| id).collect();
=======
) {
=======
) -> bool {
    let mut changes_made = false;

>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
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
<<<<<<< HEAD
<<<<<<< HEAD
            changes_made = true;
=======
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
            changes_made = true;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
        }
    }

    // First, identify which nodes should be in the engine
    let mut to_add = Vec::new();
    let mut to_remove = Vec::new();

    for node_id in &all_node_ids {
        let node = &state.snarl[*node_id];
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
<<<<<<< HEAD
<<<<<<< HEAD
        changes_made = true;
=======
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
        changes_made = true;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
    }

    // Add newly connected nodes
    for node_id in to_add {
        sync_node_to_engine(state, node_id, engine_graph, node_library);
<<<<<<< HEAD
<<<<<<< HEAD
        changes_made = true;
=======
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
        changes_made = true;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
    }

    // Update input values for nodes already in engine
    // Preserve existing connection inputs so we don't clobber them
    for node_id in &all_node_ids {
        let node = &state.snarl[*node_id];
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

<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
            // Only update if values actually changed
            if merged_inputs != instance.input_values {
                instance.input_values = merged_inputs;
                changes_made = true;
            }
<<<<<<< HEAD
=======
            instance.input_values = merged_inputs;
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
        }
    }

    // Sync all wires
    for node_id in &all_node_ids {
<<<<<<< HEAD
        if state.snarl[*node_id].engine_node_id.is_some()
            && sync_wires_for_node(state, *node_id, engine_graph, node_library) {
                changes_made = true;
            }
    }

    changes_made
=======
        if state.snarl[*node_id].engine_node_id.is_some() {
            if sync_wires_for_node(state, *node_id, engine_graph, node_library) {
                changes_made = true;
            }
        }
    }
<<<<<<< HEAD
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======

    changes_made
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
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
        validation::is_connected_to_source(&state.snarl, node_id, node_library)
    };

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
<<<<<<< HEAD
<<<<<<< HEAD
) -> bool {
    let mut changes_made = false;

    let node = &state.snarl[node_id];
    let Some(to_engine_id) = node.engine_node_id else {
        return false;
=======
) {
    let node = &state.snarl[node_id];
    let Some(to_engine_id) = node.engine_node_id else {
        return;
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
) -> bool {
    let mut changes_made = false;

    let node = &state.snarl[node_id];
    let Some(to_engine_id) = node.engine_node_id else {
        return false;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
    };

    // Get node definition to map input indices to names
    let Some(definition) = node_library.get_definition(&node.definition_name) else {
<<<<<<< HEAD
<<<<<<< HEAD
        return false;
=======
        return;
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
        return false;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
    };

    // Clear existing engine connections for this node's inputs
    for input_def in &definition.node.inputs {
<<<<<<< HEAD
<<<<<<< HEAD
        if engine_graph.disconnect(to_engine_id, &input_def.name) {
            changes_made = true;
        }
=======
        engine_graph.disconnect(to_engine_id, &input_def.name);
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
        if engine_graph.disconnect(to_engine_id, &input_def.name) {
            changes_made = true;
        }
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
    }

    // Connect all inputs
    for (wire_from, wire_to) in state.snarl.wires() {
        if wire_to.node != node_id {
            continue;
        }

        let from_node = &state.snarl[wire_from.node];
        let Some(from_engine_id) = from_node.engine_node_id else {
            continue;
        };

        // Get output and input names
        let Some(from_definition) = node_library.get_definition(&from_node.definition_name) else {
            continue;
        };

        let Some(output_def) = from_definition.node.outputs.get(wire_from.output) else {
            continue;
        };
        let Some(input_def) = definition.node.inputs.get(wire_to.input) else {
            continue;
        };

        // Connect in engine graph
<<<<<<< HEAD
<<<<<<< HEAD
        match engine_graph.connect(
=======
        if let Err(err) = engine_graph.connect(
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
        match engine_graph.connect(
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
            from_engine_id,
            output_def.name.clone(),
            to_engine_id,
            input_def.name.clone(),
        ) {
<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
            Ok(_) => {
                changes_made = true;
            }
            Err(err) => {
                util::debug_log_error!(
                    "Failed to connect {} output '{}' to {} input '{}': {}",
                    from_engine_id,
                    output_def.name,
                    to_engine_id,
                    input_def.name,
                    err
                );
            }
<<<<<<< HEAD
        }
    }

    changes_made
=======
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
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
=======
        }
    }

    changes_made
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
}

fn connected_input_names(
    snarl: &Snarl<NodeData>,
    node_id: SnarlNodeId,
    definition: &engine::node::NodeDefinition,
) -> HashSet<String> {
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
        NodeInputKind::Frame | NodeInputKind::Midi => None,
    }
}
