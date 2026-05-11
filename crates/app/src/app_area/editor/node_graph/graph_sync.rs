use egui_snarl::NodeId as SnarlNodeId;
use engine::node::{NodeInputKind, NodeLibrary};
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use std::collections::{HashMap, HashSet};

use super::NodeGraphState;

pub const VIRTUAL_OUTPUT_SINK_NAME: &str = "__virtual_output_sink__";

/// The result of attempting to sync the editor graph to the engine.
pub enum GraphSyncResult {
    /// Graph is valid and was sent to the engine.
    Valid {
        graph: NodeGraph,
        output_node: EngineNodeId,
        snarl_to_engine: HashMap<SnarlNodeId, EngineNodeId>,
    },
    /// No output sink is wired engine should pause.
    NoOutput,
    /// Graph has validation errors engine should pause.
    /// The errors are human-readable strings suitable for display.
    Invalid(Vec<String>),
}

/// Walk the graph upstream from the output sink, validate every node,
/// and build the engine graph in a single pass.
///
/// This is the only function that should touch graph->engine translation.
/// Call it whenever the editor graph changes, then act on the result.
pub fn sync_graph(state: &NodeGraphState, library: &NodeLibrary) -> GraphSyncResult {
    let Some(output_source_snarl_id) = state.output_source_snarl_node() else {
        return GraphSyncResult::NoOutput;
    };

    // Collect all snarl nodes reachable upstream from the output source,
    // validating each as we go. We do a single BFS — visited set prevents
    // redundant work on diamond-shaped graphs.
    let mut visited: HashSet<SnarlNodeId> = HashSet::new();
    let mut errors: Vec<String> = Vec::new();
    let mut ordered: Vec<SnarlNodeId> = Vec::new(); // BFS order, upstream-first
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(output_source_snarl_id);

    // Build a reverse-adjacency map once so we don't re-scan wires per node.
    // Maps: to_node -> [(from_node, from_output_index, to_input_index)]
    let mut upstream: HashMap<SnarlNodeId, Vec<(SnarlNodeId, usize, usize)>> = HashMap::new();
    for (wire_from, wire_to) in state.snarl.wires() {
        upstream.entry(wire_to.node).or_default().push((
            wire_from.node,
            wire_from.output,
            wire_to.input,
        ));
    }

    while let Some(snarl_id) = queue.pop_front() {
        if !visited.insert(snarl_id) {
            continue;
        }

        let node = &state.snarl[snarl_id];
        if node.definition_name == VIRTUAL_OUTPUT_SINK_NAME {
            continue;
        }

        ordered.push(snarl_id);

        let Some(definition) = library.get_definition(&node.definition_name) else {
            errors.push(format!("Unknown node '{}'.", node.definition_name));
            continue;
        };

        // Collect which input indices are satisfied by wires for this node.
        let wired_inputs: HashSet<usize> = upstream
            .get(&snarl_id)
            .map(|ups| ups.iter().map(|(_, _, to_idx)| *to_idx).collect())
            .unwrap_or_default();

        // Validate each input.
        for (idx, input_def) in definition.node.inputs.iter().enumerate() {
            let is_wired = wired_inputs.contains(&idx);
            let has_value = node.input_values.contains_key(&input_def.name);

            let satisfied = is_wired || has_value || has_default(&input_def.kind);

            if !satisfied {
                errors.push(format!(
                    "'{}' is missing required input '{}'.",
                    definition.node.name, input_def.name
                ));
            }

            // Enqueue upstream nodes for wired frame/midi inputs.
            if is_wired && let Some(ups) = upstream.get(&snarl_id) {
                for &(from_id, _, to_idx) in ups {
                    if to_idx == idx {
                        queue.push_back(from_id);
                    }
                }
            }
        }
    }

    if !errors.is_empty() {
        return GraphSyncResult::Invalid(errors);
    }

    // Build the engine graph from the validated, collected nodes.
    let mut engine_graph = NodeGraph::new();
    let mut snarl_to_engine: HashMap<SnarlNodeId, EngineNodeId> = HashMap::new();

    for &snarl_id in &ordered {
        let node = &state.snarl[snarl_id];
        let engine_id = node
            .engine_node_id
            .map(|id| engine_graph.add_instance_with_id(id, node.definition_name.clone()))
            .unwrap_or_else(|| engine_graph.add_instance(node.definition_name.clone()));
        snarl_to_engine.insert(snarl_id, engine_id);

        let Some(definition) = library.get_definition(&node.definition_name) else {
            continue;
        };

        for input_def in &definition.node.inputs {
            if let Some(value) = node.input_values.get(&input_def.name) {
                if !matches!(value, InputValue::Connection { .. }) {
                    let _ = engine_graph.set_input_value(
                        engine_id,
                        input_def.name.clone(),
                        value.clone(),
                    );
                }
            } else if let Some(default) = super::validation::default_input_value(input_def) {
                let _ = engine_graph.set_input_value(engine_id, input_def.name.clone(), default);
            }
        }
    }

    // Wire connections.
    for (wire_from, wire_to) in state.snarl.wires() {
        let from_node = &state.snarl[wire_from.node];
        let to_node = &state.snarl[wire_to.node];

        if from_node.definition_name == VIRTUAL_OUTPUT_SINK_NAME
            || to_node.definition_name == VIRTUAL_OUTPUT_SINK_NAME
        {
            continue;
        }

        let (Some(from_engine), Some(to_engine)) = (
            snarl_to_engine.get(&wire_from.node).copied(),
            snarl_to_engine.get(&wire_to.node).copied(),
        ) else {
            continue;
        };

        let Some(from_def) = library.get_definition(&from_node.definition_name) else {
            continue;
        };
        let Some(to_def) = library.get_definition(&to_node.definition_name) else {
            continue;
        };
        let Some(output_def) = from_def.node.outputs.get(wire_from.output) else {
            continue;
        };
        let Some(input_def) = to_def.node.inputs.get(wire_to.input) else {
            continue;
        };

        let _ = engine_graph.connect(
            from_engine,
            output_def.name.clone(),
            to_engine,
            input_def.name.clone(),
        );
    }

    let Some(&output_engine_id) = snarl_to_engine.get(&output_source_snarl_id) else {
        return GraphSyncResult::NoOutput;
    };

    GraphSyncResult::Valid {
        graph: engine_graph,
        output_node: output_engine_id,
        snarl_to_engine,
    }
}

fn has_default(kind: &NodeInputKind) -> bool {
    !matches!(kind, NodeInputKind::Frame | NodeInputKind::MidiPacket)
}
