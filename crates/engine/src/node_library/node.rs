use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use media::frame::Frame;
use crate::pipelines::common::Pipeline;
use std::any::Any;

/// Unique identifier for a node instance in the graph
pub type NodeId = usize;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Node {
    /// The name of this node
    pub name: String,

    /// Inputs to the node
    pub inputs: Vec<NodeInput>,

    /// Outputs of the node
    pub outputs: Vec<NodeOutput>,

    /// What this node does
    pub executor: NodeExecutionPlan,

    /// A short description of the node (shown on hover)
    #[serde(default)]
    pub short_description: String,

    /// A long description of the node (shown when info button is clicked)
    #[serde(default)]
    pub long_description: String,

    /// Sub-folders (in the UI) that this node should appear under
    #[serde(default)]
    pub sub_folders: Vec<String>,

    /// Keywords used to help find this node when searching
    #[serde(default)]
    pub search_keywords: Vec<String>,
}

pub enum NodeType {
    // Source nodes
    MediaInput { frame: Frame },
    
    // Effect nodes (single input → single output)
    Effect { pipeline: Box<dyn Pipeline>, params: Box<dyn Any> },
    
    // Merge nodes (multiple inputs → single output)
    // Merge { blend_mode: BlendMode },
    
    // Output node
    Output,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NodeInput {
    /// The name of input
    pub name: String,
    
    /// The kind of input
    pub kind: NodeInputKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NodeOutput {
    /// The name of output
    pub name: String,
    
    /// The kind of output
    pub kind: NodeOutputKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeOutputKind {
    Frame,
    Midi,
    Bool,
    Int,
    Float,
    Dimensions,
    Pixel,
    Text,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum NodeInputKind {
    Frame,
    Midi,
    Bool {
        #[serde(default)]
        default: bool,
    },
    Int {
        #[serde(default)]
        default: i32,
        #[serde(default)]
        min: Option<i32>,
        #[serde(default)]
        max: Option<i32>,
        #[serde(default = "default_step_i32")]
        step: i32,
        #[serde(default)]
        no_sub_step: bool,
        #[serde(default)]
        input_ui: NumberInputUiMode,
    },
    Float {
        #[serde(default)]
        default: f32,
        #[serde(default)]
        min: Option<f32>,
        #[serde(default)]
        max: Option<f32>,
        #[serde(default = "default_step_f32")]
        step: f32,
        #[serde(default)]
        no_sub_step: bool,
        #[serde(default)]
        input_ui: NumberInputUiMode,
    },
    Dimensions {
        #[serde(default)]
        default: (u32, u32),
    },
    Pixel {
        #[serde(default)]
        default: [f32; 4],
        #[serde(default)]
        no_opacity: bool,
        #[serde(default)]
        no_color: bool,
    },
    Enum {
        choices: Vec<String>,
        #[serde(default)]
        default_idx: Option<usize>,
    },
    Text {
        #[serde(default)]
        default: String,
        #[serde(default)]
        max_len: Option<u64>,
        #[serde(default = "default_ui_lines")]
        ui_lines: u64,
    },
    File {
        #[serde(default)]
        kind: FileKind,
        #[serde(default)]
        default: Option<PathBuf>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NodeExecutionPlan {
    Shader {
        /// Path of a shader file relative to the node.json file
        source: PathBuf,
    },
    BuiltIn(BuiltInHandler),
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BuiltInHandler {
    SumInputs,
    MultiplyInputs,
    ImageSource
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum NumberInputUiMode {
    #[default]
    TextInput,
    Slider,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum FileKind {
    #[default]
    Any,
    Video,
    Image,
    Midi,
}

fn default_step_i32() -> i32 { 1 }
fn default_step_f32() -> f32 { 0.1 }
fn default_ui_lines() -> u64 { 1 }