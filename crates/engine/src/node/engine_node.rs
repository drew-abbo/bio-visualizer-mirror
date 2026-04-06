use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// The structure of the node is still evolving and might change in the future.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EngineNode {
    /// The name of this node
    pub name: String,

    /// Inputs to the node
    pub inputs: Vec<NodeInput>,

    /// Outputs of the node
    pub outputs: Vec<NodeOutput>,

    /// What this node does
    pub executor: NodeExecutionPlan,

    /// A short description of the node
    #[serde(default)]
    pub short_description: String,

    /// A long description of the node
    #[serde(default)]
    pub long_description: String,

    /// Category / Folder this node belongs under
    #[serde(default)]
    pub category: String,

    /// Sub-categories this node belongs under
    #[serde(default)]
    pub subcategories: Vec<String>,

    /// Keywords used to help find this node when searching
    #[serde(default)]
    pub search_keywords: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NodeInput {
    /// The name of input
    pub name: String,

    /// The kind of input
    pub kind: NodeInputKind,

    /// Show Pin
    /// Default to true because that is the most common case
    #[serde(default = "default_show_pin")]
    pub show_pin: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NodeOutput {
    /// The name of output
    pub name: String,

    /// The kind of output
    pub kind: NodeOutputKind,

    /// Show Pin
    #[serde(default = "default_show_pin")]
    pub show_pin: bool,
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
    // Device {
    //     #[serde(default)]
    //     input_ui: DeviceInputUiMode,
    // },
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
    ImageSource,
    VideoSource,
    MidiSource,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum NumberInputUiMode {
    #[default]
    TextInput,
    Slider,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum DeviceInputUiMode {
    #[default]
    Dropdown,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum FileKind {
    #[default]
    Any,
    Video,
    Image,
}

fn default_step_i32() -> i32 {
    1
}
fn default_step_f32() -> f32 {
    0.1
}
fn default_ui_lines() -> u64 {
    1
}

fn default_show_pin() -> bool {
    true
}
