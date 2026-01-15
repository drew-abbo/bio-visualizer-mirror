//! Contains [worker], the function that will run on the worker thread.

mod helpers;

use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::time::Duration;

use egui::Context;

use thiserror::Error;

use util::channels::message_channel::{self, Inbox, Outbox};
use util::channels::request_channel::{self, Client, Server};
use util::drop_join_thread::{self, DropJoinHandle};
use util::local_data::project::{Project, ProjectError, ProjectId};
use util::stop_signals;

use crate::other_instances::{OIMsg, OIMsgReceiver};
use helpers::*;

/// The background worker that listens for messages from other instances and
/// scans for project changes.
///
/// This thread will normally live until the worker inbox drops it's connection
/// (which happens when the object is dropped), but it can exit early in case of
/// a fatal error. The thread won't stop early without at least *trying* to send
/// a [WorkerMsg::FatalError] message.
#[derive(Debug)]
pub struct Worker {
    // Do not change the field order here. The inbox has to be dropped before
    // the thread, otherwise we'll deadlock:
    // https://doc.rust-lang.org/reference/destructors.html#:~:text=The%20fields%20of%20a%20struct%20are%20dropped%20in%20declaration%20order.
    inbox: Inbox<WorkerMsg>,
    client: Client<WorkerTask, WorkerTaskResult>,
    _thread: DropJoinHandle<()>,
}

impl Worker {
    /// Create a new worker.
    pub fn new(editor_cmd: Vec<String>) -> Self {
        let (frontend_inbox, worker_outbox) = message_channel::new::<WorkerMsg>();

        let (worker_server, frontend_client) =
            request_channel::new::<WorkerTask, WorkerTaskResult>();

        let thread = drop_join_thread::spawn(|| {
            worker(editor_cmd, worker_outbox, worker_server);
        });

        Self {
            inbox: frontend_inbox,
            client: frontend_client,
            _thread: thread,
        }
    }

    /// Access an inbox for messages from the worker.
    pub fn inbox(&self) -> &Inbox<WorkerMsg> {
        &self.inbox
    }

    /// Access the client for requesting work to be done by the worker.
    pub fn client(&self) -> &Client<WorkerTask, WorkerTaskResult> {
        &self.client
    }
}

/// A message from the worker thread to the frontend.
#[derive(Debug)]
pub enum WorkerMsg {
    /// The worker thread encountered an error it couldn't recover from.
    FatalError(String),

    /// An acknowledgement from the worker thread indicating that it has
    /// completed all of its set up work.
    SetupComplete,

    /// Indicates that the UI thread should stop, closing the UI.
    Close,

    /// The same as [Close](WorkerMsg::Close) but also indicating that all
    /// editors should be sent stop signals.
    CloseAll,

    /// Another instance was launched and is now exiting. Focus the current
    /// instance to indicate this.
    Focus,

    /// A new project directory appeared.
    ProjectAppeared(Project),

    /// An existing project was modified in some way.
    ProjectChanged(ProjectId),

    /// An existing project directory disappeared.
    ProjectDisappeared(ProjectId),

    /// An editor treied to open a project but failed.
    ProjectOpenFailed,
}

impl TryFrom<OIMsg> for WorkerMsg {
    type Error = ();

    fn try_from(msg: OIMsg) -> Result<Self, Self::Error> {
        match msg {
            // Relay these messages directly:
            OIMsg::Focus => Ok(Self::Focus),
            OIMsg::ProjectOpenFailed => Ok(Self::ProjectOpenFailed),
            OIMsg::Close => Ok(Self::Close),

            // Don't relay these messages:
            OIMsg::ProjectUpdated => Err(()),
        }
    }
}

/// A task that the worker can be requested to do.
#[derive(Debug, Clone)]
pub enum WorkerTask {
    OpenProjectEditor(ProjectId),
    CreateProjectFromName(String),
    DeleteProject(ProjectId),
    RenameProject(ProjectId, String),
    UseUiContext(Context),
}

/// Info relating to a [WorkerTask] that has finished.
#[derive(Debug)]
pub enum WorkerTaskDone {
    NoInfo,
    ProjectCreated(Project),
    ProjectRenamed(ProjectId),
    ProjectDeleted(ProjectId),
}

/// The result of a [WorkerTask]. See [WorkerTaskError].
pub type WorkerTaskResult = Result<WorkerTaskDone, WorkerTaskError>;

/// Indicates that something went wrong doing a worker task.
#[derive(Error, Debug)]
pub enum WorkerTaskError {
    #[error("Failed to run editor: {0}")]
    FailedToRunEditor(#[from] io::Error),
    #[error(transparent)]
    ProjectError(#[from] ProjectError),
}

/// How long to pause after each work iteration.
const DELAY_BETWEEN_WORK_ITERATIONS: Duration = Duration::from_millis(250);

/// The max (ish) amount of time we can go without re-scanning projects.
const PROJECTS_RESCAN_INTERVAL: Duration = Duration::from_secs(20);

#[derive(Debug)]
struct WorkerData<'a> {
    outbox: &'a Outbox<WorkerMsg>,
    server: &'a Server<WorkerTask, WorkerTaskResult>,
    oi_msg_receiver: OIMsgReceiver,
    known_projects: HashMap<ProjectHashedById, ProjectKnownState>,
    editor_cmd: Vec<String>,
    ui_context: Option<Context>,
}

impl<'a> WorkerData<'a> {
    /// Sends a message to the outbox and requests a UI re-draw.
    pub fn send_outbox_msg(&self, msg: WorkerMsg) -> Result<(), StopWorkReason> {
        if self.outbox.send(msg).is_err() {
            return Err(StopWorkReason::ConnectionDropped);
        }
        self.request_ui_redraw();
        Ok(())
    }

    /// Requests a UI re-draw.
    pub fn request_ui_redraw(&self) {
        if let Some(ui_context) = self.ui_context.as_ref() {
            ui_context.request_repaint();
        }
    }
}

/// A type that cannot be constructed.
type Impossible = Infallible;

fn worker(
    editor_cmd: Vec<String>,
    worker_outbox: Outbox<WorkerMsg>,
    worker_server: Server<WorkerTask, WorkerTaskResult>,
) {
    let stop_work_reason = worker_inner(editor_cmd, &worker_outbox, &worker_server);

    match stop_work_reason.expect_err("Ok return value is impossible here.") {
        StopWorkReason::ConnectionDropped => {}
        StopWorkReason::FatalError(err_msg) => {
            _ = worker_outbox.send(WorkerMsg::FatalError(err_msg));
        }
    };
}

fn worker_inner(
    editor_cmd: Vec<String>,
    worker_outbox: &Outbox<WorkerMsg>,
    worker_server: &Server<WorkerTask, WorkerTaskResult>,
) -> Result<Impossible, StopWorkReason> {
    let oi_msg_receiver = match OIMsgReceiver::new() {
        Ok(oi_msg_receiver) => oi_msg_receiver,
        Err(e) => {
            util::debug_log_error!("Failed to create other instance message sender: {e}");
            return Err(StopWorkReason::FatalError(
                "Failed to set up receiver for messages from other instances.".into(),
            ));
        }
    };

    let mut worker_data = WorkerData {
        outbox: worker_outbox,
        server: worker_server,
        oi_msg_receiver,
        known_projects: HashMap::default(),
        editor_cmd,
        ui_context: None,
    };

    const ITERATIONS_BETWEEN_RESCANS: usize = (PROJECTS_RESCAN_INTERVAL.as_secs_f64()
        / DELAY_BETWEEN_WORK_ITERATIONS.as_secs_f64())
    .round() as usize
        - 1;

    scan_for_project_changes(&mut worker_data)?;
    worker_data.send_outbox_msg(WorkerMsg::SetupComplete)?;
    let mut iterations_since_rescan = 0;

    while worker_outbox.connection_open() {
        if stop_signals::polling::consume() {
            util::debug_log_info!("Stop signal received, relaying to the UI.");
            worker_data.send_outbox_msg(WorkerMsg::CloseAll)?;
        }

        let early_rescan_required = handle_oi_msgs(&mut worker_data)?;

        // If a re-scan is required, we want to respond as soon as possible (so
        // we won't wait for UI inputs). We won't wait after the rescan either,
        // since re-scanning may take a while (and we don't want to keep other
        // instances waiting).
        if early_rescan_required {
            handle_pending_worker_server_requests(&mut worker_data)?;
        } else {
            handle_worker_server_requests_for_time(
                &mut worker_data,
                DELAY_BETWEEN_WORK_ITERATIONS,
            )?;
        };

        if !early_rescan_required && iterations_since_rescan < ITERATIONS_BETWEEN_RESCANS {
            iterations_since_rescan += 1;
            continue;
        }
        iterations_since_rescan = 0;

        scan_for_project_changes(&mut worker_data)?;
    }

    Err(StopWorkReason::ConnectionDropped)
}
