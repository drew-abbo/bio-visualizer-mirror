use super::node_graph::NodeGraphState;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::SystemTime;
use util::local_data::project::OpenProject;

pub struct EditorStateContext {
    last_edit: Option<SystemTime>,
    open_project: Option<OpenProject<NodeGraphState>>,

    last_saved_hash: Option<u64>,
    last_saved_content_hash: Option<u64>,
}

impl EditorStateContext {
    pub fn new() -> Self {
        Self {
            last_edit: None,
            open_project: None,
            last_saved_hash: None,
            last_saved_content_hash: None,
        }
    }

    pub fn compute_state_hash(state: &NodeGraphState) -> Option<u64> {
        postcard::to_allocvec(state).ok().map(|bytes| {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            hasher.finish()
        })
    }

    pub fn compute_content_hash(state: &NodeGraphState) -> Option<u64> {
        let mut content_only_state = state.clone();
        content_only_state.graph_view = None;
        content_only_state.legacy_graph_view_zoom = None;
        Self::compute_state_hash(&content_only_state)
    }

    /// Set the open project
    pub fn set_project(&mut self, project: OpenProject<NodeGraphState>) {
        // Compute and store hash of the initial state
        self.last_saved_hash = Self::compute_state_hash(project.data());
        self.last_saved_content_hash = Self::compute_content_hash(project.data());
        // Clear any previous unsaved changes flag
        self.last_edit = None;
        self.open_project = Some(project);
    }

    pub fn node_graph(&self) -> Option<&NodeGraphState> {
        self.open_project.as_ref().map(|p| p.data())
    }

    pub fn node_graph_mut(&mut self) -> Option<&mut NodeGraphState> {
        self.open_project.as_mut().map(|p| p.data_mut())
    }

    pub fn has_open_project(&self) -> bool {
        self.open_project.is_some()
    }

    pub fn mark_edited(&mut self) {
        self.last_edit = Some(SystemTime::now());
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.last_edit.is_some()
    }

    pub fn has_only_view_unsaved_changes(&self) -> bool {
        let Some(state) = self.node_graph() else {
            return false;
        };

        let Some(last_saved_content_hash) = self.last_saved_content_hash else {
            return false;
        };

        let Some(current_content_hash) = Self::compute_content_hash(state) else {
            return false;
        };

        current_content_hash == last_saved_content_hash
    }

    /// Check if the graph state hash changed and mark as edited if so
    pub fn check_hash_changed(&mut self, current_hash: u64) {
        if let Some(last_hash) = self.last_saved_hash
            && current_hash != last_hash
        {
            self.mark_edited();
        }
    }

    /// Returns Ok(true) if data was written, Ok(false) if no changes.
    pub fn save(&mut self) -> Result<bool, String> {
        let Some(ref mut project) = self.open_project else {
            return Err("No project is currently open".to_string());
        };

        // Might be good to let the user know here with a popup
        let result = project
            .save()
            .map_err(|e| format!("Failed to save project: {}", e))?;

        // Update the saved state hash and clear unsaved changes flag
        self.last_saved_hash = Self::compute_state_hash(project.data());
        self.last_saved_content_hash = Self::compute_content_hash(project.data());
        self.last_edit = None;
        Ok(result)
    }

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
