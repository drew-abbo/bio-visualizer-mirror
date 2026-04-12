use egui;

/// Custom snarl style for the editor area
pub fn snarl_style() -> egui_snarl::ui::SnarlStyle {
    let palette = util::ui::app_palette();

    egui_snarl::ui::SnarlStyle {
        bg_pattern: Some(egui_snarl::ui::BackgroundPattern::grid(
            egui::vec2(36.0, 36.0),
            0.0,
        )),
        bg_pattern_stroke: Some(egui::Stroke::new(1.0, egui::Color32::from_rgb(18, 60, 82))),
        bg_frame: Some(
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(15, 22, 30))
                .inner_margin(0.0),
        ),
        node_frame: Some(
            egui::Frame::new()
                .fill(palette.panel)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::symmetric(14, 8)),
        ),
        header_frame: Some(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(22, 36, 47))
                .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
                .corner_radius(egui::CornerRadius {
                    nw: 12,
                    ne: 12,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(egui::Margin::symmetric(8, 6)),
        ),
        pin_size: Some(12.0),
        pin_placement: Some(egui_snarl::ui::PinPlacement::Edge),
        pin_stroke: Some(egui::Stroke::new(1.0, egui::Color32::from_rgb(20, 24, 27))),
        wire_width: Some(3.2),
        wire_frame_size: Some(52.0),
        wire_smoothness: Some(1.0),
        min_scale: Some(0.5),
        max_scale: Some(2.2),
        select_style: Some(egui_snarl::ui::SelectionStyle {
            margin: egui::Margin::same(4),
            rounding: egui::CornerRadius::same(10),
            fill: egui::Color32::from_rgba_unmultiplied(235, 12, 183, 35),
            stroke: egui::Stroke::new(2.0, palette.accent_primary),
        }),
        ..egui_snarl::ui::SnarlStyle::new()
    }
}
