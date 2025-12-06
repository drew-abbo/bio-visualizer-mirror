use egui::{Id, Pos2, Rect, Vec2};
use super::node_item::{NodeItem, NodeItemId};
use super::node_parameters::NodeParameters;
use super::stock_nodes::StockNodeSpec;

/// A node that has been placed on the blueprint
#[derive(Clone, Debug)]
pub struct PlacedNode {
    pub id: Id,
    pub item_id: NodeItemId,
    pub display_name: String,
    pub color: egui::Color32,
    pub position: Pos2,
    pub size: Vec2,
    pub parameters: NodeParameters,
    pub has_input: bool,
    pub has_output: bool,
}

impl PlacedNode {
    /// Create a new placed node from a NodeItem template
    pub fn from_item(item: &NodeItem, position: Pos2) -> Self {
        match &item.id {
            NodeItemId::Stock(stock_id) => {
                let spec = StockNodeSpec::for_node(stock_id);
                Self {
                    id: Id::new(format!("node_{:?}_{}", stock_id, egui::Id::new("counter").value())),
                    item_id: item.id.clone(),
                    display_name: item.display_name.clone(),
                    color: item.color,
                    position,
                    size: spec.default_size,
                    parameters: spec.create_default_parameters(),
                    has_input: spec.has_input,
                    has_output: spec.has_output,
                }
            }
            NodeItemId::UserMade(name) => {
                // TODO: Load user node spec from storage
                Self {
                    id: Id::new(format!("node_user_{}_{}", name, egui::Id::new("counter").value())),
                    item_id: item.id.clone(),
                    display_name: item.display_name.clone(),
                    color: item.color,
                    position,
                    size: Vec2::new(180.0, 80.0),
                    parameters: NodeParameters::None, // User nodes would define their own params
                    has_input: true,
                    has_output: true,
                }
            }
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.position, self.size)
    }

    pub fn contains(&self, pos: Pos2) -> bool {
        self.rect().contains(pos)
    }
    
    /// Get the display name for this node
    pub fn name(&self) -> &str {
        &self.display_name
    }
}