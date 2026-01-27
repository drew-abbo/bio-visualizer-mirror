//! Contains [run_ui], which starts up the launcher's UI.

mod layout;
mod searchable_projects;
mod ui_manager;
mod ui_project;

use std::process::ExitCode;

use serde::{Deserialize, Serialize};

use eframe::NativeOptions;
use egui::{Vec2, ViewportBuilder};

use util::version;

use super::PersistedData;
use super::worker::Worker;
use crate::other_instances::InstanceLock;
use ui_manager::UiManager;

/// The state the launcher's UI wants to persist across saves.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SavedUiData {
    #[serde(default = "SavedUiData::default_window_size")]
    pub window_size: Vec2,
    #[serde(default = "SavedUiData::default_zoom_factor")]
    pub zoom_factor: f32,
    #[serde(default)]
    pub last_search: String,
    #[serde(default)]
    pub stay_open: bool,
}

impl SavedUiData {
    #[inline(always)]
    const fn default_window_size() -> Vec2 {
        ui_manager::DEFAULT_WINDOW_SIZE
    }

    #[inline(always)]
    const fn default_zoom_factor() -> f32 {
        1.0
    }
}

impl Default for SavedUiData {
    fn default() -> Self {
        Self {
            window_size: ui_manager::DEFAULT_WINDOW_SIZE,
            zoom_factor: 1.0,
            last_search: String::default(),
            stay_open: false,
        }
    }
}

#[derive(Debug)]
pub struct ExitPlan {
    pub exit_code: ExitCode,
    pub close_editors: bool,
}

/// Starts up the launcher UI, normally exiting when the UI is closed.
///
/// This function can only be run from the main thread.
pub fn run_ui(instance_lock: &mut InstanceLock<PersistedData>, worker: &Worker) -> ExitPlan {
    let mut close_editors_on_exit = false;
    let ui_manager = UiManager::new(worker, instance_lock, &mut close_editors_on_exit);

    let window_title = String::from(version::APP_NAME) + " - Launcher";

    let exit_code = eframe::run_native(
        &window_title,
        NativeOptions {
            viewport: ViewportBuilder::default()
                // TODO: add .with_icon(...)
                .with_title(&window_title)
                .with_min_inner_size(ui_manager::MIN_WINDOW_SIZE)
                .with_inner_size(
                    ui_manager::MIN_WINDOW_SIZE
                        .max(ui_manager.ui_data().window_size * ui_manager.ui_data().zoom_factor),
                ),

            // TODO: Instead of always centering it, maybe we could remember the
            // last position? From the little investigation I've done here, it
            // looks like it'll be a pain or completely not impossible with the
            // way egui/eframe currently works. Still could be worth checking...
            centered: true,

            ..Default::default()
        },
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(ui_manager))
        }),
    )
    .map_or_else(
        |e| {
            util::debug_log_error!("UI (run native) failed: {e}");
            ExitCode::FAILURE
        },
        |_| ExitCode::SUCCESS,
    );

    ExitPlan {
        exit_code,
        close_editors: close_editors_on_exit,
    }
}
