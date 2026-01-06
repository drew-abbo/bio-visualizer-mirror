use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, Vec2, gui_zoom};

const ZOOM_IN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Equals);
const ZOOM_OUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Minus);

const DEFAULT_WINDOW_SIZE: Vec2 = Vec2::new(600.0, 400.0);

pub fn ui() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(DEFAULT_WINDOW_SIZE),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Project Selector",
        options,
        Box::new(|_| Ok(Box::<ProjectSelector>::default())),
    )
    .expect("The UI shouldn't fail to initialize.");

    struct ProjectSelector {
        name: String,
        age: u32,
    }

    impl Default for ProjectSelector {
        fn default() -> Self {
            Self {
                name: "Arthur".to_owned(),
                age: 42,
            }
        }
    }

    impl eframe::App for ProjectSelector {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("My egui Application");
                ui.horizontal(|ui| {
                    let name_label = ui.label("Your name: ");
                    ui.text_edit_singleline(&mut self.name)
                        .labelled_by(name_label.id);
                });
                ui.add(
                    egui::Slider::new(&mut self.age, 0..=120)
                        .text("age")
                        .show_value(false),
                );
                if ui.button("Increment").clicked() {
                    self.age += 1;
                }
                ui.label(format!("Hello '{}', age {}", self.name, self.age));

                if ui.ctx().input_mut(|i| i.consume_shortcut(&ZOOM_IN)) {
                    gui_zoom::zoom_in(ctx);
                }

                if ui.ctx().input_mut(|i| i.consume_shortcut(&ZOOM_OUT)) {
                    gui_zoom::zoom_out(ctx);
                }
            });
        }
    }
}
