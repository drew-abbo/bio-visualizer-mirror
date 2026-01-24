//! Node graph types and helpers.
//!
//! Provides [NodeInstance], [Connection], and [NodeGraph] for building and
//! mutating node graphs, plus utilities such as topological sorting to compute
//! execution order.
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Unique identifier for a node instance in the graph
pub type NodeId = usize;

/// A node instance referencing a definition and its input values.
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

/// Directed connection between two node instances.
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

/// In-memory graph used by the executor; supports mutations and topological sort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGraph {
    instances: HashMap<NodeId, NodeInstance>,
    connections: Vec<Connection>,
    next_id: NodeId,
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeGraph {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            connections: Vec::new(),
            next_id: 0,
        }
    }

    /// Add a new node instance and return its [NodeId].
    /// definition_name should match a loaded [crate::node::NodeDefinition] at execution time.
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

    /// Remove a node instance and any connections to/from it.
    pub fn remove_instance(&mut self, id: NodeId) -> Option<NodeInstance> {
        self.connections
            .retain(|conn| conn.from_node != id && conn.to_node != id);

        self.instances.remove(&id)
    }

    /// Connect an output from one node to an input of another node.
    /// Adds a [Connection] to the graph and updates the destination
    /// instance's `input_values` to an [crate::node_graph::InputValue::Connection] referencing
    /// the source node/output.
    pub fn connect(
        &mut self,
        from_node: NodeId,
        output_name: String,
        to_node: NodeId,
        input_name: String,
    ) -> Result<(), GraphError> {
        if !self.instances.contains_key(&from_node) {
            return Err(GraphError::NodeNotFound(from_node));
        }
        if !self.instances.contains_key(&to_node) {
            return Err(GraphError::NodeNotFound(to_node));
        }

        if from_node == to_node {
            return Err(GraphError::SelfConnection);
        }

        if self
            .connections
            .iter()
            .any(|c| c.to_node == to_node && c.to_input == input_name)
        {
            return Err(GraphError::InputAlreadyConnected);
        }

        self.connections.push(Connection {
            from_node,
            from_output: output_name.clone(),
            to_node,
            to_input: input_name.clone(),
        });

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

    pub fn disconnect(&mut self, to_node: NodeId, input_name: &str) -> bool {
        let removed = self
            .connections
            .iter()
            .position(|c| c.to_node == to_node && c.to_input == input_name)
            .map(|idx| self.connections.remove(idx))
            .is_some();

        if removed && let Some(instance) = self.instances.get_mut(&to_node) {
            instance.input_values.remove(input_name);
        }

        removed
    }

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

        if matches!(value, InputValue::Connection { .. }) {
            return Err(GraphError::UseConnectMethod);
        }

        instance.input_values.insert(input_name, value);
        Ok(())
    }

    pub fn get_instance(&self, id: NodeId) -> Option<&NodeInstance> {
        self.instances.get(&id)
    }

    pub fn get_instance_mut(&mut self, id: NodeId) -> Option<&mut NodeInstance> {
        self.instances.get_mut(&id)
    }

    pub fn instances(&self) -> &HashMap<NodeId, NodeInstance> {
        &self.instances
    }

    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Find all connections where `node_id` is the source.
    pub fn outgoing_connections(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.from_node == node_id)
            .collect()
    }

    /// Find all connections where `node_id` is the destination.
    pub fn incoming_connections(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.to_node == node_id)
            .collect()
    }

    /// Get the connection feeding into a specific input, if any.
    pub fn get_input_connection(&self, node_id: NodeId, input_name: &str) -> Option<&Connection> {
        self.connections
            .iter()
            .find(|c| c.to_node == node_id && c.to_input == input_name)
    }

    /// Detect whether the graph contains cycles.
    pub fn has_cycles(&self) -> bool {
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
            return true;
        }

        if *visited.get(&node_id).unwrap_or(&false) {
            return false;
        }

        visited.insert(node_id, true);
        rec_stack.insert(node_id, true);

        for conn in self.outgoing_connections(node_id) {
            if self.has_cycle_util(conn.to_node, visited, rec_stack) {
                return true;
            }
        }

        rec_stack.insert(node_id, false);
        false
    }

    /// Get execution order using topological sort.
    ///
    /// Returns a vector of [NodeId] values ordered so that dependencies appear
    /// before their consumers. If the graph contains a cycle this method
    /// returns [Err(GraphError::CyclicGraph)].
    pub fn execution_order(&self) -> Result<Vec<NodeId>, GraphError> {
        if self.has_cycles() {
            return Err(GraphError::CyclicGraph);
        }

        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut order = Vec::new();

        for &node_id in self.instances.keys() {
            in_degree.insert(node_id, 0);
        }

        for conn in &self.connections {
            *in_degree.get_mut(&conn.to_node).unwrap() += 1;
        }

        let mut queue: Vec<NodeId> = in_degree
            .iter()
            .filter(|entry| *entry.1 == 0)
            .map(|(id, _)| *id)
            .collect();
        while let Some(node_id) = queue.pop() {
            order.push(node_id);
            for conn in self.outgoing_connections(node_id) {
                let degree = in_degree.get_mut(&conn.to_node).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.push(conn.to_node);
                }
            }
        }
        if order.len() == self.instances.len() {
            Ok(order)
        } else {
            Err(GraphError::CyclicGraph)
        }
    }

    /// Find output nodes (nodes with no outgoing connections).
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

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty() && self.connections.is_empty()
    }
}

/// The value of a node input - either a direct value or a connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputValue {
    Connection {
        from_node: NodeId,
        output_name: String,
    },
    Frame,
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions {
        width: u32,
        height: u32,
    },
    Pixel {
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },
    Text(String),
    Enum(usize),
    File(PathBuf),
}

/// Errors that can occur when working with the node graph
#[derive(Error, Debug, Clone)]
pub enum GraphError {
    #[error("Node {0} not found")]
    NodeNotFound(NodeId),

    #[error("Cannot connect node to itself")]
    SelfConnection,

    #[error("Input already connected")]
    InputAlreadyConnected,

    #[error("Graph contains cycles")]
    CyclicGraph,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid output: {0}")]
    InvalidOutput(String),

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
