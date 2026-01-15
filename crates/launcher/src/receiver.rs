//! Defines [receiver], the code path that will run when this instance is the
//! main one.

pub mod ui;

mod worker;

use std::process::{Child, ExitCode, ExitStatus};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use util::stop_signals;

use crate::args::{Args, ForcibleFlag};
use crate::other_instances::InstanceLock;
use ui::SavedUiData;
use worker::Worker;

/// The launcher data we want saved on the disk.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PersistedData {
    ui_data: SavedUiData,
}

impl PersistedData {
    /// The UI data.
    pub fn ui_data(&self) -> &SavedUiData {
        &self.ui_data
    }

    /// A *mutable* reference to the UI data.
    pub fn ui_data_mut(&mut self) -> &mut SavedUiData {
        &mut self.ui_data
    }
}

/// The main instance code path.
pub fn receiver(args: Args, mut instance_lock: InstanceLock<PersistedData>) -> ExitCode {
    if let Some(required) = args.send_only {
        return match required {
            ForcibleFlag::Force => {
                eprintln!("There is no main instance to send messages.");
                ExitCode::FAILURE
            }
            ForcibleFlag::True => ExitCode::SUCCESS,
        };
    }

    let editor_cmd = if !args.editor_cmd.is_empty() {
        args.editor_cmd
    } else {
        util::debug_log_warning!(
            "{}",
            concat!(
                "The editor command has no real default right now ",
                "(you should probably provide the `--editor-cmd` flag)."
            )
        );
        vec!["TODO".into()]
    };

    let worker = Worker::new(editor_cmd);
    let exit_plan = ui::run_ui(&mut instance_lock, &worker);

    // Another instance shouldn't be blocked while we're shutting down (waiting
    // for an editor to close may take a while).
    drop(worker);
    drop(instance_lock);

    wait_on_child_processes(&opened_editors(), exit_plan.close_editors);

    exit_plan.exit_code
}

/// A collection of editor processes that have been started over the project's
/// lifetime. Not all processes are guaranteed to still be running.
pub fn opened_editors() -> MutexGuard<'static, Vec<Arc<Mutex<Child>>>> {
    static OPENED_EDITORS: LazyLock<Mutex<Vec<Arc<Mutex<Child>>>>> = LazyLock::new(Mutex::default);
    OPENED_EDITORS
        .lock()
        .expect("No thread should panic with the opened editors mutex.")
}

fn wait_on_child_processes(child_processes: &[Arc<Mutex<Child>>], close_editors: bool) {
    if let Err(e) = stop_signals::passthrough::enable() {
        util::debug_log_error!("Failed to enable stop signal passthrough (ignoring): {e}");
        return;
    }
    const ALREADY_ENABLED: &str = "Shouldn't fail, stop signal passthrough is already enabled.";

    for child in child_processes {
        stop_signals::passthrough::register_child(child.clone()).expect(ALREADY_ENABLED);
    }

    if close_editors {
        stop_signals::passthrough::signal_children_to_stop().expect(ALREADY_ENABLED);
    }

    util::debug_log_info!("Waiting on child processes...");
    for child in child_processes {
        let mut child = child
            .lock()
            .expect("Another thread shouldn't panic with the child locked.");
        wait_on_child(&mut child);
    }
}

fn wait_on_child(child: &mut Child) {
    const MAX_ATTEMPTS: usize = 3;
    for attempt in 1..=MAX_ATTEMPTS {
        match child.wait() {
            Ok(exit_status) => {
                log_child_exit(child, exit_status);
                return;
            }

            Err(e) if attempt < MAX_ATTEMPTS => {
                util::debug_log_error!("Failed to wait on child (try {attempt}, retrying): {e}");
            }
            Err(e) => {
                util::debug_log_error!("Failed to wait on child (try {attempt}, killing): {e}");
            }
        }
    }

    if let Err(e) = child.kill() {
        util::debug_log_error!("Failed to kill child (ignoring): {e}");
    }
}

fn log_child_exit(child: &Child, exit_status: ExitStatus) {
    if exit_status.success() {
        util::debug_log_info!("Child process (id={}) exited successfully.", child.id());
    } else {
        util::debug_log_error!(
            "Child process (id={}) exited unsuccessfully (code={}).",
            child.id(),
            exit_status
                .code()
                .map_or_else(|| "N/A".into(), |code| code.to_string()),
        );
    }
}
