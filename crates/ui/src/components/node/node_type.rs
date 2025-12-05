use egui::{Id, Pos2, Rect, Vec2};

#[derive(Clone, Debug)]
pub enum NodeType {
    Blur,
    ColorCorrection,
    EdgeDetection,
    ColorGrading,
    Input,
}

impl NodeType {
    pub fn name(&self) -> &str {
        match self {
            NodeType::Blur => "Blur",
            NodeType::ColorCorrection => "Color Correction",
            NodeType::EdgeDetection => "Edge Detection",
            NodeType::ColorGrading => "Color Grading",
            NodeType::Input => "Data Input",
        }
    }

    // pub fn icon(&self) -> &str {
    //     match self {
    //         NodeType::Blur => "💧",
    //         NodeType::ColorCorrection => "🎨",
    //         NodeType::EdgeDetection => "🔍",
    //         NodeType::ColorGrading => "🌈",
    //         NodeType::Input => "📥",
    //     }
    // }

    pub fn color(&self) -> egui::Color32 {
        match self {
            NodeType::Blur => egui::Color32::from_rgb(100, 150, 255),
            NodeType::ColorCorrection => egui::Color32::from_rgb(255, 150, 100),
            NodeType::EdgeDetection => egui::Color32::from_rgb(150, 255, 100),
            NodeType::ColorGrading => egui::Color32::from_rgb(255, 100, 200),
            NodeType::Input => egui::Color32::from_rgb(200, 200, 100),
        }
    }

    pub fn all() -> Vec<NodeType> {
        vec![
            NodeType::Blur,
            NodeType::ColorCorrection,
            NodeType::EdgeDetection,
            NodeType::ColorGrading,
            NodeType::Input,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct PlacedNode {
    pub id: Id,
    pub node_type: NodeType,
    pub position: Pos2,
    pub size: Vec2,
}

impl PlacedNode {
    pub fn new(node_type: NodeType, position: Pos2) -> Self {
        Self {
            id: Id::new(format!("node_{:?}_{}", node_type, egui::Id::new("counter").value())),
            node_type,
            position,
            size: Vec2::new(180.0, 80.0),
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.position, self.size)
    }

    pub fn contains(&self, pos: Pos2) -> bool {
        self.rect().contains(pos)
    }
}