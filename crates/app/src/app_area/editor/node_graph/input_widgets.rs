use egui::{self, Ui};
use egui_snarl::NodeId as SnarlNodeId;
use engine::node::engine_node::NodeInput;
use engine::node::{NodeInputKind, NodeLibrary};
use engine::node_graph::InputValue;
use std::collections::HashMap;
use std::path::PathBuf;

use media::midi::streams::list_ports;
use util::channels::message_channel;

/// I just hate strings
const VIDEO_NODE_NAME: &str = "Video";
const IMAGE_NODE_NAME: &str = "Image";

pub struct InputWidgetState {
    pending_file_dialogs: HashMap<String, message_channel::Inbox<Option<PathBuf>>>,
}

impl InputWidgetState {
    pub fn new() -> Self {
        Self {
            pending_file_dialogs: HashMap::new(),
        }
    }
}

impl Default for InputWidgetState {
    fn default() -> Self {
        Self::new()
    }
}

enum FileFilter {
    Video,
    Image,
    Any,
}

fn file_dialog_key(node_id: SnarlNodeId, input_name: &str) -> String {
    format!("{:?}:{}", node_id, input_name)
}

/// Renders the appropriate input widget based on the NodeInputKind
/// Declutters the node_graph
pub fn show_input_widget(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    node_name: &str,
    node_library: &NodeLibrary,
    node_id: SnarlNodeId,
    state: &mut InputWidgetState,
) {
    match &input_def.kind {
        NodeInputKind::File { .. } => {
            show_file_input(
                ui,
                input_values,
                input_def,
                node_name,
                node_library,
                node_id,
                state,
            );
        }
        NodeInputKind::Bool { default } => {
            show_bool_input(ui, input_values, input_def, *default);
        }
        NodeInputKind::Int {
            default, min, max, ..
        } => {
            show_int_input(ui, input_values, input_def, *default, *min, *max);
        }
        NodeInputKind::Float {
            default, min, max, ..
        } => {
            show_float_input(ui, input_values, input_def, *default, *min, *max);
        }
        NodeInputKind::Text { default, .. } => {
            show_text_input(ui, input_values, input_def, default);
        }
        NodeInputKind::Dimensions { default } => {
            show_dimensions_input(ui, input_values, input_def, *default);
        }
        NodeInputKind::Pixel { default, .. } => {
            show_pixel_input(ui, input_values, input_def, *default);
        }
        NodeInputKind::Frame | NodeInputKind::MidiPacket => {
            ui.label("Must be connected");
        }
        NodeInputKind::Enum {
            choices,
            default_idx,
            ..
        } => {
            show_enum_input(ui, input_values, input_def, choices, *default_idx);
        }
        NodeInputKind::PortSelection => {
            show_port_selection_input(ui, input_values, input_def);
        }
    }
}

fn show_port_selection_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
) {
    let ports: Vec<String> = list_ports()
        .ok()
        .map(|iter| iter.map(|port| port.port_name().to_string()).collect())
        .filter(|ports: &Vec<String>| !ports.is_empty())
        .unwrap_or_else(|| vec!["No ports available".to_string()]);

    let mut selected_port = match input_values.get(&input_def.name) {
        Some(InputValue::Text(value)) if !value.is_empty() => value.clone(),
        _ => {
            let default = ports.first().cloned().unwrap_or_default();
            input_values.insert(input_def.name.clone(), InputValue::Text(default.clone()));
            default
        }
    };

    egui::ComboBox::from_id_salt(&input_def.name)
        .selected_text(&selected_port)
        .show_ui(ui, |ui| {
            for port in &ports {
                if ui
                    .selectable_value(&mut selected_port, port.clone(), port)
                    .changed()
                {
                    input_values.insert(input_def.name.clone(), InputValue::Text(port.clone()));
                }
            }
        });
}

fn show_file_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    node_name: &str,
    node_library: &NodeLibrary,
    node_id: SnarlNodeId,
    state: &mut InputWidgetState,
) {
    let key = file_dialog_key(node_id, &input_def.name);

    if let Some(inbox) = state.pending_file_dialogs.get(&key) {
        match inbox.check_non_blocking() {
            Ok(Some(Some(path))) => {
                input_values.insert(input_def.name.clone(), InputValue::File(path));
                state.pending_file_dialogs.remove(&key);
            }
            Ok(Some(None)) | Err(_) => {
                state.pending_file_dialogs.remove(&key);
            }
            Ok(None) => {
                // Keep repainting while dialog is pending so selection can apply immediately.
                ui.ctx().request_repaint();
            }
        }
    }

    let current_value = input_values.get(&input_def.name);
    let display_text = if let Some(InputValue::File(path)) = current_value {
        path.to_string_lossy()
    } else {
        "Select file...".into()
    };

    if ui.button(display_text).clicked() && !state.pending_file_dialogs.contains_key(&key) {
        let filter = if let Some(def) = node_library.get_definition(node_name) {
            match def.node.name.as_str() {
                VIDEO_NODE_NAME => FileFilter::Video,
                IMAGE_NODE_NAME => FileFilter::Image,
                _ => FileFilter::Any,
            }
        } else {
            FileFilter::Any
        };

        let (inbox, outbox) = message_channel::new();
        state.pending_file_dialogs.insert(key, inbox);

        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new();

            match filter {
                FileFilter::Video => {
                    dialog = dialog.add_filter(
                        "Video Files",
                        &[
                            "mp4", "avi", "mov", "mkv", "webm", "flv", "wmv", "m4v", "mpg", "mpeg",
                        ],
                    );
                }
                FileFilter::Image => {
                    dialog = dialog.add_filter(
                        "Image Files",
                        &[
                            "png", "jpg", "jpeg", "bmp", "gif", "tiff", "tif", "webp", "ico",
                        ],
                    );
                }
                FileFilter::Any => {}
            }

            let _ = outbox.send(dialog.pick_file());
        });

        ui.ctx().request_repaint();
    }
}

fn show_bool_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    default: bool,
) {
    let mut value = if let Some(InputValue::Bool(v)) = input_values.get(&input_def.name) {
        *v
    } else {
        default
    };

    if ui.checkbox(&mut value, "").changed() {
        input_values.insert(input_def.name.clone(), InputValue::Bool(value));
    }
}

fn show_int_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    default: i32,
    min: Option<i32>,
    max: Option<i32>,
) {
    let mut value = if let Some(InputValue::Int(v)) = input_values.get(&input_def.name) {
        *v
    } else {
        default
    };

    let changed = if let (Some(min_val), Some(max_val)) = (min, max) {
        ui.add(egui::Slider::new(&mut value, min_val..=max_val))
            .changed()
    } else {
        ui.add(egui::DragValue::new(&mut value)).changed()
    };

    if changed {
        input_values.insert(input_def.name.clone(), InputValue::Int(value));
    }
}

fn show_float_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    default: f32,
    min: Option<f32>,
    max: Option<f32>,
) {
    let mut value = if let Some(InputValue::Float(v)) = input_values.get(&input_def.name) {
        *v
    } else {
        default
    };

    let changed = if let (Some(min_val), Some(max_val)) = (min, max) {
        ui.add(egui::Slider::new(&mut value, min_val..=max_val))
            .changed()
    } else {
        ui.add(egui::DragValue::new(&mut value).speed(0.1))
            .changed()
    };

    if changed {
        input_values.insert(input_def.name.clone(), InputValue::Float(value));
    }
}

fn show_text_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    default: &str,
) {
    let mut value = if let Some(InputValue::Text(v)) = input_values.get(&input_def.name) {
        v.clone()
    } else {
        default.to_string()
    };

    if ui.text_edit_singleline(&mut value).changed() {
        input_values.insert(input_def.name.clone(), InputValue::Text(value));
    }
}

/// We don't really use this yet but it's here.
fn show_dimensions_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    default: (u32, u32),
) {
    let (mut width, mut height) =
        if let Some(InputValue::Dimensions { width, height }) = input_values.get(&input_def.name) {
            (*width, *height)
        } else {
            default
        };

    let mut changed = ui
        .add(egui::DragValue::new(&mut width).prefix("W: "))
        .changed();
    changed |= ui
        .add(egui::DragValue::new(&mut height).prefix("H: "))
        .changed();

    if changed {
        input_values.insert(
            input_def.name.clone(),
            InputValue::Dimensions { width, height },
        );
    }
}

/// We don't really use this yet but it's here.
fn show_pixel_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    default: [f32; 4],
) {
    let (r, g, b, a) =
        if let Some(InputValue::Pixel { r, g, b, a }) = input_values.get(&input_def.name) {
            (*r, *g, *b, *a)
        } else {
            (default[0], default[1], default[2], default[3])
        };

    let mut color = egui::Color32::from_rgba_premultiplied(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (a * 255.0) as u8,
    );

    if ui.color_edit_button_srgba(&mut color).changed() {
        let [r_u8, g_u8, b_u8, a_u8] = color.to_array();
        input_values.insert(
            input_def.name.clone(),
            InputValue::Pixel {
                r: r_u8 as f32 / 255.0,
                g: g_u8 as f32 / 255.0,
                b: b_u8 as f32 / 255.0,
                a: a_u8 as f32 / 255.0,
            },
        );
    }
}

fn show_enum_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    choices: &[String],
    default_idx: Option<usize>,
) {
    let mut selected_idx = if let Some(InputValue::Enum(idx)) = input_values.get(&input_def.name) {
        *idx
    } else {
        let default = default_idx.unwrap_or(0);
        // Initialize with default value if not set
        input_values.insert(input_def.name.clone(), InputValue::Enum(default));
        default
    };

    egui::ComboBox::from_id_salt(&input_def.name)
        .selected_text(choices.get(selected_idx).unwrap_or(&"None".to_string()))
        .show_ui(ui, |ui| {
            for (idx, option) in choices.iter().enumerate() {
                if ui
                    .selectable_value(&mut selected_idx, idx, option)
                    .changed()
                {
                    input_values.insert(input_def.name.clone(), InputValue::Enum(idx));
                }
            }
        });
}
