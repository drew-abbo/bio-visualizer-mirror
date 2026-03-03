//! The [AppContext] struct, which holds global state and sate of the editor.
use crate::app_area::editor::NodeGraphState;

/// The [EditorContext] will go into the data.json of the project.
/// The [AppState] will be saved in a global.json file and will hold things like app dimensions, theme, and other settings that should persist across sessions but not be tied to a specific project.
pub struct AppContext {
    editor_context: EditorContext,
    app_state: AppState,
}

pub struct AppState {
    app_dimensions: AppDimensions,
}

pub struct AppDimensions {
    width: f32,
    height: f32,
}

pub struct EditorContext {
    node_graph_state: NodeGraphState,
    zoom_level: f32,
}