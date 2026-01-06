//! Contains [SearchableProjects], a wrapper around a sorted and filtered [Vec]
//! of [UiProject]s.

use std::cmp::Ordering;

use util::fuzzy_search::FuzzySearcher;
use util::local_data::project::{Project, ProjectError, ProjectId};

use crate::receiver::ui::ui_project::UiProject;

/// A searchable pool of [UiProject]s.
#[derive(Debug, Default)]
pub struct SearchableProjects {
    projects: Vec<UiProject>,
    searched_project_indices: Vec<usize>,
    searcher: FuzzySearcher,
    search_buffer: String,
}

impl SearchableProjects {
    /// An iterator over the projects, with fuzzy searching applied (see
    /// [Self::search_buffer] and [Self::update_search]). If the search buffer
    /// is empty or all whitespace, every project is returned (sorted by last
    /// touch time then name).
    pub fn searched_projects(&mut self) -> impl Iterator<Item = &mut UiProject> {
        #[cfg(debug_assertions)]
        {
            use std::collections::HashSet;
            let set: HashSet<usize> = self.searched_project_indices.iter().cloned().collect();
            debug_assert_eq!(
                set.len(),
                self.searched_project_indices.len(),
                "All searched project indices must be unique."
            );
        }

        self.searched_project_indices.iter().cloned().map(|i| {
            // SAFETY: We need to do this because Rust can't guarantee we don't
            // already have a reference to the project at index `i`. This is
            // safe because each index in `searched_project_indices` is unique
            // (see debug assertion above).
            unsafe { &mut *self.projects.as_mut_ptr().add(i) }
        })
    }

    /// The number of projects when unfiltered.
    pub fn len(&self) -> usize {
        self.projects.len()
    }

    /// The search buffer. This is the value that is searched by for
    /// [Self::searched_projects] (once [Self::update_search] is called).
    pub fn search_buffer(&self) -> &str {
        &self.search_buffer
    }

    /// A *mutable* reference to the search buffer. This is the value that is
    /// searched by for [Self::searched_projects] (once [Self::update_search] is
    /// called).
    pub fn search_buffer_mut(&mut self) -> &mut String {
        &mut self.search_buffer
    }

    /// Updates the actual search to use what is now in the
    /// [Self::search_buffer], re-searching (this is only done if what is in the
    /// search buffer has actually changed).
    pub fn update_search(&mut self) {
        if self.search_buffer != self.searcher.search_str() {
            self.searcher.set_search_str(&self.search_buffer);
            self.re_sort();
        }
    }

    /// Inserts the project into the
    pub fn insert_project(&mut self, new_project: Project) -> Result<(), ProjectError> {
        let new_project = UiProject::new(new_project)?;
        let insert_idx = self.find_insert_idx(&new_project);
        self.projects.insert(insert_idx, new_project);

        self.re_sort();

        Ok(())
    }

    /// Updates the project with the provided ID, possibly moving it to keep the
    /// list sorted. Whether the project info changed is returned.
    ///
    /// This function will not unblock edits.
    pub fn update_project(&mut self, project_id: &ProjectId) -> Result<bool, ProjectError> {
        self.with_updated_project(project_id, |_| {})
    }

    /// The same as [Self::update_project] but the project will have its edit
    /// capability unblocked.
    pub fn update_edit_blocked_project(
        &mut self,
        project_id: &ProjectId,
    ) -> Result<bool, ProjectError> {
        self.with_updated_project(project_id, |project| {
            if !project.block_edits() {
                util::debug_log_warning!("Unblocking edits but project edits aren't blocked.")
            }
            *project.block_edits_mut() = false;
        })
    }

    /// Removes a project with the provided ID.
    pub fn remove_project(&mut self, project_id: &ProjectId) {
        let Some(old_idx) = self.find_project_idx_from_id(project_id) else {
            util::debug_log_warning!("Failed to find project to remove (ignoring).");
            return;
        };

        self.projects.remove(old_idx);

        self.re_sort();
    }

    fn find_project_idx_from_id(&self, project_id: &ProjectId) -> Option<usize> {
        self.projects
            .iter()
            .position(|project| project.id() == project_id)
    }

    fn find_insert_idx(&self, new_project: &UiProject) -> usize {
        // Sort by time (most recent first), then by name.
        self.projects
            .binary_search_by(|probe| {
                match new_project
                    .last_touch_time_raw()
                    .cmp(&probe.last_touch_time_raw())
                {
                    Ordering::Equal => probe.name().cmp(new_project.name()),
                    other => other,
                }
            })
            .unwrap_or_else(|idx| idx)
    }

    fn re_sort(&mut self) {
        self.searched_project_indices.clear();
        self.searched_project_indices
            .extend(self.searcher.search_indices(&self.projects));
    }

    fn with_updated_project<F>(
        &mut self,
        project_id: &ProjectId,
        f: F,
    ) -> Result<bool, ProjectError>
    where
        F: FnOnce(&mut UiProject),
    {
        let Some(old_idx) = self.find_project_idx_from_id(project_id) else {
            util::debug_log_warning!("Failed to find project to update (ignoring).");
            return Ok(false);
        };

        let project = &mut self.projects[old_idx];

        let changed = project.refresh().inspect_err(|e| {
            util::debug_log_error!("Failed to refresh project: {e}");
        })?;

        f(project);

        if changed {
            let project = self.projects.remove(old_idx);
            self.projects
                .insert(self.find_insert_idx(&project), project);
        }

        self.re_sort();

        Ok(changed)
    }
}
