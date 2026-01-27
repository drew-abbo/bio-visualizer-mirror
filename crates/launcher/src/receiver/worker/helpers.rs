//! Contains helper utilities for the parent module.

use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use egui::Context;

use thiserror::Error;

use util::channels::ChannelError;
use util::channels::request_channel::ReqRes;
use util::local_data::project::{self, Project, ProjectHeader, ProjectId, ProjectInfo};

use super::{WorkerData, WorkerMsg, WorkerTask, WorkerTaskDone, WorkerTaskResult};
use crate::other_instances::OIMsg;
use crate::receiver;

/// A reason why the worker has stopped.
#[derive(Error, Debug)]
pub enum StopWorkReason {
    #[error("An unrecoverable error was encountered: {0}")]
    FatalError(String),
    #[error("The other end of the connection was dropped.")]
    ConnectionDropped,
}

/// A wrapper around [Project] that re-implements [Hash] and [Eq] to *only*
/// depend on the project info's [ProjectId].
#[derive(Debug)]
pub struct ProjectHashedById(Project);

impl From<ProjectHashedById> for Project {
    fn from(project: ProjectHashedById) -> Self {
        project.0
    }
}

impl From<Project> for ProjectHashedById {
    fn from(project: Project) -> Self {
        ProjectHashedById(project)
    }
}

impl Deref for ProjectHashedById {
    type Target = Project;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ProjectHashedById {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Borrow<ProjectId> for ProjectHashedById {
    fn borrow(&self) -> &ProjectId {
        self.0.cached_info().id()
    }
}

impl PartialEq for ProjectHashedById {
    fn eq(&self, other: &Self) -> bool {
        self.0.cached_info().id() == other.0.cached_info().id()
    }
}

impl Eq for ProjectHashedById {}

impl Hash for ProjectHashedById {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.cached_info().id().hash(state);
    }
}

/// Used to keep track of what we know about a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectKnownState {
    NotNew,
    New,
    Missing,
}

/// Relays all relevant messages to the frontend. `true` is returned if the
/// projects should be re-scanned.
pub fn handle_oi_msgs(worker_data: &mut WorkerData) -> Result<bool, StopWorkReason> {
    let mut rescan_required = false;

    while let Some(msg) = worker_data.oi_msg_receiver.receive().map_err(|e| {
        util::debug_log_error!("Failed to receive message from other instance: {e}");
        StopWorkReason::FatalError("Failed to receive message from other instance.".into())
    })? {
        util::debug_log_info!("Other instance message received: `{msg}`.");

        // If it can be converted to a message for the frontend, we should send
        // it to the frontend.
        if let Ok(msg) = msg.try_into() {
            worker_data
                .send_outbox_msg(msg)
                .map_err(|_| StopWorkReason::ConnectionDropped)?;
        }

        rescan_required = rescan_required
            && match msg {
                OIMsg::Focus => false,
                OIMsg::Close => false,

                OIMsg::ProjectUpdated => true,
                OIMsg::ProjectOpenFailed => true,
            };
    }

    Ok(rescan_required)
}

/// Handles any requests from the frontend that are being waited on.
pub fn handle_pending_worker_server_requests(
    worker_data: &mut WorkerData,
) -> Result<(), StopWorkReason> {
    match worker_data.server.check_all() {
        Ok(Some(requests)) => respond_to_worker_server_requests(worker_data, requests),
        Ok(None) => Ok(()),
        Err(_) => Err(StopWorkReason::ConnectionDropped),
    }
}

/// Listens for and handles requests from the frontend as they come in for
/// around `timeout` time.
pub fn handle_worker_server_requests_for_time(
    worker_data: &mut WorkerData,
    mut timeout: Duration,
) -> Result<(), StopWorkReason> {
    let start_time = SystemTime::now();
    while timeout > Duration::ZERO {
        match worker_data.server.wait_timeout_all(timeout) {
            Ok(requests) => respond_to_worker_server_requests(worker_data, requests)?,
            Err(ChannelError::Timeout { .. }) => return Ok(()),
            Err(_) => return Err(StopWorkReason::ConnectionDropped),
        };

        timeout = timeout
            .checked_sub(elapsed_time(start_time))
            .unwrap_or(Duration::ZERO);
    }

    Ok(())
}

/// Scans for any changes in the project folder, alerting the frontend if there
/// are changes.
pub fn scan_for_project_changes(worker_data: &mut WorkerData) -> Result<(), StopWorkReason> {
    update_known_projects(worker_data, project_ids_on_disk()?);
    refresh_projects(worker_data)
}

fn respond_to_worker_server_requests(
    worker_data: &mut WorkerData,
    requests: VecDeque<ReqRes<WorkerTask, WorkerTaskResult>>,
) -> Result<(), StopWorkReason> {
    for (req, res) in requests {
        let response: WorkerTaskResult = match req {
            WorkerTask::OpenProjectEditor(project_id) => {
                open_project_editor(worker_data, project_id)
            }
            WorkerTask::CreateProjectFromName(name) => {
                create_project_from_name(&mut worker_data.known_projects, name)
            }
            WorkerTask::DeleteProject(project_id) => {
                delete_project(&mut worker_data.known_projects, project_id)
            }
            WorkerTask::RenameProject(project_id, name) => rename_project(project_id, name),
            WorkerTask::UseUiContext(ui_context) => {
                use_ui_context(worker_data, ui_context);
                continue; // No reply.
            }
        };

        if let Some(res) = res {
            res.respond(response)
                .map_err(|_| StopWorkReason::ConnectionDropped)?;
            worker_data.request_ui_redraw();
        };
    }

    Ok(())
}

fn open_project_editor(worker_data: &WorkerData, project_id: ProjectId) -> WorkerTaskResult {
    if util::debug_log::enabled() {
        let mut cmd_str = worker_data.editor_cmd.join(" ");
        cmd_str.push(' ');
        cmd_str.push_str(&project_id.as_ref().to_string_lossy());
        util::debug_log_info!("Running command: {cmd_str}");
    }

    let child_process = Command::new(&worker_data.editor_cmd[0])
        .args(&worker_data.editor_cmd[1..])
        .arg(project_id)
        .spawn()
        .inspect_err(|e| util::debug_log_error!("Failed to launch editor (ignoring): {e}"))?;

    receiver::opened_editors().push(Arc::new(Mutex::new(child_process)));

    Ok(WorkerTaskDone::NoInfo)
}

fn create_project_from_name(
    known_projects: &mut HashMap<ProjectHashedById, ProjectKnownState>,
    name: String,
) -> WorkerTaskResult {
    let new_project = ProjectInfo::new(name).create_project()?;
    known_projects.insert(
        ProjectHashedById(new_project.try_clone()?),
        ProjectKnownState::Missing,
    );
    Ok(WorkerTaskDone::ProjectCreated(new_project))
}

fn delete_project(
    known_projects: &mut HashMap<ProjectHashedById, ProjectKnownState>,
    project_id: ProjectId,
) -> WorkerTaskResult {
    Project::load(&project_id)?.delete()?;
    known_projects.remove(&project_id);
    Ok(WorkerTaskDone::ProjectDeleted(project_id))
}

fn rename_project(project_id: ProjectId, new_name: String) -> WorkerTaskResult {
    Project::load(&project_id)?.with_info_mut(|info| {
        *info.name_mut() = new_name;
    })?;
    Ok(WorkerTaskDone::ProjectRenamed(project_id))
}

fn use_ui_context(worker_data: &mut WorkerData, ui_context: Context) {
    if worker_data.ui_context.is_some() {
        util::debug_log_error!("Worker got multiple `UseUIContext` requests (ignoring).");
    }

    worker_data.ui_context = Some(ui_context);
}

fn elapsed_time(start_time: SystemTime) -> Duration {
    SystemTime::now()
        .duration_since(start_time)
        .unwrap_or(Duration::ZERO)
}

fn project_ids_on_disk() -> Result<impl Iterator<Item = ProjectId>, StopWorkReason> {
    Ok(match project::iter_projects() {
        Ok(d) => d,
        Err(e) => {
            util::debug_log_error!("Failed to create project iterator: {e}");
            return Err(StopWorkReason::FatalError(
                "Failed to read projects directory.".into(),
            ));
        }
    }
    .filter_map(|id_or_err| {
        id_or_err
            .inspect_err(|e| {
                util::debug_log_error!("Failed to get project iterator entry (ignoring): {e}");
            })
            .ok()
    }))
}

/// Updates the [ProjectKnownState] of all known projects based on the
/// `project_ids` iterator.
fn update_known_projects(
    worker_data: &mut WorkerData,
    project_ids: impl Iterator<Item = ProjectId>,
) {
    for project_id in project_ids {
        if let Some(entry) = worker_data.known_projects.get_mut(&project_id) {
            *entry = ProjectKnownState::NotNew;
            continue;
        }

        let header = match project_id.load_header() {
            Ok(header) => header,
            Err(e) => {
                util::debug_log_error!(
                    "Failed to load header from project iterator entry (ignoring): {e}"
                );
                continue;
            }
        };

        worker_data
            .known_projects
            .insert(ProjectHashedById(header), ProjectKnownState::New);
    }
}

/// Refreshes the known projects based on the [ProjectKnownState], notifying the
/// frontend of any changes as it goes.
fn refresh_projects(worker_data: &mut WorkerData) -> Result<(), StopWorkReason> {
    // We're going to put our new findings in `known_projects_updated`, reading
    // from `worker_data.known_projects`, but we can't access `worker_data`
    // mutably if we call `into_iter`/`drain` on `worker_data.known_projects`,
    // so we have to pull the known projects out (replacing it with an empty
    // hash map that won't allocate). This is fine since `refresh_project`
    // doesn't care about the `known_projects` field. Hack fix!
    let mut known_projects_updated = HashMap::with_capacity(worker_data.known_projects.len());
    let known_projects = mem::take(&mut worker_data.known_projects);

    let mut ret = Ok(());

    known_projects_updated.extend(
        known_projects
            .into_iter()
            .map_while(|(mut project, known_state)| {
                // Refresh all projects, notifying the frontend of changes.

                let possible_msg = refresh_project(&mut project, known_state);

                if let Some(msg) = possible_msg {
                    // Stop iteration if the frontend drops.
                    if worker_data.send_outbox_msg(msg).is_err() {
                        ret = Err(StopWorkReason::ConnectionDropped);
                        return None;
                    }
                }

                Some((project, known_state))
            })
            .filter_map(|(project, known_state)| match known_state {
                // Mark all projects as missing for the next iteration.
                ProjectKnownState::NotNew | ProjectKnownState::New => {
                    Some((project, ProjectKnownState::Missing))
                }

                // We don't keep missing projects for next time.
                ProjectKnownState::Missing => None,
            }),
    );

    worker_data.known_projects = known_projects_updated;

    ret
}

fn refresh_project(project: &mut Project, known_state: ProjectKnownState) -> Option<WorkerMsg> {
    match known_state {
        ProjectKnownState::NotNew => match project.refresh() {
            Ok(changed) if changed => Some(WorkerMsg::ProjectChanged(
                project.cached_info().id().clone(),
            )),
            Ok(_) => None,
            Err(e) => {
                util::debug_log_error!("Failed to refresh project info (ignoring): {e}");
                None
            }
        },

        ProjectKnownState::New => project
            .try_clone()
            .map(WorkerMsg::ProjectAppeared)
            .inspect_err(|e| {
                util::debug_log_error!("Couldn't clone project header (ignoring): {e}");
                util::debug_log_warning!("Not notifying frontend of new project.");
            })
            .ok(),

        ProjectKnownState::Missing => Some(WorkerMsg::ProjectDisappeared(
            project.cached_info().id().clone(),
        )),
    }
}
