use engine::renderer::pipelines::color_grading::ColorGradingParams;

/// This will change dramatically since we want things to be more data-driven/polymorphic
use super::node_item::StockNodeId;

/// Parameters for different node types
#[derive(Clone, Debug)]
pub enum NodeParameters {
    // Stock node parameters
    ColorGrading(ColorGradingParams),
    
    // I/O nodes typically
    None,
}

impl NodeParameters {
    /// Create default parameters for a stock node
    pub fn default_for_stock(node_id: &StockNodeId) -> Self {
        match node_id {
            StockNodeId::ColorGrading => {
                NodeParameters::ColorGrading(ColorGradingParams::default())
            }
            StockNodeId::DataInput | StockNodeId::VideoOutput => {
                NodeParameters::None
            }
            _ => NodeParameters::None, // Other nodes that we don't have params for yet
        }
    }
    
    /// Get color grading params if this is a color grading node
    pub fn as_color_grading_mut(&mut self) -> Option<&mut ColorGradingParams> {
        match self {
            NodeParameters::ColorGrading(params) => Some(params),
            _ => None,
        }
    }
    
    // Add more accessors as needed
}