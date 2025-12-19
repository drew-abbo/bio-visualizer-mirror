use crate::errors::EngineError;
use std::any::Any;

/// Core trait defining a GPU rendering pipeline for video effects.
///
/// A pipeline encapsulates:
/// - Shader code (vertex + fragment)
/// - GPU state (blend modes, depth testing, etc.)
/// - Resource bindings (textures, uniforms, samplers)
///
/// # Pipeline Architecture
///
/// All pipelines follow a standard structure:
/// ```text
/// Input Texture → [Shader Processing] → Output Texture
///                        ↑
///                   Parameters (uniforms)
/// ```
///
/// # Rendering Flow
///
/// 1. **Setup**: Create pipeline with shaders and bind group layout
/// 2. **Update**: Pass new parameters via `update_params()`
/// 3. **Render**: Apply effect via `apply()` which:
///    - Updates GPU uniform buffer with parameters
///    - Creates bind group linking input texture + sampler + params
///    - Runs a render pass with a fullscreen triangle
///
/// # Standard Bind Group Layout
///
/// Most pipelines use this binding structure:
/// - `binding 0`: Sampler (how to read texture pixels)
/// - `binding 1`: Input texture (the video frame or previous effect)
/// - `binding 2`: Uniform buffer (effect parameters like brightness, blur radius, etc.)
pub trait Pipeline {
    /// Create a new pipeline instance.
    ///
    /// This initializes all GPU resources needed for rendering:
    /// - Compiles shaders
    /// - Creates render pipeline
    /// - Allocates uniform buffers
    /// - Sets up bind group layouts
    ///
    /// # Errors
    ///
    /// Returns an error if shader compilation fails or GPU resources can't be created.
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Result<Self, EngineError>
    where
        Self: Sized;

    /// Get the underlying wgpu render pipeline.
    ///
    /// This is the compiled GPU state that includes:
    /// - Vertex and fragment shaders
    /// - Render state (blending, depth, etc.)
    /// - Vertex layout
    fn pipeline(&self) -> &wgpu::RenderPipeline;

    /// Get the bind group layout defining resource bindings.
    ///
    /// This describes what resources (textures, buffers, samplers) the shader expects
    /// and how they're bound (binding numbers, visibility, types).
    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout;

    /// Get the texture sampler for reading input pixels.
    ///
    /// Samplers control how textures are read:
    /// - Nearest: Sharp, pixel-perfect (good for pixel art)
    /// - Linear: Smooth, interpolated (good for scaling)
    fn sampler(&self) -> &wgpu::Sampler;

    /// Get the uniform buffer containing effect parameters.
    ///
    /// This GPU buffer holds the effect's parameters (like blur radius, color tint)
    /// that get passed to the shader. Updated via `update_params()`.
    fn params_buffer(&self) -> &wgpu::Buffer;

    /// Get the pipeline's name for debugging and error messages.
    fn name(&self) -> &str;

    /// Get the expected parameter type name for error messages.
    fn expected_param_type(&self) -> &str;

    /// Update the GPU uniform buffer with new effect parameters.
    ///
    /// This copies new parameter values from CPU to GPU memory. Implementations should:
    /// 1. Downcast `params` to the expected type
    /// 2. Write the data to the params buffer using `queue.write_buffer()`
    ///
    /// # Arguments
    ///
    /// * `queue` - GPU command queue for buffer writes
    /// * `params` - Effect parameters (must be downcast to concrete type)
    ///
    /// # Errors
    ///
    /// Returns an error if the params can't be downcast to the expected type.
    /// See example in color_grading.rs for reference.
    fn update_params(&self, queue: &wgpu::Queue, params: &dyn Any) -> Result<(), EngineError>;

    /// Create a bind group linking input texture to pipeline resources.
    ///
    /// A bind group is a collection of resources (textures, buffers, samplers) bound together
    /// for a shader to use. This creates the standard binding:
    /// - binding 0: sampler
    /// - binding 1: input texture
    /// - binding 2: params buffer
    ///
    /// Override this if you need custom bind group creation.
    fn bind_group_for(
        &self,
        device: &wgpu::Device,
        tex_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        self.create_standard_bind_group(device, tex_view, None)
    }

    /// Create a standard bind group with optional label.
    ///
    /// This is the default implementation used by most pipelines.
    /// It binds the sampler, input texture, and parameters buffer
    /// to bindings 0, 1, and 2 respectively.
    fn create_standard_bind_group(
        &self,
        device: &wgpu::Device,
        tex_view: &wgpu::TextureView,
        label: Option<&str>,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: self.bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(self.sampler()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer().as_entire_binding(),
                },
            ],
        })
    }

    /// Apply this effect to an input texture, rendering to an output texture.
    ///
    /// This is the main rendering method that:
    /// 1. Updates uniform buffer with new parameters
    /// 2. Creates bind group linking input texture to shader
    /// 3. Runs a render pass drawing a fullscreen triangle
    ///
    /// # How it works
    ///
    /// The pipeline renders a single triangle that covers the entire screen:
    /// ```text
    /// Vertex Shader: Generates fullscreen triangle from vertex ID
    /// Fragment Shader: Samples input texture and applies effect
    /// Output: Writes to output texture
    /// ```
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device for creating bind groups
    /// * `queue` - GPU queue for buffer updates
    /// * `encoder` - Command encoder to record rendering commands
    /// * `input` - Input texture (video frame or previous effect output)
    /// * `output` - Output texture to render into
    /// * `params` - Effect parameters (will be downcast to concrete type)
    ///
    /// # Errors
    ///
    /// Returns an error if parameter update fails or rendering encounters an issue.
    fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        params: &dyn Any,
    ) -> Result<(), EngineError> {
        // Update parameters
        self.update_params(queue, params)?;

        // Create bind group
        let bind_group = self.bind_group_for(device, input);

        // Create render pass
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("effect_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(self.pipeline());
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1); // Draw 1 instance of a 3-vertex triangle

        Ok(())
    }
}

/// Won't be used in our project more than likely but just in case
/// Creates a nearest-neighbor sampler for pixel-perfect rendering.
///
/// **When to use:**
/// - Pixel art or retro aesthetics
/// - When you want sharp, crisp edges
/// - Video playback at native resolution
/// - No scaling or minimal scaling
///
/// **Characteristics:**
/// - No interpolation between pixels
/// - Sharp transitions
/// - Can look blocky when scaled up
/// - Fastest sampling method
///
/// **Filter modes:**
/// - Magnification: Nearest (when zooming in)
/// - Minification: Nearest (when zooming out)
/// - Mipmaps: Nearest
///
/// **Address modes:**
/// - ClampToEdge: Pixels outside texture bounds use edge color
pub fn create_nearest_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sampler/nearest"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// This will be used 99% of the time
/// Creates a linear (bilinear) sampler for smooth rendering.
///
/// **When to use:**
/// - Blur effects
/// - Smooth scaling (up or down)
/// - Anti-aliasing
/// - Photo/video processing
/// - Natural-looking visuals
///
/// **Characteristics:**
/// - Interpolates between neighboring pixels
/// - Smooth gradients and transitions
/// - No jagged edges when scaling
/// - Slightly slower than nearest
///
/// **Filter modes:**
/// - Magnification: Linear (smooth when zooming in)
/// - Minification: Linear (smooth when zooming out)
/// - Mipmaps: Linear (smooth LOD transitions)
///
/// **Address modes:**
/// - ClampToEdge: Pixels outside texture bounds use edge color
pub fn create_linear_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sampler/linear"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// Creates the standard bind group layout used by most effect pipelines.
///
/// This defines the resource binding structure that shaders expect:
///
/// ```wgsl
/// @group(0) @binding(0) var input_sampler: sampler;
/// @group(0) @binding(1) var input_texture: texture_2d<f32>;
/// @group(0) @binding(2) var<uniform> params: EffectParams;
/// ```
///
/// # Bindings
///
/// ## Binding 0: Sampler
/// - **Type**: Filtering sampler (nearest or linear)
/// - **Visibility**: Fragment shader only
/// - **Purpose**: Controls how texture pixels are read
///
/// ## Binding 1: Texture 2D
/// - **Type**: 2D texture with float values
/// - **Visibility**: Fragment shader only
/// - **Purpose**: The input image/video frame to process
/// - **Filterable**: Yes (works with linear sampling)
///
/// ## Binding 2: Uniform Buffer
/// - **Type**: Uniform buffer (read-only from shader)
/// - **Visibility**: Fragment shader only
/// - **Purpose**: Effect parameters (brightness, blur radius, etc.)
/// - **Dynamic offset**: No
///
/// # Arguments
///
/// * `device` - GPU device to create the layout on
/// * `label` - Debug label for GPU debugging tools
///
/// # Returns
///
/// A bind group layout that can be used in pipeline creation.
pub fn create_standard_bind_group_layout(
    device: &wgpu::Device,
    label: &str,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            // binding 0: sampler
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // binding 1: texture_2d<f32>
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
            // binding 2: uniform buffer (params)
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}