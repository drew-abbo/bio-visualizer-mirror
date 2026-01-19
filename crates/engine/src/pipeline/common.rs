use crate::engine_errors::EngineError;
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
/// Pipelines can process one or more input textures:
/// ```text
/// Primary Input → [Shader Processing] → Output
/// Secondary Inputs ↗     ↑
///                   Parameters (uniforms)
/// ```
///
/// # Rendering Flow
///
/// 1. **Setup**: Create pipeline with shaders and bind group layout
/// 2. **Update**: Pass new parameters via `update_params()`
/// 3. **Render**: Apply effect via `apply()` which:
///    - Updates GPU uniform buffer with parameters
///    - Creates bind group linking input texture(s) + sampler + params
///    - Runs a render pass with a fullscreen triangle
pub trait Pipeline {
    /// Create a new pipeline instance.
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Result<Self, EngineError>
    where
        Self: Sized;

    /// Get the underlying wgpu render pipeline.
    fn pipeline(&self) -> &wgpu::RenderPipeline;

    /// Get the bind group layout defining resource bindings.
    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout;

    /// Get the texture sampler for reading input pixels.
    fn sampler(&self) -> &wgpu::Sampler;

    /// Get the uniform buffer containing effect parameters.
    fn params_buffer(&self) -> &wgpu::Buffer;

    /// Get the pipeline's name for debugging and error messages.
    fn name(&self) -> &str;

    /// Get the expected parameter type name for error messages.
    fn expected_param_type(&self) -> &str;

    /// Number of additional texture inputs this pipeline needs (beyond the primary input).
    ///
    /// # Examples
    ///
    /// - Color grading: 0 (only needs primary input)
    /// - Blend/Overlay: 1 (needs primary + overlay texture)
    /// - Displacement map: 1 (needs primary + displacement texture)
    /// - Multi-layer composite: 2+ (needs primary + multiple overlay layers)
    fn additional_input_count(&self) -> usize {
        0 // Default: no additional inputs needed
    }

    /// Update the GPU uniform buffer with new effect parameters.
    fn update_params(&self, queue: &wgpu::Queue, params: &dyn Any) -> Result<(), EngineError>;

    /// Create a bind group linking input texture(s) to pipeline resources.
    ///
    /// Override this to customize bind group creation for multi-input pipelines.
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device for creating bind groups
    /// * `primary_input` - Main input texture view
    /// * `additional_inputs` - Optional secondary texture views (overlays, masks, etc.)
    ///
    /// # Default Implementation
    ///
    /// The default creates a standard single-input bind group:
    /// - binding 0: sampler
    /// - binding 1: primary input texture
    /// - binding 2: params buffer
    fn bind_group_for(
        &self,
        device: &wgpu::Device,
        primary_input: &wgpu::TextureView,
        additional_inputs: &[&wgpu::TextureView],
    ) -> Result<wgpu::BindGroup, EngineError> {
        // Validate input count
        if additional_inputs.len() != self.additional_input_count() {
            return Err(EngineError::InvalidInputCount {
                expected: self.additional_input_count(),
                actual: additional_inputs.len(),
            });
        }

        // Default implementation for single-input pipelines
        if additional_inputs.is_empty() {
            Ok(self.create_standard_bind_group(device, primary_input, None))
        } else {
            Err(EngineError::UnsupportedOperation(format!(
                "{} has additional inputs but doesn't override bind_group_for()",
                self.name()
            )))
        }
    }

    /// Create a standard bind group for single-input pipelines.
    ///
    /// This is the default implementation used by most simple pipelines.
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

    /// Apply this effect to input texture(s), rendering to an output texture.
    ///
    /// This is the main rendering method that:
    /// 1. Updates uniform buffer with new parameters
    /// 2. Creates bind group linking input texture(s) to shader
    /// 3. Runs a render pass drawing a fullscreen triangle
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device for creating bind groups
    /// * `queue` - GPU queue for buffer updates
    /// * `encoder` - Command encoder to record rendering commands
    /// * `primary_input` - Primary input texture (video frame or previous effect output)
    /// * `additional_inputs` - Optional secondary inputs (overlays, masks, displacement maps, etc.)
    /// * `output` - Output texture to render into
    /// * `params` - Effect parameters (will be downcast to concrete type)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parameter update fails
    /// - Wrong number of additional inputs provided
    /// - Bind group creation fails
    fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        primary_input: &wgpu::TextureView,
        additional_inputs: &[&wgpu::TextureView],
        output: &wgpu::TextureView,
        params: &dyn Any,
    ) -> Result<(), EngineError> {
        // Update parameters
        self.update_params(queue, params)?;

        // Create bind group with all inputs
        let bind_group = self.bind_group_for(device, primary_input, additional_inputs)?;

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
            ..Default::default()
        });

        rpass.set_pipeline(self.pipeline());
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1); // Draw 1 instance of a 3-vertex triangle

        Ok(())
    }
}

/// Creates a nearest-neighbor sampler for pixel-perfect rendering.
pub fn create_nearest_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sampler/nearest"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// Creates a linear (bilinear) sampler for smooth rendering.
pub fn create_linear_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sampler/linear"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// Creates the standard bind group layout used by most single-input effect pipelines.
///
/// # Bindings
///
/// - **Binding 0**: Sampler (filtering sampler)
/// - **Binding 1**: Texture 2D (input texture)
/// - **Binding 2**: Uniform Buffer (effect parameters)
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

pub fn create_empty_params_buffer(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &[0u8; 16], // Minimum size for uniform buffer
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}
