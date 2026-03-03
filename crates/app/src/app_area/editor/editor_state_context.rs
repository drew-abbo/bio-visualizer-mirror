use super::node_graph::NodeGraphState;
use std::time::SystemTime;
use util::local_data::project::OpenProject;

pub struct EditorStateContext {
    pub last_edit: Option<SystemTime>,
    /// The currently open project containing the node graph state
    open_project: Option<OpenProject<NodeGraphState>>,
}

impl EditorStateContext {
    pub fn new() -> Self {
        Self {
            last_edit: None,
            open_project: None,
        }
    }

    /// Set the open project (called when a project is opened)
    pub fn set_project(&mut self, project: OpenProject<NodeGraphState>) {
        self.open_project = Some(project);
    }

    /// Get mutable access to the node graph state
    pub fn node_graph_mut(&mut self) -> Option<&mut NodeGraphState> {
        self.open_project.as_mut().map(|p| p.data_mut())
    }

    /// Check if a project is currently open
    pub fn has_open_project(&self) -> bool {
        self.open_project.is_some()
    }

    pub fn mark_edited(&mut self) {
        self.last_edit = Some(SystemTime::now());
    }

    /// Save the project to disk. Returns Ok(true) if data was written, Ok(false) if no changes.
    pub fn save(&mut self) -> Result<bool, String> {
        let Some(ref mut project) = self.open_project else {
            return Err("No project is currently open".to_string());
        };

        let saved = project
            .save()
            .map_err(|e| format!("Failed to save project: {}", e));

        if saved? {
            self.last_edit = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Close the current project
    pub fn close_project(&mut self) -> Result<(), String> {
        if let Some(project) = self.open_project.take() {
            project
                .close()
                .map(|_| ())
                .map_err(|e| format!("Failed to close project: {}", e))
        } else {
            Ok(())
        }
    }
}
