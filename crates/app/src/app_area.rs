pub mod editor;
mod main_output;
mod title_bar;

use super::args::Args;
use super::launcher_comm;
use editor::{EditorArea, NodeGraphState};
use main_output::MainOutputArea;
use title_bar::Command;
use util::local_data::project::{Project, ProjectId};
use util::ui::popup_window;

/// This is the main area of the app.
/// Anything you add to this please make sure it is contained within an _area file
/// The app struct should handle as little logic as possible, and should just be responsible for rendering the different areas of the app and passing data between them
pub struct AppArea {
    title_bar: title_bar::TitleBarArea,
    editor_area: EditorArea,
    main_output: MainOutputArea,
    show_exit_confirmation: bool,
    /// Flag to indicate we're exiting, prevents re-checking for changes
    is_exiting: bool,
    /// One-time guard to request maximized window after native window exists.
    startup_maximized_requested: bool,
}

impl AppArea {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        let mut editor_area = EditorArea::new();

        // Load project if specified in args (launcher passes ProjectId as string)
        if !args.open_project.is_empty() {
            match ProjectId::try_from(args.open_project.clone())
                .and_then(Project::try_from)
                .and_then(|p| p.open::<NodeGraphState>())
            {
                Ok(project) => {
                    editor_area.editor_state_context_mut().set_project(project);
                    util::debug_log_info!("Successfully opened project: {}", args.open_project);
                }
                Err(e) => {
                    util::debug_log_error!("Failed to open project '{}': {}", args.open_project, e);
                    launcher_comm::notify_project_open_failed();
                }
            }
        }

        Self {
            title_bar: title_bar::TitleBarArea::new(),
            editor_area,
            main_output: MainOutputArea::new(),
            show_exit_confirmation: false,
            is_exiting: false,
            startup_maximized_requested: false,
        }
    }

    fn request_startup_maximized(&mut self, ctx: &egui::Context) {
        if self.startup_maximized_requested {
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        ctx.request_repaint();
        self.startup_maximized_requested = true;
    }

    fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(24, 29, 31))
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ctx, |ui| {
                self.title_bar.ui(ui);
            });
    }

    /// This is for things that are not in the app area but still need things in the app area.
    /// Like the save button needing access to the editor area to trigger saves.
    fn process_pending_commands(&mut self) {
        let commands = self.title_bar.toolbar_mut().drain_pending();

        for command in commands {
            match command {
                Command::SaveProject => {
                    util::debug_log_info!("Saving project");
                    self.editor_area.save_state();
                }
            }
        }
    }

    fn handle_exit(&mut self, ctx: &egui::Context) {
        if !self.is_exiting {
            // essentially, if there are unsaved changes, we want to show a confirmation dialog.
            // however, if the only unsaved changes are viewport changes, we can just save those and exit without confirmation
            let (has_unsaved_changes, only_view_unsaved_changes) = {
                let state_context = self.editor_area.editor_state_context_mut();
                let has_unsaved =
                    state_context.has_open_project() && state_context.has_unsaved_changes();
                let only_view_unsaved =
                    has_unsaved && state_context.has_only_view_unsaved_changes();
                (has_unsaved, only_view_unsaved)
            };

            if has_unsaved_changes {
                if only_view_unsaved_changes {
                    // Persist viewport-only changes without user interruption.
                    self.editor_area.save_state();
                    self.is_exiting = true;
                } else {
                    // Prevent the close and show confirmation dialog
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.show_exit_confirmation = true;
                }
            }
        }
    }
}

impl eframe::App for AppArea {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.request_startup_maximized(ctx);
        self.process_pending_commands();

        // Check if the user is trying to close the window
        if ctx.input(|i| i.viewport().close_requested()) {
            // Only check for unsaved changes if we're not already exiting
            self.handle_exit(ctx);
        }

        // Show exit confirmation popup if requested
        // could move this into its own function but it is pretty self contained I think
        if self.show_exit_confirmation {
            popup_window(ctx, "Unsaved Changes", |ui| {
                ui.label("You have unsaved changes. Do you want to save them?");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save and Exit").clicked() {
                        self.editor_area.save_state();
                        self.is_exiting = true;
                        self.show_exit_confirmation = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Discard and Exit").clicked() {
                        self.is_exiting = true;
                        self.show_exit_confirmation = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_exit_confirmation = false;
                    }
                });
            });
        }

        self.show_top_bar(ctx);
        self.main_output.show(ctx);
        self.editor_area
            .show_with_main_output(ctx, frame, &mut self.main_output);
    }

    fn persist_egui_memory(&self) -> bool {
        true
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // auto save on unexpected exits
        if !self.is_exiting {
            let has_unsaved_changes = self
                .editor_area
                .editor_state_context_mut()
                .has_unsaved_changes();
            if has_unsaved_changes {
                self.editor_area.save_state();
            }
        }

        self.editor_area
            .editor_state_context_mut()
            .close_project()
            .unwrap_or_else(|e| {
                util::debug_log_error!("Failed to close project on exit: {}", e);
            });
    }
}
