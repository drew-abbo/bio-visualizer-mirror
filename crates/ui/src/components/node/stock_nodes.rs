use super::node_item::StockNodeId;
use super::node_parameters::NodeParameters;

/// Specification for a stock node type
/// Contains all metadata needed to instantiate the node
pub struct StockNodeSpec {
    pub id: StockNodeId,
    pub has_input: bool,
    pub has_output: bool,
    pub default_size: egui::Vec2,
}

impl StockNodeSpec {
    /// Get specification for a stock node
    pub fn for_node(id: &StockNodeId) -> Self {
        match id {
            StockNodeId::Blur => StockNodeSpec {
                id: id.clone(),
                has_input: true,
                has_output: true,
                default_size: egui::Vec2::new(180.0, 80.0),
            },
            StockNodeId::ColorGrading => StockNodeSpec {
                id: id.clone(),
                has_input: true,
                has_output: true,
                default_size: egui::Vec2::new(180.0, 80.0),
            },
            StockNodeId::ColorCorrection => StockNodeSpec {
                id: id.clone(),
                has_input: true,
                has_output: true,
                default_size: egui::Vec2::new(180.0, 80.0),
            },
            StockNodeId::EdgeDetection => StockNodeSpec {
                id: id.clone(),
                has_input: true,
                has_output: true,
                default_size: egui::Vec2::new(180.0, 80.0),
            },
            StockNodeId::DataInput => StockNodeSpec {
                id: id.clone(),
                has_input: false,
                has_output: true,
                default_size: egui::Vec2::new(150.0, 60.0),
            },
            StockNodeId::VideoOutput => StockNodeSpec {
                id: id.clone(),
                has_input: true,
                has_output: false,
                default_size: egui::Vec2::new(150.0, 60.0),
            },
        }
    }
    
    /// Create default parameters for this node type
    pub fn create_default_parameters(&self) -> NodeParameters {
        NodeParameters::default_for_stock(&self.id)
    }
}