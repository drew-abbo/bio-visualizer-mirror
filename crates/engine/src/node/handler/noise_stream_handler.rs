use std::collections::{HashMap, HashSet};

use media::fps::Fps;
use media::noise::{NoiseStream, NoiseStreamError, ProceduralNoiseStream};

use crate::graph_executor::NodeValue;
use crate::node::engine_node::NoiseKind;
use crate::node_graph::EngineNodeId;

use super::timed_stream_handler::TimedStreamHandler;

#[derive(Debug, thiserror::Error)]
pub enum NoiseStreamHandlerError {
    #[error("noise input '{input_name}' for '{noise_kind:?}' is missing")]
    MissingInput {
        noise_kind: NoiseKind,
        input_name: &'static str,
    },
    #[error("noise input '{input_name}' for '{noise_kind:?}' must be a {expected}")]
    InvalidInput {
        noise_kind: NoiseKind,
        input_name: &'static str,
        expected: &'static str,
    },
    #[error("noise stream error: {0}")]
    Stream(#[from] NoiseStreamError),
}

pub struct NodeNoiseStreamRequest<'a> {
    pub node_id: EngineNodeId,
    pub noise_kind: NoiseKind,
    pub inputs: &'a HashMap<String, NodeValue>,
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct NodeNoiseStreamKey {
    node_id: EngineNodeId,
    config: NoiseConfigKey,
}

#[derive(Clone, Hash, Eq, PartialEq)]
enum NoiseConfigKey {
    Random,
    Sin {
        speed_bits: u32,
        frequency_bits: u32,
    },
    Perlin {
        speed_bits: u32,
        frequency_bits: u32,
        octaves: i32,
    },
}

pub struct NoiseStreamHandler {
    stream_cache: HashMap<NodeNoiseStreamKey, Box<dyn NoiseStream>>,
    paused: bool,
}

impl Default for NoiseStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl NoiseStreamHandler {
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
        request: &NodeNoiseStreamRequest,
    ) -> Result<Vec<NodeValue>, NoiseStreamHandlerError> {
        let stream = self.create_stream(request)?;
        let sample = stream.fetch()?;

        Ok(vec![NodeValue::Float(sample)])
    }

    fn create_stream(
        &mut self,
        request: &NodeNoiseStreamRequest,
    ) -> Result<&mut Box<dyn NoiseStream>, NoiseStreamHandlerError> {
        let config = build_config_key(request.noise_kind, request.inputs)?;
        let key = NodeNoiseStreamKey {
            node_id: request.node_id,
            config,
        };

        let stale_keys: Vec<NodeNoiseStreamKey> = self
            .stream_cache
            .keys()
            .filter(|cached_key| cached_key.node_id == request.node_id && **cached_key != key)
            .cloned()
            .collect();
        for stale_key in stale_keys {
            self.stream_cache.remove(&stale_key);
        }

        if !self.stream_cache.contains_key(&key) {
            let mut stream = build_noise_stream(request)?;
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

impl TimedStreamHandler for NoiseStreamHandler {
    type Stream = Box<dyn NoiseStream>;

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

fn build_config_key(
    noise_kind: NoiseKind,
    inputs: &HashMap<String, NodeValue>,
) -> Result<NoiseConfigKey, NoiseStreamHandlerError> {
    match noise_kind {
        NoiseKind::Random => Ok(NoiseConfigKey::Random),
        NoiseKind::Sin => {
            let speed = read_float_input(inputs, noise_kind, "Speed")?;
            let frequency = read_float_input(inputs, noise_kind, "Frequency")?;
            Ok(NoiseConfigKey::Sin {
                speed_bits: speed.to_bits(),
                frequency_bits: frequency.to_bits(),
            })
        }
        NoiseKind::Perlin => {
            let speed = read_float_input(inputs, noise_kind, "Speed")?;
            let frequency = read_float_input(inputs, noise_kind, "Frequency")?;
            let octaves = read_int_input(inputs, noise_kind, "Octaves")?;
            Ok(NoiseConfigKey::Perlin {
                speed_bits: speed.to_bits(),
                frequency_bits: frequency.to_bits(),
                octaves,
            })
        }
    }
}

fn build_noise_stream(
    request: &NodeNoiseStreamRequest,
) -> Result<Box<dyn NoiseStream>, NoiseStreamHandlerError> {
    match request.noise_kind {
        NoiseKind::Random => {
            let stream = ProceduralNoiseStream::new(|_t_seconds| Ok(rand::random::<f32>()));
            Ok(Box::new(stream))
        }
        NoiseKind::Sin => {
            let speed = read_float_input(request.inputs, request.noise_kind, "Speed")?;
            let frequency = read_float_input(request.inputs, request.noise_kind, "Frequency")?;

            let stream = ProceduralNoiseStream::new(move |t_seconds| {
                let phase = (t_seconds as f32) * speed * frequency * std::f32::consts::TAU;
                Ok(((phase.sin() * 0.5) + 0.5).clamp(0.0, 1.0))
            });
            Ok(Box::new(stream))
        }
        NoiseKind::Perlin => {
            let speed = read_float_input(request.inputs, request.noise_kind, "Speed")?;
            let frequency = read_float_input(request.inputs, request.noise_kind, "Frequency")?;
            let octaves = read_int_input(request.inputs, request.noise_kind, "Octaves")?.max(1);

            let stream = ProceduralNoiseStream::new(move |t_seconds| {
                let mut amplitude = 1.0_f32;
                let mut octave_frequency = frequency.max(0.0001);
                let mut value = 0.0_f32;
                let time = t_seconds as f32 * speed;

                for octave in 0..(octaves as u32) {
                    let phase =
                        time * octave_frequency * std::f32::consts::TAU + octave as f32 * 0.37;
                    let wave = phase.sin() * 0.5 + 0.5;
                    value += (wave * 2.0 - 1.0) * amplitude;
                    amplitude *= 0.5;
                    octave_frequency *= 2.0;
                }

                Ok(((value + 1.0) * 0.5).clamp(0.0, 1.0))
            });
            Ok(Box::new(stream))
        }
    }
}

fn read_float_input(
    inputs: &HashMap<String, NodeValue>,
    noise_kind: NoiseKind,
    input_name: &'static str,
) -> Result<f32, NoiseStreamHandlerError> {
    match inputs.get(input_name) {
        Some(NodeValue::Float(value)) => Ok(*value),
        Some(NodeValue::Int(value)) => Ok(*value as f32),
        Some(_) => Err(NoiseStreamHandlerError::InvalidInput {
            noise_kind,
            input_name,
            expected: "Float",
        }),
        None => Err(NoiseStreamHandlerError::MissingInput {
            noise_kind,
            input_name,
        }),
    }
}

fn read_int_input(
    inputs: &HashMap<String, NodeValue>,
    noise_kind: NoiseKind,
    input_name: &'static str,
) -> Result<i32, NoiseStreamHandlerError> {
    match inputs.get(input_name) {
        Some(NodeValue::Int(value)) => Ok(*value),
        Some(_) => Err(NoiseStreamHandlerError::InvalidInput {
            noise_kind,
            input_name,
            expected: "Int",
        }),
        None => Err(NoiseStreamHandlerError::MissingInput {
            noise_kind,
            input_name,
        }),
    }
}
