//! Contains [UiManager] which is used to manage everything UI related except
//! the UI's actual layout and style.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::time::{self, Duration, SystemTime};

use eframe::{App, Frame};
use egui::{CentralPanel, Context, Ui, UserAttentionType, Vec2, ViewportCommand, Visuals};

use util::channels::request_channel::Request;
use util::local_data::project::ProjectId;
use util::ui::{ErrorPopup, icons};

use super::layout;
use super::searchable_projects::SearchableProjects;
use super::{PersistedData, SavedUiData};
use crate::other_instances::InstanceLock;
use crate::receiver::worker::{
    Worker, WorkerMsg, WorkerTask, WorkerTaskDone, WorkerTaskError, WorkerTaskResult,
};

/// The default size of the UI's window. When the UI has never been opened, this
/// will be its size.
pub const DEFAULT_WINDOW_SIZE: Vec2 = Vec2::new(700.0, 500.0);

/// The minimum size of the UI's window. It will not be able to shrink smaller
/// than this.
pub const MIN_WINDOW_SIZE: Vec2 = Vec2::new(700.0, 500.0);

/// An object for managing the UI's data and state.
#[derive(Debug)]
pub struct UiManager<'a> {
    worker: &'a Worker,
    projects: SearchableProjects,
    error_popup_queue: VecDeque<String>,
    waiting_requests: VecDeque<Request<WorkerTaskResult>>,
    answered_requests: VecDeque<WorkerTaskResult>,
    last_save_worked: bool,
    last_save_timestamp: SystemTime,
    unsaved_ui_data: SavedUiData,
    instance_lock: &'a mut InstanceLock<PersistedData>,
    worker_setup_complete: bool,
    is_1st_update: bool,
    new_project_name_buffer: Option<String>,
    close_editors_on_exit: &'a mut bool,
}

impl<'a> UiManager<'a> {
    /// Create a UI manager.
    ///
    /// This will not start the UI.
    pub fn new(
        worker: &'a Worker,
        instance_lock: &'a mut InstanceLock<PersistedData>,
        close_editors_on_exit: &'a mut bool,
    ) -> Self {
        let unsaved_ui_data = instance_lock.data().ui_data().clone();

        let mut projects = SearchableProjects::default();
        *projects.search_buffer_mut() = unsaved_ui_data.last_search.clone();

        Self {
            worker,
            projects,
            error_popup_queue: VecDeque::default(),
            waiting_requests: VecDeque::default(),
            answered_requests: VecDeque::default(),
            last_save_worked: true,
            last_save_timestamp: time::UNIX_EPOCH,
            unsaved_ui_data,
            instance_lock,
            worker_setup_complete: false,
            is_1st_update: true,
            new_project_name_buffer: None,
            close_editors_on_exit,
        }
    }

    /// The internal [SavedUiData] data that is being stored.
    pub fn ui_data(&self) -> &SavedUiData {
        &self.unsaved_ui_data
    }

    /// How often to save the UI configuration.
    const SAVE_INTERVAL: Duration = Duration::from_secs(10);

    /// Calls [Self::save_ui_data] if it has been at least [Self::SAVE_INTERVAL]
    /// since the last save.
    ///
    /// Make sure to call [Self::update_unsaved_ui_data] first.
    fn save_ui_data_on_interval(&mut self) {
        let now = SystemTime::now();
        if now
            .duration_since(self.last_save_timestamp)
            .unwrap_or(Duration::ZERO)
            < Self::SAVE_INTERVAL
        {
            return;
        }

        self.last_save_timestamp = now;
        self.save_ui_data();
    }

    /// Tries to save the UI data to disk, returning whether or not the
    /// operation worked.
    ///
    /// The disk will not be written to if the data hasn't changed and the last
    /// save worked. Also see [Self::force_save_ui_data].
    ///
    /// If an error occurs, an internal flag is set.
    ///
    /// Make sure to call [Self::update_unsaved_ui_data] first.
    fn save_ui_data(&mut self) {
        if !self.last_save_worked || self.unsaved_ui_data != *self.instance_lock.data().ui_data() {
            self.force_save_ui_data()
        }
    }

    /// The same as [Self::save_ui_data] except it will *always* actually write
    /// the data to disk, regardless of whether anything has changed or not.
    ///
    /// Make sure to call [Self::update_unsaved_ui_data] first.
    fn force_save_ui_data(&mut self) {
        self.last_save_worked = self
            .instance_lock
            .with_data(|data| {
                *data.ui_data_mut() = self.unsaved_ui_data.clone();
            })
            .inspect_err(|e| {
                util::debug_log_error!("Failed to save instance lock file (ignoring): {e}");
            })
            .is_ok();

        self.last_save_timestamp = SystemTime::now();
    }

    /// To be called before [Self::save_ui_data] so that the data being saved is
    /// as up to date as possible. This function will never actually write to
    /// disk.
    fn update_unsaved_ui_data(&mut self, ctx: &Context) {
        self.unsaved_ui_data.window_size = ctx.content_rect().size();
        self.unsaved_ui_data.zoom_factor = ctx.zoom_factor();

        self.unsaved_ui_data.last_search.clear();
        self.unsaved_ui_data
            .last_search
            .push_str(self.projects.search_buffer());
    }

    fn handle_worker_msgs(&mut self, ui_action_queue: &mut VecDeque<UiAction>) {
        let msgs = match self.worker.inbox().check_all() {
            Ok(Some(msgs)) => msgs,
            Ok(None) => return,
            Err(e) => {
                panic!("Worker dropped msg channel without sending a fatal error first: {e}");
            }
        };

        for msg in msgs {
            match msg {
                WorkerMsg::FatalError(e) => util::eprintln_and_exit!("Fatal error: {e}"),

                WorkerMsg::SetupComplete => self.worker_setup_complete = true,

                WorkerMsg::Close => ui_action_queue.push_back(UiAction::Close),
                WorkerMsg::CloseAll => {
                    *self.close_editors_on_exit = true;
                    ui_action_queue.push_back(UiAction::Close);
                }

                WorkerMsg::Focus => ui_action_queue.push_back(UiAction::Focus),

                WorkerMsg::ProjectAppeared(project) => {
                    self.projects.insert_project(project).unwrap_or_else(|e| {
                        util::debug_log_error!("Failed to insert a project for the UI: {e}");
                        ui_action_queue.push_back(UiAction::ShowError(GENERIC_ERROR_MSG.into()));
                    });
                }

                WorkerMsg::ProjectChanged(project_id) => {
                    self.projects
                        .update_project(&project_id)
                        .map(|_| ())
                        .unwrap_or_else(|e| {
                            util::debug_log_error!("Failed to update a project for the UI: {e}");
                            ui_action_queue
                                .push_back(UiAction::ShowError(GENERIC_ERROR_MSG.into()));
                        });
                }

                WorkerMsg::ProjectDisappeared(project_id) => {
                    self.projects.remove_project(&project_id)
                }

                WorkerMsg::ProjectOpenFailed => {
                    util::debug_log_error!("Failed to open project.");
                    ui_action_queue
                        .push_back(UiAction::ShowError("Failed to open project.".into()));
                }
            }
        }
    }

    fn handle_worker_task_responses(&mut self, ui_action_queue: &mut VecDeque<UiAction>) {
        self.waiting_requests
            .retain_mut(|request| match request.check_non_blocking() {
                Ok(Some(worker_result)) => {
                    self.answered_requests.push_back(worker_result);
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    // If this happens we're probably about to panic/crash, but
                    // we'll ignore it until we get sent a fatal error.
                    util::debug_log_error!("Worker request not responded to (ignoring): {e}");
                    false
                }
            });

        for answer in self.answered_requests.drain(..) {
            let task_done = match answer {
                Ok(task_done) => task_done,
                Err(e) => {
                    util::debug_log_error!("A worker task failed (alerting): {e}");

                    let err_msg = match e {
                        WorkerTaskError::FailedToRunEditor(_) => "Failed to launch editor.",
                        WorkerTaskError::ProjectError(_) => GENERIC_ERROR_MSG,
                    }
                    .into();
                    ui_action_queue.push_back(UiAction::ShowError(err_msg));

                    continue;
                }
            };

            match task_done {
                WorkerTaskDone::NoInfo => {}

                WorkerTaskDone::ProjectCreated(project) => {
                    self.projects.insert_project(project).unwrap_or_else(|e| {
                        util::debug_log_error!("Failed to insert a project for the UI: {e}");
                        ui_action_queue.push_back(UiAction::ShowError(GENERIC_ERROR_MSG.into()));
                    });
                }

                WorkerTaskDone::ProjectRenamed(project_id) => {
                    if let Err(e) = self.projects.update_edit_blocked_project(&project_id) {
                        util::debug_log_error!("Failed to update a project for the UI: {e}");
                        ui_action_queue.push_back(UiAction::ShowError(GENERIC_ERROR_MSG.into()));
                    }
                }

                WorkerTaskDone::ProjectDeleted(project_id) => {
                    self.projects.remove_project(&project_id);
                }
            }
        }
    }

    fn handle_ui_action(&mut self, ctx: &Context, action: UiAction) {
        let task = match action {
            UiAction::CreateProjectFromName(project_name) => {
                WorkerTask::CreateProjectFromName(project_name)
            }
            UiAction::RenameProject(project_id, project_name) => {
                WorkerTask::RenameProject(project_id.clone(), project_name)
            }
            UiAction::DeleteProject(project_id) => WorkerTask::DeleteProject(project_id.clone()),
            UiAction::OpenProjectEditor(project_id) => {
                WorkerTask::OpenProjectEditor(project_id.clone())
            }

            UiAction::ShowError(err_msg) => {
                self.error_popup_queue.push_back(err_msg);
                return;
            }

            UiAction::Close => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
                return;
            }

            UiAction::Focus => {
                ctx.send_viewport_cmd(ViewportCommand::RequestUserAttention(
                    UserAttentionType::Informational,
                ));
                return;
            }
        };

        let request = match self.worker.client().request(task) {
            Ok(request) => request,
            Err(e) => {
                // If this happens we're probably about to panic/crash, but
                // we'll ignore it until we get sent a fatal error.
                util::debug_log_error!("Worker request not sent (ignoring): {e}");
                return;
            }
        };
        self.waiting_requests.push_back(request);
    }

    fn handle_1st_update(&mut self, ctx: &Context) {
        ctx.set_zoom_factor(self.unsaved_ui_data.zoom_factor);
        ctx.request_discard("First frame shouldn't be drawn since we just changed the zoom.");

        _ = self
            .worker
            .client()
            .alert(WorkerTask::UseUiContext(ctx.clone()))
            .inspect_err(|e| {
                util::debug_log_error!("Failed to send UI context to worker (ignoring): {e}");
            });

        self.is_1st_update = false;
    }
}

impl<'a> ErrorPopup<String> for UiManager<'a> {
    fn error_queue_mut(&mut self) -> &mut VecDeque<String> {
        &mut self.error_popup_queue
    }
}

impl<'a> App for UiManager<'a> {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        let icons_are_preloaded =
            util::ui::bytes_images_are_loaded(ctx, [icons::trash_64x64(), icons::folder_64x64()])
                .expect("Images shouldn't fail to load.");

        if self.is_1st_update {
            // `handle_1st_update` discards the current draw calculation so we
            // can skip a bunch of work if we return after calling it.
            self.handle_1st_update(ctx);
            return;
        }

        let mut ui_action_queue = VecDeque::default();

        self.handle_worker_msgs(&mut ui_action_queue);
        self.handle_worker_task_responses(&mut ui_action_queue);

        apply_base_style(ctx);

        CentralPanel::default().show(ctx, |ui| {
            // We'll show a loading screen while we wait for a few things:
            //  - The worker should be set up, otherwise we'll see an empty or
            //    partial list of projects for a split second.
            //  - The icons should be pre-loaded, otherwise they won't show as
            //    actual images immediately.
            if !self.worker_setup_complete || !icons_are_preloaded {
                loading_screen(ui);
                if self.worker_setup_complete {
                    ctx.request_repaint();
                }
                return;
            }

            layout::layout(
                ui,
                &mut LayoutState {
                    projects: &mut self.projects,
                    ui_action_queue: &mut ui_action_queue,
                    new_project_name_buffer: &mut self.new_project_name_buffer,
                    stay_open: &mut self.unsaved_ui_data.stay_open,
                },
            );
        });

        for action in ui_action_queue.drain(..) {
            self.handle_ui_action(ctx, action);
        }

        self.update_unsaved_ui_data(ctx);
        self.save_ui_data_on_interval();

        self.show_any_error_popups(ctx);

        util::ui::handle_zoom_shortcuts(ctx, ZOOM_LIMITS.0, ZOOM_LIMITS.1);
        util::ui::windows_scroll_fix(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_ui_data();

        util::debug_log_info!("Closing launcher UI...");
    }
}

/// An action the user performed that should be responded to.
#[derive(Debug)]
pub enum UiAction {
    OpenProjectEditor(ProjectId),
    CreateProjectFromName(String),
    RenameProject(ProjectId, String),
    DeleteProject(ProjectId),
    ShowError(String),
    Close,
    Focus,
}

/// Data needed for the main UI's layout.
#[derive(Debug)]
pub struct LayoutState<'a> {
    pub projects: &'a mut SearchableProjects,
    pub ui_action_queue: &'a mut VecDeque<UiAction>,
    pub new_project_name_buffer: &'a mut Option<String>,
    pub stay_open: &'a mut bool,
}

const ZOOM_LIMITS: (f32, f32) = (0.5, 2.0);

const GENERIC_ERROR_MSG: &str = "Something went wrong, consider restarting.";

fn apply_base_style(ctx: &Context) {
    // Disable light mode, always use dark.
    ctx.set_visuals(Visuals::dark());

    ctx.style_mut(|style| {
        // Add some padding to the inside of buttons.
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
    });
}

fn loading_screen(ui: &mut Ui) {
    ui.centered_and_justified(|ui| ui.label("Loading..."));
}
