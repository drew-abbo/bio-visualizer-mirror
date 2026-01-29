/// REMOVE THIS CRATE ONCE THE UI IS IN A GOOD STATE
use engine::node_graph::{InputValue, NodeGraph};
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use engine::graph_executor::GraphExecutor;
use engine::graph_executor::OutputValue;
use engine::node::NodeLibrary;

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    state: Option<RenderState>,
    last_output: Option<wgpu::TextureView>,

    target_fps: f32,
    next_frame_time: Option<std::time::Instant>,

    video_graph: Option<NodeGraph>,
    executor: Option<GraphExecutor>,
    node_library: Option<NodeLibrary>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
}

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    blit_pipeline: BlitPipeline,
}

struct BlitPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl BlitPipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(BLIT_SHADER)),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            ..Default::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("blit_encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        queue.submit(Some(encoder.finish()));
    }
}

const BLIT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((vid << 1u) & 2u);
    let y = f32(vid & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@group(0) @binding(0) var tex_sampler: sampler;
@group(0) @binding(1) var tex: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, in.uv);
}
"#;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let format = surface.get_capabilities(&adapter).formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        let blit_pipeline = BlitPipeline::new(&device, format);

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
        let nodes_path = workspace_root.join("Nodes");

        let node_library = NodeLibrary::load_from_disk(&nodes_path).unwrap();
        let executor = GraphExecutor::new(format);

        let mut graph = NodeGraph::new();
        let video = graph.add_instance("Video".to_string());
        let image = graph.add_instance("Image".to_string());

        let invert = graph.add_instance("Invert".to_string());
        // let brightness = graph.add_instance("Brightness".to_string());

        let overlay = graph.add_instance("Overlay".to_string());
        graph
            .set_input_value(overlay, "opacity".to_string(), InputValue::Float(0.7))
            .unwrap();

        graph
            .set_input_value(
                video,
                "path".to_string(),
                InputValue::File(PathBuf::from("C:\\Users\\Zach\\Downloads\\rick.mp4")),
            )
            .unwrap();

        graph
            .set_input_value(
                image,
                "path".to_string(),
                InputValue::File(PathBuf::from(
                    "C:\\Users\\Zach\\Downloads\\backgroundtest.jpg",
                )),
            )
            .unwrap();

        graph
            .connect(image, "output".to_string(), invert, "input".to_string())
            .unwrap();

        // graph
        //     .connect(
        //         invert,
        //         "output".to_string(),
        //         overlay,
        //         "background".to_string(),
        //     )
        //     .unwrap();

        graph
            .connect(
                invert,
                "output".to_string(),
                overlay,
                "background".to_string(),
            )
            .unwrap();

        graph
            .connect(
                video,
                "output".to_string(),
                overlay,
                "foreground".to_string(),
            )
            .unwrap();

        self.target_fps = 30.0;
        let frame_interval = std::time::Duration::from_secs_f32(1.0 / self.target_fps);
        let now = std::time::Instant::now();
        self.next_frame_time = Some(now + frame_interval);

        self.video_graph = Some(graph);
        self.executor = Some(executor);
        self.node_library = Some(node_library);
        self.device = Some(device.clone());
        self.queue = Some(queue.clone());

        event_loop.set_control_flow(ControlFlow::WaitUntil(now + frame_interval));

        self.state = Some(RenderState {
            surface,
            device,
            queue,
            config,
            blit_pipeline,
        });

        self.window = Some(window);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let winit::event::StartCause::ResumeTimeReached { .. } = cause {
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            let frame_interval = std::time::Duration::from_secs_f32(1.0 / self.target_fps);

            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + frame_interval,
            ));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                if let Some(state) = self.state.as_mut() {
                    state.config.width = size.width;
                    state.config.height = size.height;
                    state.surface.configure(&state.device, &state.config);
                }
            }

            WindowEvent::RedrawRequested => {
                let state = self.state.as_mut().unwrap();

                if let (Some(graph), Some(exec), Some(lib), Some(dev), Some(q)) = (
                    self.video_graph.as_mut(),
                    self.executor.as_mut(),
                    self.node_library.as_ref(),
                    self.device.as_ref(),
                    self.queue.as_ref(),
                ) {
                    let result = exec.execute(graph, lib, dev, q).unwrap();
                    if let Some(OutputValue::Frame(frame)) = result.outputs.get("output") {
                        self.last_output = Some(frame.view().clone());
                    }
                }

                let frame = match state.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("Surface error: {:?}", e);
                        return;
                    }
                };

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                if let Some(output) = &self.last_output {
                    state
                        .blit_pipeline
                        .render(&state.device, &state.queue, output, &view);
                }

                frame.present();
            }

            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            _ => {}
        }
    }
}
