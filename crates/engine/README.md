Engine crate
=============

Brief
-----
The `engine` crate is the core execution engine for node graphs. It resolves node inputs, runs shader-based nodes and built-in handlers (image/video sources), uploads CPU frames to the GPU, and caches intermediate outputs and compiled render pipelines.

Key modules
-----------
- `graph_executor.rs` - `GraphExecutor` executes a `NodeGraph` using definitions from `NodeLibrary`. It maintains caches (`output_cache`, `pipeline_cache`)
- `node/handler` - built-in handlers implementing the `NodeHandler` trait, e.g. `ImageSourceHandler` and `VideoSourceHandler` for loading media and producing `GpuFrame`s.
- `upload_stager` - utilities to upload CPU image/frame data to GPU textures.
- `node_render_pipeline` - dynamic creation of shader pipelines from WGSL shaders.

Usage (short)
-------------
- Create an executor:

  let mut executor = GraphExecutor::new(wgpu::TextureFormat::Rgba8Unorm);

- Run the graph (returns first output node's results):

  let result = executor.execute(&graph, &library, &device, &queue)?;

- Manage caches between runs:

  executor.clear_producer_cache();
  executor.clear_image_cache();
  executor.invalidate_execution_order();

View the `render_testing` crate for examples.

Errors
------
Use `ExecutionError` to inspect and handle runtime failures (e.g. `TextureUploadError`, `VideoFetchError`, `ShaderLoadError`).