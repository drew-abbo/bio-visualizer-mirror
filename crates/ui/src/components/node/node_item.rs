/// Represents a node template in the selection list
/// This is what users drag from the left panel
#[derive(Clone, Debug)]
pub struct NodeItem {
    pub id: NodeItemId,
    pub display_name: String,
    pub color: egui::Color32,
    pub category: NodeCategory,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeItemId {
    Stock(StockNodeId),
    UserMade(String), // User-defined node by name
}

#[derive(Clone, Debug, PartialEq)]
pub enum StockNodeId {
    // Effects
    Blur,
    ColorGrading,
    ColorCorrection,
    EdgeDetection,
    
    // I/O
    DataInput,
    VideoOutput,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeCategory {
    Effect,
    IO,
    UserMade,
}

impl NodeItem {
    /// Get all available stock nodes
    pub fn stock_nodes() -> Vec<NodeItem> {
        vec![
            // Effects
            NodeItem {
                id: NodeItemId::Stock(StockNodeId::Blur),
                display_name: "Blur".to_string(),
                color: egui::Color32::from_rgb(100, 150, 255),
                category: NodeCategory::Effect,
            },
            NodeItem {
                id: NodeItemId::Stock(StockNodeId::ColorGrading),
                display_name: "Color Grading".to_string(),
                color: egui::Color32::from_rgb(255, 100, 200),
                category: NodeCategory::Effect,
            },
            NodeItem {
                id: NodeItemId::Stock(StockNodeId::ColorCorrection),
                display_name: "Color Correction".to_string(),
                color: egui::Color32::from_rgb(255, 150, 100),
                category: NodeCategory::Effect,
            },
            NodeItem {
                id: NodeItemId::Stock(StockNodeId::EdgeDetection),
                display_name: "Edge Detection".to_string(),
                color: egui::Color32::from_rgb(150, 255, 100),
                category: NodeCategory::Effect,
            },
            
            // I/O
            NodeItem {
                id: NodeItemId::Stock(StockNodeId::DataInput),
                display_name: "Data Input".to_string(),
                color: egui::Color32::from_rgb(200, 200, 100),
                category: NodeCategory::IO,
            },
            NodeItem {
                id: NodeItemId::Stock(StockNodeId::VideoOutput),
                display_name: "Video Output".to_string(),
                color: egui::Color32::from_rgb(150, 100, 255),
                category: NodeCategory::IO,
            },
        ]
    }
    
    /// Get all available nodes (stock + user-made)
    pub fn all_nodes() -> Vec<NodeItem> {
        let nodes = Self::stock_nodes();
        
        // TODO: Load user-made nodes from storage/config
        // nodes.extend(Self::load_user_nodes());
        
        nodes
    }
}