pub mod editor;
mod title_bar;

use super::args::Args;
use super::launcher_comm;
use editor::{EditorArea, NodeGraphState};
use util::eframe;
use util::egui;
use util::local_data::project::{Project, ProjectId};
use util::ui::popup_window;

/// This is the main area of the app.
/// Anything you add to this please make sure it is contained within an _area file
/// The app struct should handle as little logic as possible, and should just be responsible for rendering the different areas of the app and passing data between them
pub struct AppArea {
    title_bar: title_bar::TitleBarArea,
    editor_area: EditorArea,
    show_exit_confirmation: bool,
    /// Flag to indicate we're exiting, prevents re-checking for changes
    is_exiting: bool,
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
            show_exit_confirmation: false,
            is_exiting: false,
        }
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
}

impl eframe::App for AppArea {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Check if the user is trying to close the window
        if ctx.input(|i| i.viewport().close_requested()) {
            // Only check for unsaved changes if we're not already exiting
            if !self.is_exiting {
                let state_context = self.editor_area.editor_state_context_mut();
                let has_unsaved_changes =
                    state_context.has_open_project() && state_context.has_unsaved_changes();

                if has_unsaved_changes {
                    // Prevent the close and show confirmation dialog
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.show_exit_confirmation = true;
                }
            }
        }

        // Show exit confirmation popup if requested
        if self.show_exit_confirmation {
            popup_window(ctx, "Unsaved Changes", |ui| {
                ui.label("You have unsaved changes. Do you want to save them?");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save and Exit").clicked() {
                        // Skip notification - launcher refreshes on reopen anyway
                        self.editor_area.save_state(true);
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
        self.editor_area.show(ctx, frame);
    }

    fn persist_egui_memory(&self) -> bool {
        true
    }

    fn on_exit(&mut self, _gl: Option<&util::eframe::glow::Context>) {
        // Auto-save on unexpected exit, but don't notify launcher
        // (notification only works if launcher is running, otherwise spawns unwanted window)
        if !self.is_exiting {
            let has_unsaved_changes = self
                .editor_area
                .editor_state_context_mut()
                .has_unsaved_changes();
            if has_unsaved_changes {
                self.editor_area.save_state(true);
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
