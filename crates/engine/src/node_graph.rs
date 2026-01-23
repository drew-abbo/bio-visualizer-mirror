use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Unique identifier for a node instance in the graph
pub type NodeId = usize;

/// A single instance of a node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInstance {
    /// Unique ID for this instance
    pub id: NodeId,

    /// Name of the node definition this instance is based on
    /// References a Node loaded from node.json
    pub definition_name: String,

    /// Current values for this instance's inputs
    /// Keys are input names from the node definition
    pub input_values: HashMap<String, InputValue>,
}

/// A connection between two nodes in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Source node ID
    pub from_node: NodeId,

    /// Name of the output on the source node
    pub from_output: String,

    /// Destination node ID
    pub to_node: NodeId,

    /// Name of the input on the destination node
    pub to_input: String,
}

/// The node graph - a collection of node instances and their connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGraph {
    /// All node instances in the graph
    instances: HashMap<NodeId, NodeInstance>,

    /// All connections between node instances
    connections: Vec<Connection>,

    /// Counter for generating unique node IDs
    next_id: NodeId,
}

impl NodeGraph {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            connections: Vec::new(),
            next_id: 0,
        }
    }

    /// Add a new node instance to the graph
    pub fn add_instance(&mut self, definition_name: String) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;

        self.instances.insert(
            id,
            NodeInstance {
                id,
                definition_name,
                input_values: HashMap::new(),
            },
        );

        id
    }

    /// Remove a node instance from the graph
    pub fn remove_instance(&mut self, id: NodeId) -> Option<NodeInstance> {
        // Remove all connections to/from this node
        self.connections
            .retain(|conn| conn.from_node != id && conn.to_node != id);

        self.instances.remove(&id)
    }

    /// Connect an output from one node to an input of another node
    pub fn connect(
        &mut self,
        from_node: NodeId,
        output_name: String,
        to_node: NodeId,
        input_name: String,
    ) -> Result<(), GraphError> {
        // Validate nodes exist
        if !self.instances.contains_key(&from_node) {
            return Err(GraphError::NodeNotFound(from_node));
        }
        if !self.instances.contains_key(&to_node) {
            return Err(GraphError::NodeNotFound(to_node));
        }

        // Check for cycles (connecting a node to itself or creating a loop)
        if from_node == to_node {
            return Err(GraphError::SelfConnection);
        }

        // Check if this input is already connected
        if self
            .connections
            .iter()
            .any(|c| c.to_node == to_node && c.to_input == input_name)
        {
            return Err(GraphError::InputAlreadyConnected);
        }

        // Create the connection
        self.connections.push(Connection {
            from_node,
            from_output: output_name.clone(),
            to_node,
            to_input: input_name.clone(),
        });

        // Update the instance's input values to reference this connection
        if let Some(instance) = self.instances.get_mut(&to_node) {
            instance.input_values.insert(
                input_name,
                InputValue::Connection {
                    from_node,
                    output_name,
                },
            );
        }

        Ok(())
    }

    /// Disconnect an input from a node
    pub fn disconnect(&mut self, to_node: NodeId, input_name: &str) -> bool {
        let removed = self
            .connections
            .iter()
            .position(|c| c.to_node == to_node && c.to_input == input_name)
            .map(|idx| self.connections.remove(idx))
            .is_some();

        if removed {
            // Remove the connection reference from the instance
            if let Some(instance) = self.instances.get_mut(&to_node) {
                instance.input_values.remove(input_name);
            }
        }

        removed
    }

    /// Set a direct value for a node's input (not connected to another node)
    pub fn set_input_value(
        &mut self,
        node_id: NodeId,
        input_name: String,
        value: InputValue,
    ) -> Result<(), GraphError> {
        let instance = self
            .instances
            .get_mut(&node_id)
            .ok_or(GraphError::NodeNotFound(node_id))?;

        // Don't allow setting a value if there's a connection
        if matches!(value, InputValue::Connection { .. }) {
            return Err(GraphError::UseConnectMethod);
        }

        instance.input_values.insert(input_name, value);
        Ok(())
    }

    /// Get a node instance
    pub fn get_instance(&self, id: NodeId) -> Option<&NodeInstance> {
        self.instances.get(&id)
    }

    /// Get a mutable node instance
    pub fn get_instance_mut(&mut self, id: NodeId) -> Option<&mut NodeInstance> {
        self.instances.get_mut(&id)
    }

    /// Get all node instances
    pub fn instances(&self) -> &HashMap<NodeId, NodeInstance> {
        &self.instances
    }

    /// Get all connections
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Find all connections where this node is the source
    pub fn outgoing_connections(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.from_node == node_id)
            .collect()
    }

    /// Find all connections where this node is the destination
    pub fn incoming_connections(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.to_node == node_id)
            .collect()
    }

    /// Get the connection feeding into a specific input
    pub fn get_input_connection(&self, node_id: NodeId, input_name: &str) -> Option<&Connection> {
        self.connections
            .iter()
            .find(|c| c.to_node == node_id && c.to_input == input_name)
    }

    /// Check if the graph contains cycles
    pub fn has_cycles(&self) -> bool {
        // Use depth-first search to detect cycles
        let mut visited = HashMap::new();
        let mut rec_stack = HashMap::new();

        for &node_id in self.instances.keys() {
            if self.has_cycle_util(node_id, &mut visited, &mut rec_stack) {
                return true;
            }
        }

        false
    }

    fn has_cycle_util(
        &self,
        node_id: NodeId,
        visited: &mut HashMap<NodeId, bool>,
        rec_stack: &mut HashMap<NodeId, bool>,
    ) -> bool {
        if *rec_stack.get(&node_id).unwrap_or(&false) {
            return true; // Found a cycle
        }

        if *visited.get(&node_id).unwrap_or(&false) {
            return false; // Already visited this node
        }

        visited.insert(node_id, true);
        rec_stack.insert(node_id, true);

        // Check all outgoing connections
        for conn in self.outgoing_connections(node_id) {
            if self.has_cycle_util(conn.to_node, visited, rec_stack) {
                return true;
            }
        }

        rec_stack.insert(node_id, false);
        false
    }

    /// Get execution order using topological sort
    /// Returns nodes in the order they should be executed
    pub fn execution_order(&self) -> Result<Vec<NodeId>, GraphError> {
        if self.has_cycles() {
            return Err(GraphError::CyclicGraph);
        }

        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut order = Vec::new();

        // Initialize in-degrees
        for &node_id in self.instances.keys() {
            in_degree.insert(node_id, 0);
        }

        // Count incoming edges for each node
        for conn in &self.connections {
            *in_degree.get_mut(&conn.to_node).unwrap() += 1;
        }

        // Find all nodes with no incoming edges
        let mut queue: Vec<NodeId> = in_degree
            .iter()
            .filter(|entry| *entry.1 == 0)
            .map(|(id, _)| *id)
            .collect();

        // Process nodes in topological order
        while let Some(node_id) = queue.pop() {
            order.push(node_id);

            // Reduce in-degree for all neighbors
            for conn in self.outgoing_connections(node_id) {
                let degree = in_degree.get_mut(&conn.to_node).unwrap();
                *degree -= 1;

                if *degree == 0 {
                    queue.push(conn.to_node);
                }
            }
        }

        // If we processed all nodes, we have a valid order
        if order.len() == self.instances.len() {
            Ok(order)
        } else {
            Err(GraphError::CyclicGraph)
        }
    }

    /// Find the output node (if any)
    /// Output nodes are typically nodes with no outgoing connections
    pub fn find_output_nodes(&self) -> Vec<NodeId> {
        self.instances
            .keys()
            .filter(|&&id| self.outgoing_connections(id).is_empty())
            .copied()
            .collect()
    }

    /// Clear all nodes and connections
    pub fn clear(&mut self) {
        self.instances.clear();
        self.connections.clear();
        self.next_id = 0;
    }
}

/// The value of a node input - either a direct value or a connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputValue {
    /// Connected to another node's output
    Connection {
        from_node: NodeId,
        output_name: String,
    },

    /// Direct value: Frame (stored as reference, not serialized as actual texture)
    Frame,

    /// Direct value: Boolean
    Bool(bool),

    /// Direct value: Integer
    Int(i32),

    /// Direct value: Float
    Float(f32),

    /// Direct value: Dimensions
    Dimensions { width: u32, height: u32 },

    /// Direct value: Pixel/Color
    Pixel { r: f32, g: f32, b: f32, a: f32 },

    /// Direct value: Text
    Text(String),

    /// Direct value: Enum choice (index into choices array)
    Enum(usize),

    /// Direct value: File path
    File(PathBuf),
}

/// Errors that can occur when working with the node graph
#[derive(Error, Debug, Clone)]
pub enum GraphError {
    /// Node with given ID not found
    #[error("Node {0} not found")]
    NodeNotFound(NodeId),

    /// Attempted to connect a node to itself
    #[error("Cannot connect node to itself")]
    SelfConnection,

    /// The input is already connected to another node
    #[error("Input already connected")]
    InputAlreadyConnected,

    /// The graph contains a cycle
    #[error("Graph contains cycles")]
    CyclicGraph,

    /// Input doesn't exist on the node
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Output doesn't exist on the node
    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    /// Tried to set InputValue::Connection directly (use connect() instead)
    #[error("Use connect() method for connections")]
    UseConnectMethod,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_graph_operations() {
        let mut graph = NodeGraph::new();

        // Add nodes
        let node_a = graph.add_instance("ColorGrading".to_string());
        let node_b = graph.add_instance("Blur".to_string());
        let node_c = graph.add_instance("Output".to_string());

        assert_eq!(graph.instances().len(), 3);

        // Connect: A -> B -> C
        graph
            .connect(node_a, "output".to_string(), node_b, "input".to_string())
            .unwrap();

        graph
            .connect(node_b, "output".to_string(), node_c, "input".to_string())
            .unwrap();

        assert_eq!(graph.connections().len(), 2);

        // Check execution order
        let order = graph.execution_order().unwrap();
        assert_eq!(order, vec![node_a, node_b, node_c]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = NodeGraph::new();

        let node_a = graph.add_instance("Node1".to_string());
        let node_b = graph.add_instance("Node2".to_string());

        // A -> B
        graph
            .connect(node_a, "out".to_string(), node_b, "in".to_string())
            .unwrap();

        // Try to create cycle: B -> A
        graph
            .connect(node_b, "out".to_string(), node_a, "in".to_string())
            .unwrap();

        assert!(graph.has_cycles());
        assert!(graph.execution_order().is_err());
    }

    #[test]
    fn test_input_value_setting() {
        let mut graph = NodeGraph::new();
        let node = graph.add_instance("ColorGrading".to_string());

        // Set some input values
        graph
            .set_input_value(node, "brightness".to_string(), InputValue::Float(1.5))
            .unwrap();

        graph
            .set_input_value(node, "enabled".to_string(), InputValue::Bool(true))
            .unwrap();

        let instance = graph.get_instance(node).unwrap();
        assert_eq!(instance.input_values.len(), 2);
    }

    #[test]
    fn test_node_removal() {
        let mut graph = NodeGraph::new();

        let node_a = graph.add_instance("A".to_string());
        let node_b = graph.add_instance("B".to_string());
        let node_c = graph.add_instance("C".to_string());

        // A -> B -> C
        graph
            .connect(node_a, "out".to_string(), node_b, "in".to_string())
            .unwrap();
        graph
            .connect(node_b, "out".to_string(), node_c, "in".to_string())
            .unwrap();

        // Remove B
        graph.remove_instance(node_b);

        // Connections involving B should be gone
        assert_eq!(graph.connections().len(), 0);
        assert_eq!(graph.instances().len(), 2);
    }
}
