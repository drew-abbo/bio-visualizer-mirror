use crate::components::node::NodeParameters;

pub struct NodePropertyWindow;

impl NodePropertyWindow {
    pub fn show(ui: &mut egui::Ui, params: &mut NodeParameters) {
        ui.heading("Node Properties");

        match params {
            NodeParameters::ColorGrading(p) => {
                ui.label("Color Grading");
                ui.add(egui::Slider::new(&mut p.exposure, -5.0..=5.0).text("Exposure"));
                ui.add(egui::Slider::new(&mut p.contrast, 0.0..=2.0).text("Contrast"));
                ui.add(egui::Slider::new(&mut p.saturation, 0.0..=3.0).text("Saturation"));
                // etc
            }

            NodeParameters::None => {
                ui.label("No parameters");
            }
        }
    }
}