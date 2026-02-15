use util::egui;

<<<<<<< HEAD
/// Custom snarl style for the editor area
pub fn snarl_style() -> egui_snarl::ui::SnarlStyle {
    egui_snarl::ui::SnarlStyle {
        bg_pattern: Some(egui_snarl::ui::BackgroundPattern::grid(
            egui::vec2(17.0, 17.0),
            0.0,
        )),
        bg_pattern_stroke: Some(egui::Stroke::new(1.0, egui::Color32::from_rgb(0x3E, 0x3A, 0x3C))),
        bg_frame: Some(
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(0x1E, 0x1C, 0x1D))
=======
pub fn snarl_style() -> egui_snarl::ui::SnarlStyle {
    egui_snarl::ui::SnarlStyle {
        bg_pattern: Some(egui_snarl::ui::BackgroundPattern::grid(
            egui::vec2(40.0, 40.0),
            0.0,
        )),
        bg_pattern_stroke: Some(egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 55))),
        bg_frame: Some(
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(30, 30, 35))
>>>>>>> a665ac9 (commit now so I don't screw something up)
                .inner_margin(0.0),
        ),
        select_style: Some(egui_snarl::ui::SelectionStyle {
            margin: egui::Margin::same(4),
            rounding: egui::CornerRadius::same(6),
            fill: egui::Color32::from_rgba_unmultiplied(100, 150, 255, 30),
            stroke: egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
        }),
        ..egui_snarl::ui::SnarlStyle::new()
    }
}
