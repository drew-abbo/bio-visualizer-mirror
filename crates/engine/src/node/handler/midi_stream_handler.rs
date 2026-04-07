use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use media::fps::{Fps, consts::FPS_30};
use media::midi::streams::{LiveMidiStream, MidiStream, MidiStreamError, Port, list_ports};
use media::playback_stream::PlaybackStream;

use crate::graph_executor::NodeValue;
use crate::node_graph::EngineNodeId;

use super::timed_stream_handler::TimedStreamHandler;

#[derive(Debug, thiserror::Error)]
pub enum MidiStreamHandlerError {
    #[error("midi input '{input_name}' is missing")]
    MissingInput { input_name: &'static str },
    #[error("midi input '{input_name}' must be a {expected}")]
    InvalidInput {
        input_name: &'static str,
        expected: &'static str,
    },
    #[error("no MIDI input ports are available")]
    NoPortsAvailable,
    #[error("midi port '{query}' was not found")]
    PortNotFound { query: String },
    #[error("midi stream error: {0}")]
    Stream(#[from] MidiStreamError),
}

pub struct NodeMidiStreamRequest<'a> {
    pub node_id: EngineNodeId,
    pub inputs: &'a HashMap<String, NodeValue>,
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct NodeMidiStreamKey {
    node_id: EngineNodeId,
    port_query: Option<String>,
}

pub struct MidiStreamHandler {
    stream_cache: HashMap<NodeMidiStreamKey, Box<dyn MidiStream>>,
    paused: bool,
}

fn midi_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| match std::env::var("BIO_MIDI_DEBUG") {
        Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => false,
    })
}

impl Default for MidiStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiStreamHandler {
    fn default_property_outputs(key: media::midi::Key) -> Vec<NodeValue> {
        vec![
            NodeValue::Bool(false),
            NodeValue::Int(0),
            NodeValue::Float(0.0),
            NodeValue::Float(0.0),
            NodeValue::Float(key.as_frequency() as f32),
            NodeValue::Float(0.0),
            NodeValue::Int(0),
            NodeValue::Int(0),
            NodeValue::Int(0),
            NodeValue::Float(0.0),
            NodeValue::Int(0),
            NodeValue::Float(0.0),
            NodeValue::Int(0),
            NodeValue::Float(0.0),
            NodeValue::Float(0.0),
        ]
    }

    pub fn new() -> Self {
        Self {
            stream_cache: HashMap::new(),
            paused: false,
        }
    }

    pub fn pause_all_streams(&mut self) {
        <Self as TimedStreamHandler>::pause_all_streams(self);
    }

    pub fn play_all_streams(&mut self) {
        <Self as TimedStreamHandler>::play_all_streams(self);
    }

    pub fn clear_cache(&mut self) {
        <Self as TimedStreamHandler>::clear_cache(self);
    }

    pub fn set_target_fps_all(&mut self, target_fps: Fps) {
        <Self as TimedStreamHandler>::set_target_fps_all(self, target_fps);
    }

    pub fn set_target_fps_for_nodes(
        &mut self,
        target_fps: Fps,
        active_nodes: &HashSet<EngineNodeId>,
    ) {
        <Self as TimedStreamHandler>::set_target_fps_for_nodes(self, target_fps, active_nodes);
    }

    pub fn set_playback_for_nodes(&mut self, active_nodes: &HashSet<EngineNodeId>) {
        <Self as TimedStreamHandler>::set_playback_for_nodes(self, active_nodes);
    }

    pub fn execute_handler(
        &mut self,
        request: &NodeMidiStreamRequest,
    ) -> Result<Vec<NodeValue>, MidiStreamHandlerError> {
        let stream = self.create_stream(request)?;
        let packet = stream.fetch()?;

        Ok(vec![NodeValue::Midi(packet)])
    }

    pub fn extract_properties(
        &self,
        inputs: &HashMap<String, NodeValue>,
    ) -> Result<Vec<NodeValue>, MidiStreamHandlerError> {
        let key_value = match inputs.get("Key") {
            Some(NodeValue::Int(value)) => *value,
            Some(NodeValue::Float(value)) => *value as i32,
            Some(_) => {
                return Err(MidiStreamHandlerError::InvalidInput {
                    input_name: "Key",
                    expected: "Int",
                });
            }
            None => 60,
        }
        .clamp(0, 127) as u8;

        let key = media::midi::Key::from_u8(key_value).expect("clamped key");

        let packet = match inputs.get("Input") {
            Some(NodeValue::Midi(packet)) => packet,
            Some(_) => {
                return Err(MidiStreamHandlerError::InvalidInput {
                    input_name: "Input",
                    expected: "Midi",
                });
            }
            None => {
                return Ok(Self::default_property_outputs(key));
            }
        };

        let controller = match inputs.get("Controller") {
            Some(NodeValue::Int(value)) => *value,
            Some(NodeValue::Float(value)) => *value as i32,
            Some(_) => {
                return Err(MidiStreamHandlerError::InvalidInput {
                    input_name: "Controller",
                    expected: "Int",
                });
            }
            None => 1,
        }
        .clamp(0, 127) as u8;

        let velocity = packet.key_velocity(key);
        let velocity_normalized = velocity as f32 / 127.0;
        let selected_key_frequency = key.as_frequency() as f32;
        let active_frequency = packet
            .strongest_key()
            .map(|active_key| active_key.as_frequency() as f32)
            .unwrap_or(0.0);
        let average_frequency = packet.average_frequency().unwrap_or(0.0) as f32;
        let max_velocity_normalized = packet.max_velocity() as f32 / 127.0;
        let control_value = packet.control_value(controller);
        let control_value_normalized = packet.control_value_normalized(controller);
        let max_control_value = packet.max_control_value();
        let max_control_value_normalized = packet.max_control_value_normalized();
        let pitch_bend = packet.pitch_bend();
        let pitch_bend_normalized = packet.pitch_bend_normalized();
        let channel_pressure = packet.channel_pressure();
        let channel_pressure_normalized = packet.channel_pressure_normalized();
        let active_key_count = packet.active_key_count() as i32;
        let signal = velocity_normalized
            .max(max_velocity_normalized)
            .max(control_value_normalized)
            .max(max_control_value_normalized)
            .max(channel_pressure_normalized)
            .max(pitch_bend_normalized.abs());

        #[cfg(debug_assertions)]
        if midi_debug_enabled() {
            util::debug_log_info!(
                "MIDI Properties: key_on={}, velocity={}, velocity_normalized={:.3}, frequency={:.3}, selected_key_frequency={:.3}, average_frequency={:.3}, max_velocity={}, max_velocity_normalized={:.3}, active_keys={}, cc{}_value={}, cc{}_normalized={:.3}, max_cc_value={}, max_cc_normalized={:.3}, pitch_bend={}, pitch_bend_normalized={:.3}, channel_pressure={}, channel_pressure_normalized={:.3}, signal={:.3}",
                packet.is_key_on(key),
                velocity,
                velocity_normalized,
                active_frequency,
                selected_key_frequency,
                average_frequency,
                packet.max_velocity(),
                max_velocity_normalized,
                active_key_count,
                controller,
                control_value,
                controller,
                control_value_normalized,
                max_control_value,
                max_control_value_normalized,
                pitch_bend,
                pitch_bend_normalized,
                channel_pressure,
                channel_pressure_normalized,
                signal
            );
        }

        Ok(vec![
            NodeValue::Bool(packet.is_key_on(key)),
            NodeValue::Int(velocity as i32),
            NodeValue::Float(velocity_normalized),
            NodeValue::Float(active_frequency),
            NodeValue::Float(selected_key_frequency),
            NodeValue::Float(average_frequency),
            NodeValue::Int(packet.max_velocity() as i32),
            NodeValue::Int(active_key_count),
            NodeValue::Int(control_value as i32),
            NodeValue::Float(control_value_normalized),
            NodeValue::Int(pitch_bend as i32),
            NodeValue::Float(pitch_bend_normalized),
            NodeValue::Int(channel_pressure as i32),
            NodeValue::Float(channel_pressure_normalized),
            NodeValue::Float(signal),
        ])
    }

    fn create_stream(
        &mut self,
        request: &NodeMidiStreamRequest,
    ) -> Result<&mut Box<dyn MidiStream>, MidiStreamHandlerError> {
        let port_query = resolve_port_query(request.inputs)?;
        let key = NodeMidiStreamKey {
            node_id: request.node_id,
            port_query: port_query.clone(),
        };

        let stale_keys: Vec<NodeMidiStreamKey> = self
            .stream_cache
            .keys()
            .filter(|cached_key| cached_key.node_id == request.node_id && **cached_key != key)
            .cloned()
            .collect();
        for stale_key in stale_keys {
            self.stream_cache.remove(&stale_key);
        }

        if !self.stream_cache.contains_key(&key) {
            let port = select_port(port_query.as_deref())?;
            let mut stream = Box::new(LiveMidiStream::new(port, FPS_30, self.paused)?);
            if self.paused {
                stream.pause();
            } else {
                stream.play();
            }
            self.stream_cache.insert(key.clone(), stream);
        }

        Ok(self
            .stream_cache
            .get_mut(&key)
            .expect("stream inserted above"))
    }
}

impl TimedStreamHandler for MidiStreamHandler {
    type Stream = Box<dyn MidiStream>;

    fn for_each_stream_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(EngineNodeId, &mut Self::Stream),
    {
        for (key, stream) in self.stream_cache.iter_mut() {
            f(key.node_id, stream);
        }
    }

    fn set_paused_state(&mut self, paused: bool) {
        self.paused = paused;
    }

    fn is_paused_state(&self) -> bool {
        self.paused
    }

    fn clear_stream_cache(&mut self) {
        self.stream_cache.clear();
    }

    fn stream_pause(stream: &mut Self::Stream) {
        stream.pause();
    }

    fn stream_play(stream: &mut Self::Stream) {
        stream.play();
    }

    fn stream_set_target_fps(stream: &mut Self::Stream, target_fps: Fps) {
        stream.set_target_fps(target_fps);
    }
}

fn resolve_port_query(
    inputs: &HashMap<String, NodeValue>,
) -> Result<Option<String>, MidiStreamHandlerError> {
    match inputs.get("Port") {
        Some(NodeValue::Enum(index)) => Ok(Some(index.to_string())),
        Some(NodeValue::Text(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        Some(_) => Err(MidiStreamHandlerError::InvalidInput {
            input_name: "Port",
            expected: "Enum",
        }),
        None => Err(MidiStreamHandlerError::MissingInput { input_name: "Port" }),
    }
}

fn select_port(query: Option<&str>) -> Result<Port, MidiStreamHandlerError> {
    let ports: Vec<Port> = list_ports()?.collect();
    let Some(first_port) = ports.first().cloned() else {
        return Err(MidiStreamHandlerError::NoPortsAvailable);
    };

    let Some(query) = query else {
        return Ok(first_port);
    };

    if let Ok(index) = query.parse::<usize>()
        && let Some(port) = ports.get(index)
    {
        return Ok(port.clone());
    }

    ports
        .into_iter()
        .find(|port| port.port_name() == query)
        .ok_or_else(|| MidiStreamHandlerError::PortNotFound {
            query: query.to_string(),
        })
}
