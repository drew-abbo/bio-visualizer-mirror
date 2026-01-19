use engine::graph_executor::{GraphExecutor, enums::OutputValue};
use engine::node_graph::{InputValue, NodeGraph};
use engine::node_library::errors::LibraryError;
use engine::node_library::NodeLibrary;
use std::path::PathBuf;
use wgpu::TextureFormat;

pub struct RenderTests {
    node_library: NodeLibrary,
    executor: GraphExecutor,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl RenderTests {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target_format: TextureFormat,
    ) -> Result<Self, LibraryError> {
        let executor = GraphExecutor::new(target_format);

        // Get root path of the project
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("failed to find workspace root");

        let nodes_path = workspace_root.join("Nodes");

        println!("Loading nodes from: {:?}", nodes_path);
        let node_library = NodeLibrary::load_from_disk(&nodes_path)?;
        println!("Loaded {} node definitions", node_library.definitions().len());

        Ok(Self {
            node_library,
            executor,
            device,
            queue
        })
    }

    /// Test a simple invert effect using the graph executor
    pub fn test_invert(&mut self) -> Result<wgpu::TextureView, Box<dyn std::error::Error>> {
        println!("Running test_invert...");
        
        // Build the graph
        let mut graph = NodeGraph::new();
        let image = graph.add_instance("Image".to_string());
        let invert = graph.add_instance("Invert".to_string());
        
        graph.set_input_value(
            image,
            "path".to_string(),
            InputValue::File(PathBuf::from("C:\\Users\\Zach\\Downloads\\integrate_jobtread.png")),
        )?;

        // Connect image output to invert input
        graph.connect(
            image, "output".to_string(),
            invert, "input".to_string(),
        )?;

        // Execute the graph - executor handles everything!
        let result = self.executor.execute(
            &graph,
            &self.node_library,
            &self.device,
            &self.queue,
        )?;
        
        // Get the output frame
        let output = result.outputs.get("output")
            .ok_or("No output found")?;
        
        match output {
            OutputValue::Frame(view) => {
                println!("✓ test_invert completed");
                Ok(view.clone())
            }
            _ => Err("Output is not a frame".into())
        }
    }

    /// Test brightness adjustment using the graph executor
    pub fn test_brightness(&mut self) -> Result<wgpu::TextureView, Box<dyn std::error::Error>> {
        println!("Running test_brightness...");
        
        // Build the graph
        let mut graph = NodeGraph::new();
        let brightness = graph.add_instance("Brightness".to_string());
        
        // Set brightness parameter
        graph.set_input_value(
            brightness,
            "brightness".to_string(),
            InputValue::Float(2.0),
        )?;
        
        // Execute - executor handles pipeline creation, parameter conversion, rendering
        let result = self.executor.execute(
            &graph,
            &self.node_library,
            &self.device,
            &self.queue,
        )?;
        
        let output = result.outputs.get("output")
            .ok_or("No output found")?;
        
        match output {
            OutputValue::Frame(view) => {
                println!("✓ test_brightness completed");
                Ok(view.clone())
            }
            _ => Err("Output is not a frame".into())
        }
    }
    
    /// Test a chain: Brightness -> Invert
    pub fn test_effect_chain(&mut self) -> Result<wgpu::TextureView, Box<dyn std::error::Error>> {
        println!("Running test_effect_chain...");
        
        // Build the graph
        let mut graph = NodeGraph::new();
        
        let brightness = graph.add_instance("Brightness".to_string());
        let invert = graph.add_instance("Invert".to_string());
        
        // Set parameters
        graph.set_input_value(
            brightness,
            "brightness".to_string(),
            InputValue::Float(1.5),
        )?;
        
        // Connect them: Brightness -> Invert
        graph.connect(
            brightness, "output".to_string(),
            invert, "input".to_string(),
        )?;
        
        // Execute the whole chain - executor figures out the order and runs it
        let result = self.executor.execute(
            &graph,
            &self.node_library,
            &self.device,
            &self.queue,
        )?;
        
        let output = result.outputs.get("output")
            .ok_or("No output found")?;
        
        match output {
            OutputValue::Frame(view) => {
                println!("✓ test_effect_chain completed");
                Ok(view.clone())
            }
            _ => Err("Output is not a frame".into())
        }
    }

    /// Run all tests
    pub fn run_all_tests(&mut self) -> Option<wgpu::TextureView> {
        println!("\n=== Running All Tests ===\n");
        
        let mut last_result = None;
        
        match self.test_invert() {
            Ok(view) => {
                println!("✓ Invert test passed");
                last_result = Some(view);
            }
            Err(e) => eprintln!("X Invert test failed: {}", e),
        }
        
        match self.test_brightness() {
            Ok(view) => {
                println!("✓ Brightness test passed");
                last_result = Some(view);
            }
            Err(e) => eprintln!("X Brightness test failed: {}", e),
        }
        
        match self.test_effect_chain() {
            Ok(view) => {
                println!("✓ Effect chain test passed");
                last_result = Some(view);
            }
            Err(e) => eprintln!("X Effect chain test failed: {}", e),
        }
        
        println!("\n=== Tests Complete ===\n");
        last_result
    }
}