use std::collections::HashMap;

use crate::graph_executor::NodeValue;
use crate::node_graph::EngineNodeId;

#[derive(Debug, thiserror::Error)]
pub enum SignalEnvelopeHandlerError {
    #[error("signal envelope input '{input_name}' is missing")]
    MissingInput { input_name: &'static str },
    #[error("signal envelope input '{input_name}' must be a {expected}")]
    InvalidInput {
        input_name: &'static str,
        expected: &'static str,
    },
}

pub struct NodeSignalEnvelopeRequest<'a> {
    pub node_id: EngineNodeId,
    pub inputs: &'a HashMap<String, NodeValue>,
}

#[derive(Debug, Default, Clone, Copy)]
struct EnvelopeState {
    value: f32,
    hold_frames_remaining: i32,
}

pub struct SignalEnvelopeHandler {
    state_cache: HashMap<EngineNodeId, EnvelopeState>,
}

impl Default for SignalEnvelopeHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalEnvelopeHandler {
    pub fn new() -> Self {
        Self {
            state_cache: HashMap::new(),
        }
    }

    pub fn clear_cache(&mut self) {
        self.state_cache.clear();
    }

    pub fn execute_handler(
        &mut self,
        request: &NodeSignalEnvelopeRequest,
    ) -> Result<Vec<NodeValue>, SignalEnvelopeHandlerError> {
        let input = read_float_input(request.inputs, "Input")?;
        let attack = read_float_input(request.inputs, "Attack")?.clamp(0.0, 1.0);
        let release = read_float_input(request.inputs, "Release")?.clamp(0.0, 1.0);
        let hold_frames = read_int_input(request.inputs, "Hold Frames")?.max(0);
        let threshold = read_float_input(request.inputs, "Threshold")?.max(0.0);

        let mut target = input;
        if target.abs() < threshold {
            target = 0.0;
        }

        let state = self.state_cache.entry(request.node_id).or_default();

        if target > state.value {
            state.value += (target - state.value) * attack;
            state.hold_frames_remaining = hold_frames;
        } else if state.value > target {
            let should_hold = target == 0.0 && state.hold_frames_remaining > 0;
            if should_hold {
                state.hold_frames_remaining -= 1;
            } else {
                state.value += (target - state.value) * release;
                state.hold_frames_remaining = 0;
            }
        } else {
            state.hold_frames_remaining = hold_frames;
        }

        if state.value.abs() < 1e-6 {
            state.value = 0.0;
        }

        Ok(vec![NodeValue::Float(state.value)])
    }
}

fn read_float_input(
    inputs: &HashMap<String, NodeValue>,
    input_name: &'static str,
) -> Result<f32, SignalEnvelopeHandlerError> {
    match inputs.get(input_name) {
        Some(NodeValue::Float(value)) => Ok(*value),
        Some(NodeValue::Int(value)) => Ok(*value as f32),
        Some(_) => Err(SignalEnvelopeHandlerError::InvalidInput {
            input_name,
            expected: "Float",
        }),
        None => Err(SignalEnvelopeHandlerError::MissingInput { input_name }),
    }
}

fn read_int_input(
    inputs: &HashMap<String, NodeValue>,
    input_name: &'static str,
) -> Result<i32, SignalEnvelopeHandlerError> {
    match inputs.get(input_name) {
        Some(NodeValue::Int(value)) => Ok(*value),
        Some(NodeValue::Float(value)) => Ok(*value as i32),
        Some(_) => Err(SignalEnvelopeHandlerError::InvalidInput {
            input_name,
            expected: "Int",
        }),
        None => Err(SignalEnvelopeHandlerError::MissingInput { input_name }),
    }
}