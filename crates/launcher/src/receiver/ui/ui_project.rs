//! Contains [UiProject], a wrapper around a [Project].

use std::path::Path;
use std::time::SystemTime;

use util::fuzzy_search::FuzzySearchable;
use util::local_data::project::{Project, ProjectError, ProjectHeader, ProjectId};

/// A wrapper around a [Project] with just enough info for the UI.
///
/// All getters are cached and can be refreshed with [Self::refresh].
#[derive(Debug)]
pub struct UiProject {
    project: Project,
    last_touch_time: (SystemTime, String),
    name_change_buffer: String,
    delete_promt_open: bool,
    block_edits: bool,
}

impl UiProject {
    /// Create a [UiProject], wrapping a [Project].
    pub fn new(project: Project) -> Result<Self, ProjectError> {
        Ok(Self {
            last_touch_time: last_touch_time(&project)?,
            name_change_buffer: project.cached_info().name().into(),
            project,
            delete_promt_open: false,
            block_edits: false,
        })
    }

    /// The project's name.
    pub fn name(&self) -> &str {
        self.project.cached_info().name()
    }

    /// The time the project was created, formatted as a human readable string
    /// (e.g. `1:02 PM 3/4/2025`).
    pub fn last_touch_time(&self) -> &str {
        &self.last_touch_time.1
    }

    /// The time the project was created as the raw internal [SystemTime].
    pub fn last_touch_time_raw(&self) -> SystemTime {
        self.last_touch_time.0
    }

    /// The project's [ProjectId].
    pub fn id(&self) -> &ProjectId {
        self.project.cached_info().id()
    }

    /// A string that can be used to change the name of this project.
    pub fn name_change_buffer(&self) -> &str {
        &self.name_change_buffer
    }

    /// A mutable string that can be used to change the name of this project.
    pub fn name_change_buffer_mut(&mut self) -> &mut String {
        &mut self.name_change_buffer
    }

    /// Resets the name change buffer to match the name (see
    /// [Self::name_change_buffer_mut]).
    pub fn reset_name_change_buffer(&mut self) {
        self.name_change_buffer.clear();
        self.name_change_buffer
            .push_str(self.project.cached_info().name());
    }

    /// Whether or not the result of [Self::name_change_buffer] is any different
    /// from the cached name.
    pub fn name_change_buffer_has_changed(&self) -> bool {
        self.name_change_buffer != self.project.cached_info().name()
    }

    /// Whether a prompt to delete this project should be open.
    pub fn delete_promt_open(&self) -> bool {
        self.delete_promt_open
    }

    /// A *mutable* reference to whether a prompt to delete this project should
    /// be open.
    pub fn delete_promt_open_mut(&mut self) -> &mut bool {
        &mut self.delete_promt_open
    }

    /// Whether edits to the project's info should be blocked or not.
    pub fn block_edits(&self) -> bool {
        self.block_edits
    }

    /// A *mutable* reference to whether edits to the project's info should be
    /// blocked or not.
    pub fn block_edits_mut(&mut self) -> &mut bool {
        &mut self.block_edits
    }

    /// Refresh all internal data.
    pub fn refresh(&mut self) -> Result<bool, ProjectError> {
        let info_changed = self.project.refresh()?;
        if info_changed {
            self.reset_name_change_buffer();
        }

        let new_last_touch_time = last_touch_time(&self.project)?;
        let last_touch_time_changed = new_last_touch_time.0 != self.last_touch_time.0;
        self.last_touch_time = new_last_touch_time;

        Ok(info_changed || last_touch_time_changed)
    }

    /// The path to this project's directory.
    pub fn dir_path(&self) -> &Path {
        self.project.dir_path()
    }
}

impl FuzzySearchable for UiProject {
    fn as_search_string(&self) -> &str {
        self.name()
    }
}

fn last_touch_time(project: &Project) -> Result<(SystemTime, String), ProjectError> {
    let last_edited = project.last_edited_with_string()?;
    if let Some(last_edited) = last_edited {
        return Ok(last_edited);
    }
    Ok(project.cached_info().created_with_string())
}
