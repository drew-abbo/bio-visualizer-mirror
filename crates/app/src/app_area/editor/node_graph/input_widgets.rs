<<<<<<< HEAD
use engine::node::engine_node::NodeInput;
=======
use engine::node::node::NodeInput;
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
use engine::node::{NodeInputKind, NodeLibrary};
use engine::node_graph::InputValue;
use std::collections::HashMap;
use util::egui::{self, Ui};

/// I just hate strings
const VIDEO_NODE_NAME: &str = "Video";
const IMAGE_NODE_NAME: &str = "Image";

/// Renders the appropriate input widget based on the NodeInputKind
/// Declutters the node_graph
pub fn show_input_widget(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    node_name: &str,
    node_library: &NodeLibrary,
) {
    match &input_def.kind {
        NodeInputKind::File { .. } => {
            show_file_input(ui, input_values, input_def, node_name, node_library);
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
        NodeInputKind::Frame | NodeInputKind::Midi => {
            ui.label("Must be connected");
        }
        NodeInputKind::Enum {
            choices,
            default_idx,
            ..
        } => {
            show_enum_input(ui, input_values, input_def, choices, *default_idx);
        }
    }
}

fn show_file_input(
    ui: &mut Ui,
    input_values: &mut HashMap<String, InputValue>,
    input_def: &NodeInput,
    node_name: &str,
    node_library: &NodeLibrary,
) {
    let current_value = input_values.get(&input_def.name);
    let display_text = if let Some(InputValue::File(path)) = current_value {
        path.to_string_lossy()
    } else {
        "Select file...".into()
    };

    if ui.button(display_text).clicked() {
        // Create file dialog with appropriate filters based on node name
        let mut dialog = rfd::FileDialog::new();

        // I think it is reasonable to match on the node name since these should be built in
        if let Some(def) = node_library.get_definition(node_name) {
            match def.node.name.as_str() {
                VIDEO_NODE_NAME => {
                    dialog = dialog.add_filter(
                        "Video Files",
                        &[
<<<<<<< HEAD
                            "mp4", "avi", "mov", "mkv", "webm", "flv", "wmv", "m4v", "mpg", "mpeg",
=======
                            "mp4", "avi", "mov", "mkv", "webm", "flv", "wmv", "m4v", "mpg",
                            "mpeg",
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
                        ],
                    );
                }
                IMAGE_NODE_NAME => {
                    dialog = dialog.add_filter(
                        "Image Files",
<<<<<<< HEAD
                        &[
                            "png", "jpg", "jpeg", "bmp", "gif", "tiff", "tif", "webp", "ico",
                        ],
=======
                        &["png", "jpg", "jpeg", "bmp", "gif", "tiff", "tif", "webp", "ico"],
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
                    );
                }
                _ => {
                    // Default: all files... gg
                }
            }
        }

        if let Some(path) = dialog.pick_file() {
            input_values.insert(input_def.name.clone(), InputValue::File(path));
        }
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
<<<<<<< HEAD
    let (r, g, b, a) =
        if let Some(InputValue::Pixel { r, g, b, a }) = input_values.get(&input_def.name) {
            (*r, *g, *b, *a)
        } else {
            (default[0], default[1], default[2], default[3])
        };
=======
    let (r, g, b, a) = if let Some(InputValue::Pixel { r, g, b, a }) =
        input_values.get(&input_def.name)
    {
        (*r, *g, *b, *a)
    } else {
        (default[0], default[1], default[2], default[3])
    };
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)

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
