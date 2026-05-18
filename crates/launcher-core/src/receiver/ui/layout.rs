//! Contains [layout] which lays out the UI each frame.

use core::f32;
use std::collections::VecDeque;

use egui::text::LayoutJob;
use egui::{
    self, Align, CentralPanel, Color32, Context, CursorIcon, FontId, Frame, Id, Image, ImageSource,
    IntoAtoms, Key, KeyboardShortcut, LayerId, Layout, Margin, Modifiers, Order, Pos2, Rect,
    Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextFormat, TextStyle,
    TopBottomPanel, Ui, Vec2,
};

use util::ui::icons;

use super::ui_manager::{LayoutState, UiAction};
use super::ui_project::UiProject;

/// Draws the main UI for a frame.
pub fn layout(ui: &mut Ui, state: &mut LayoutState<'_>) {
    search_bar(ui, state);

    bottom_panel(ui, |ui| {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let new_project_button = ui.button("New Project");
                if state.new_project_name_buffer.is_some() || new_project_button.clicked() {
                    util::ui::popup_window(ui.ctx(), "New Project", |ui| {
                        new_project_popup(ui, state)
                    });
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(state.stay_open, "Stay Open").on_hover_text(
                    "Whether the launcher should stay open when a project is opened.",
                );
            });
        });
    });

    central_panel(ui, |ui| {
        let mut n = 0;

        for project in state.projects.searched_projects() {
            n += 1;

            ui.separator();

            ui.add_enabled_ui(!project.block_edits(), |ui| {
                let row = project_row(ui, |ui| {
                    ui.columns_const(|[col1, col2]| {
                        col1.allocate_ui_with_layout(
                            Vec2::ZERO,
                            Layout::left_to_right(Align::Center),
                            |ui| project_name_field(ui, state.ui_action_queue, project),
                        );

                        col2.allocate_ui_with_layout(
                            Vec2::ZERO,
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                delete_icon(ui, state.ui_action_queue, project);
                                open_folder_icon(ui, state.ui_action_queue, project);
                                ui.add_space(10.0);
                                ui.label(project.last_touch_time());
                            },
                        );
                    });
                });

                if row.double_clicked() {
                    state
                        .ui_action_queue
                        .push_back(UiAction::OpenProjectEditor(project.id().clone()));

                    if !*state.stay_open {
                        state.ui_action_queue.push_back(UiAction::Close);
                    }
                }
            });
        }

        if n == 0 {
            ui.centered_and_justified(|ui| {
                ui.heading(RichText::new("There's nothing here...").weak());
            });
        } else {
            ui.separator();
        }

        overlay_pill(ui, format!("Showing {n}/{}", state.projects.len()));
    });

    handle_close_shortcut(ui.ctx(), state);
}

fn search_bar(ui: &mut Ui, state: &mut LayoutState) {
    TopBottomPanel::top("top_panel").show(ui.ctx(), |ui| {
        let search_bar = ui.add(
            TextEdit::singleline(state.projects.search_buffer_mut())
                .font(FontId::proportional(18.0))
                .frame(false)
                .desired_width(f32::INFINITY)
                .margin(Margin::symmetric(15, 15))
                .background_color(Color32::TRANSPARENT)
                .hint_text("Search..."),
        );

        if util::ui::shortcut_pressed(ui.ctx(), Modifiers::COMMAND, Key::F) {
            search_bar.request_focus();
        }

        state.projects.update_search();
    });
}

fn new_project_popup(ui: &mut Ui, state: &mut LayoutState<'_>) {
    Frame::new().inner_margin(15.0).show(ui, |ui| {
        let is_first_frame_of_popup = state.new_project_name_buffer.is_none();
        let new_project_name_buffer = state
            .new_project_name_buffer
            .get_or_insert_with(|| "".into());

        let name_input = TextEdit::singleline(new_project_name_buffer)
            .font(TextStyle::Heading)
            .desired_width(f32::INFINITY)
            .background_color(Color32::TRANSPARENT)
            .margin(Margin::symmetric(15, 15))
            .hint_text("Project name...")
            .show(ui);

        if is_first_frame_of_popup {
            name_input.response.request_focus();
        }

        ui.add_space(15.0);
        ui.separator();
        ui.add_space(15.0);

        let confirm_result = confirm_buttons(ui, "Create Project");

        let submitted =
            matches!(confirm_result, Some(true)) || util::ui::key_pressed(ui.ctx(), Key::Enter);
        let cancelled = matches!(confirm_result, Some(false));

        if submitted {
            // We won't allow submission with an empty name.
            if new_project_name_buffer.is_empty() {
                name_input.response.request_focus();
                return;
            }

            state
                .ui_action_queue
                .push_back(UiAction::CreateProjectFromName(
                    state
                        .new_project_name_buffer
                        .take()
                        .expect("The buffer should be present."),
                ));
        } else if cancelled {
            *state.new_project_name_buffer = None;
        }
    });
}

fn bottom_panel(ui: &mut Ui, inner: impl FnOnce(&mut Ui)) {
    TopBottomPanel::bottom("bottom_panel")
        .frame(Frame::new().inner_margin(Margin::symmetric(15, 15)))
        .show(ui.ctx(), inner);
}

fn central_panel(ui: &mut Ui, inner: impl FnOnce(&mut Ui)) {
    CentralPanel::default()
        .frame(Frame::new().inner_margin(Margin::symmetric(10, 0)))
        .show(ui.ctx(), |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                Frame::new()
                    .inner_margin(Margin::symmetric(10, 15))
                    .show(ui, inner);
            });
        });
}

fn project_row(ui: &mut Ui, inner: impl FnOnce(&mut Ui)) -> Response {
    let response = ui
        .allocate_ui((ui.available_width(), 0.0).into(), |ui| {
            Frame::new().inner_margin(10.0).show(ui, |ui| {
                ui.set_width(ui.available_width());

                inner(ui);
            })
        })
        .response
        .interact(Sense::click());

    ui.painter().rect_filled(
        response.rect,
        0.0,
        if response.hovered() {
            Color32::from_white_alpha(10)
        } else {
            egui::Color32::TRANSPARENT
        },
    );

    response
}

fn project_name_field(
    ui: &mut Ui,
    ui_action_queue: &mut VecDeque<UiAction>,
    project: &mut UiProject,
) {
    let name_change_input_width = ui
        .ctx()
        .fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                project.name_change_buffer().into(),
                TextStyle::Heading.resolve(ui.style()),
                Color32::from_gray(180),
            )
        })
        .size()
        .x;

    let name_change_input = TextEdit::singleline(project.name_change_buffer_mut())
        .font(TextStyle::Heading)
        .desired_width(name_change_input_width)
        .background_color(Color32::TRANSPARENT)
        .show(ui);

    // `name_change_input_width` becomes out of date the second
    // its contents change, so we need to re-compute its width.
    if name_change_input.response.changed() {
        ui.ctx().request_discard("Text width must be re-computed.");
    }

    if name_change_input.response.lost_focus() {
        let confirmed = util::ui::key_pressed(ui.ctx(), Key::Enter);

        if confirmed {
            if project.name_change_buffer().is_empty() {
                name_change_input.response.request_focus();
                return;
            }

            if project.name_change_buffer_has_changed() {
                *project.block_edits_mut() = true;
                ui_action_queue.push_back(UiAction::RenameProject(
                    project.id().clone(),
                    project.name_change_buffer().into(),
                ));
            }
        } else {
            project.reset_name_change_buffer();
        }
    }
}

fn delete_icon(ui: &mut Ui, ui_action_queue: &mut VecDeque<UiAction>, project: &mut UiProject) {
    let icon = draw_padded_icon(ui, icons::trash_64x64()).interact(Sense::click());

    if icon.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if icon.clicked() {
        *project.delete_promt_open_mut() = true;
    }
    if !project.delete_promt_open() {
        return;
    }

    util::ui::popup_window(ui.ctx(), "Delete Project", |ui| {
        Frame::new().inner_margin(15.0).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let mut job = LayoutJob::default();

                const QUESTION_TEXT_SIZE: f32 = 18.0;

                job.append(
                    "Are you sure you want to delete\n",
                    0.0,
                    TextFormat {
                        font_id: FontId::proportional(QUESTION_TEXT_SIZE),
                        ..Default::default()
                    },
                );
                job.append(
                    project.name(),
                    0.0,
                    TextFormat {
                        font_id: FontId::proportional(QUESTION_TEXT_SIZE),
                        italics: true,
                        ..Default::default()
                    },
                );
                job.append(
                    "?",
                    4.0,
                    TextFormat {
                        font_id: FontId::proportional(QUESTION_TEXT_SIZE),
                        ..Default::default()
                    },
                );

                ui.label(job);
                ui.add_space(15.0);
                ui.label("This cannot be undone.");
            });

            ui.add_space(15.0);
            ui.separator();
            ui.add_space(15.0);

            let confirm_result = confirm_buttons(ui, "Delete Project");

            if confirm_result.is_none() {
                return;
            }

            *project.delete_promt_open_mut() = false;

            if matches!(confirm_result, Some(true)) {
                *project.block_edits_mut() = true;
                ui_action_queue.push_back(UiAction::DeleteProject(project.id().clone()));
            }
        });
    });
}

fn open_folder_icon(
    ui: &mut Ui,
    ui_action_queue: &mut VecDeque<UiAction>,
    project: &mut UiProject,
) {
    let icon = draw_padded_icon(ui, icons::folder_64x64()).interact(Sense::click());

    if icon.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if icon.clicked()
        && let Err(e) = util::ui::open_folder_in_file_explorer(project.dir_path())
    {
        util::debug_log_error!("Failed to open project in file explorer: {e}");
        ui_action_queue.push_back(UiAction::ShowError("Couldn't open project folder.".into()));
    }
}

fn draw_padded_icon<'a>(ui: &mut Ui, icon: impl Into<ImageSource<'a>>) -> Response {
    Frame::new()
        .outer_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.add(Image::new(icon).fit_to_exact_size((15.0, 15.0).into()))
        })
        .response
}

/// Returns the right-aligned buttons for a confirmation prompt, returning
/// [None] if no action was taken, `true` if the confirm button was pressed, and
/// `false` if the cancel button or escape was pressed.
fn confirm_buttons<'a>(ui: &mut Ui, confirm_text: impl IntoAtoms<'a>) -> Option<bool> {
    let (submit_button, cancel_button) = ui
        .allocate_ui_with_layout(
            (ui.available_width(), 0.0).into(),
            Layout::right_to_left(Align::Center),
            |ui| (ui.button(confirm_text), ui.button("Cancel")),
        )
        .inner;

    let cancelled = || cancel_button.clicked() || util::ui::key_pressed(ui.ctx(), Key::Escape);

    if submit_button.clicked() {
        Some(true)
    } else if cancelled() {
        Some(false)
    } else {
        None
    }
}

fn overlay_pill(ui: &Ui, text: impl Into<String>) {
    let background_color = Color32::from_black_alpha(25);

    let layer = LayerId::new(Order::Background, Id::new("floating_pill"));
    let painter = ui.ctx().layer_painter(layer);

    let galley = ui.ctx().fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            text.into(),
            FontId::proportional(8.0),
            Color32::from_gray(180),
        )
    });

    let text_size = galley.size();
    let padding = Vec2::new(8.0, 4.0);
    let panel_rect = ui.ctx().available_rect();

    // Centered horizontally, slightly above bottom.
    let pos = Pos2::new(
        panel_rect.center().x - (text_size.x + padding.x * 2.0) / 2.0,
        panel_rect.bottom() - text_size.y - padding.y - 10.0,
    );

    let pill_rect = Rect::from_min_size(pos, text_size + padding * 2.0);
    painter.rect(
        pill_rect,
        pill_rect.height() / 2.0,
        background_color,
        Stroke::new(0.0, Color32::TRANSPARENT),
        StrokeKind::Middle,
    );

    painter.galley(
        pill_rect.center() - text_size / 2.0,
        galley,
        background_color,
    );
}

/// Handles `ctrl+w` and `ctrl+q` for closing the window.
fn handle_close_shortcut(ctx: &Context, state: &mut LayoutState) {
    if ctx.input_mut(|i| {
        i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::W))
            || i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Q))
    }) {
        state.ui_action_queue.push_back(UiAction::Close);
    }
}
