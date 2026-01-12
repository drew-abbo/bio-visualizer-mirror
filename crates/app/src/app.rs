use crate::ui::{menu_buttons::menu_bar::MenuBar, node_blueprint::NodeBlueprint, View};

pub struct App {
    menu_bar: MenuBar,
    node_blueprint: NodeBlueprint,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_styles(&cc.egui_ctx);
        Self {
            menu_bar: MenuBar,
            node_blueprint: NodeBlueprint::new(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu_bar.ui(ui);
        });
        
        // Add a left side panel
        egui::SidePanel::left("left_panel")
            .default_width(250.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Tools");
                ui.separator();
                
                //left panel content here
                ui.label("Node Library");
            });
        
        // Blueprint takes the remaining space
        egui::CentralPanel::default().show(ctx, |ui| {
            self.node_blueprint.ui(ui);
        });
    }
}

fn configure_styles(ctx: &egui::Context) {
    use egui::{Color32, Visuals};

    let mut visuals = Visuals::dark();

    // Custom blue theme
    // visuals.window_fill = Color32::from_rgb(24, 29, 31);

    // Main background
    visuals.panel_fill = Color32::from_rgb(24, 29, 31);

    // Text edits, scroll bars and other things
    visuals.extreme_bg_color = Color32::from_rgb(33, 54, 33);

    // visuals.selection.bg_fill = Color32::from_rgb(40, 100, 180);
    // visuals.widgets.active.bg_fill = Color32::from_rgb(50, 110, 190);
    // visuals.widgets.hovered.bg_fill = Color32::from_rgb(45, 105, 185);

    // Rounded corners
    // visuals.window_corner_radius = CornerRadius::same(8);
    // visuals.widgets. = CornerRadius::same(4);

    ctx.set_visuals(visuals);
}
