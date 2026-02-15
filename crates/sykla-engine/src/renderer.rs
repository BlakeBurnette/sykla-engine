use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUniform};
use crate::cyclist::CyclistInstanceData;
use crate::grass_blades::GrassBladeInstance;
use crate::mesh::{GpuMesh, Vertex};
use crate::post_process::PostProcessUniforms;
use crate::skeleton::{MAX_CYCLISTS, NUM_BONES};
use crate::sky::SkyUniforms;
use crate::vegetation::{InstanceData, TreeInstanceData};
use crate::wildlife::AnimatedInstanceData;

const SHADOW_MAP_SIZE: u32 = 4096;
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightUniform {
    pub direction: [f32; 4],
    pub color: [f32; 4],
    pub ambient: [f32; 4],
    pub fog_color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub mid_color: [f32; 4],
    pub high_color: [f32; 4],
    pub zone_params: [f32; 4],
    pub terrain_params: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BloomUniforms {
    pub texel_size: [f32; 2],
    pub direction: [f32; 2],
}

pub struct DrawCall {
    pub mesh: GpuMesh,
    pub material_bind_group: wgpu::BindGroup,
}

pub struct InstancedDrawCall {
    pub mesh: GpuMesh,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
    pub material_bind_group: wgpu::BindGroup,
}

pub struct AnimatedDrawCall {
    pub mesh: GpuMesh,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
    pub material_bind_group: wgpu::BindGroup,
}

pub struct GrassShellDraw {
    pub mesh: GpuMesh,
    pub material_bind_group: wgpu::BindGroup,
    pub num_shells: u32,
}

pub struct GrassBladeDrawCall {
    pub mesh: GpuMesh,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
    pub material_bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CyclistPipeline {
    Solid,
    Textured,
    Skin,
}

pub struct CyclistMeshPart {
    pub mesh: GpuMesh,
    pub material_bind_group: wgpu::BindGroup,
    pub pipeline: CyclistPipeline,
}

pub struct CyclistModel {
    pub parts: Vec<CyclistMeshPart>,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
}

pub struct TreeMeshPart {
    pub mesh: GpuMesh,
    pub material_bind_group: wgpu::BindGroup,
}

pub struct TreeModel {
    pub parts: Vec<TreeMeshPart>,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    // Main pass pipelines
    pipeline: wgpu::RenderPipeline,
    pipeline_road: wgpu::RenderPipeline,
    pipeline_grass_shell: wgpu::RenderPipeline,
    pipeline_grass_blade: wgpu::RenderPipeline,
    pipeline_instanced: wgpu::RenderPipeline,
    pipeline_animated: wgpu::RenderPipeline,
    pipeline_cyclist: wgpu::RenderPipeline,
    pipeline_textured_cyclist: wgpu::RenderPipeline,
    pipeline_skin_cyclist: wgpu::RenderPipeline,
    pipeline_water: wgpu::RenderPipeline,
    pipeline_emissive_instanced: wgpu::RenderPipeline,
    pipeline_textured_instanced: wgpu::RenderPipeline,
    // Shadow pass pipelines
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_pipeline_instanced: wgpu::RenderPipeline,
    shadow_pipeline_animated: wgpu::RenderPipeline,
    shadow_pipeline_cyclist: wgpu::RenderPipeline,
    shadow_pipeline_textured_instanced: wgpu::RenderPipeline,
    // Bind groups and buffers
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    shadow_depth_view: wgpu::TextureView,
    material_bgl: wgpu::BindGroupLayout,
    pub textured_material_bgl: wgpu::BindGroupLayout,
    light_buffer: wgpu::Buffer,
    depth_view: wgpu::TextureView,
    pub light_dir: Vec3,
    pub width: u32,
    pub height: u32,
    // Bone matrix skinning
    bone_buffer: wgpu::Buffer,
    bone_bind_group: wgpu::BindGroup,
    shadow_bone_bind_group: wgpu::BindGroup,
    // HUD
    pipeline_hud: wgpu::RenderPipeline,
    hud_camera_buffer: wgpu::Buffer,
    hud_camera_bind_group: wgpu::BindGroup,
    hud_vertex_buffer: wgpu::Buffer,
    hud_index_buffer: wgpu::Buffer,
    hud_num_indices: u32,
    hud_material_bind_group: wgpu::BindGroup,
    // Sky
    sky_pipeline: wgpu::RenderPipeline,
    sky_buffer: wgpu::Buffer,
    sky_bind_group: wgpu::BindGroup,
    // HDR + Post-process
    hdr_view: wgpu::TextureView,
    post_pipeline: wgpu::RenderPipeline,
    post_buffer: wgpu::Buffer,
    post_bind_group: wgpu::BindGroup,
    post_bgl: wgpu::BindGroupLayout,
    // Bloom
    bloom_extract_pipeline: wgpu::RenderPipeline,
    bloom_blur_pipeline: wgpu::RenderPipeline,
    bloom_extract_view: wgpu::TextureView,
    bloom_blur_temp_view: wgpu::TextureView,
    bloom_extract_bind_group: wgpu::BindGroup,
    bloom_blur_h_bind_group: wgpu::BindGroup,
    bloom_blur_v_bind_group: wgpu::BindGroup,
    bloom_bgl: wgpu::BindGroupLayout,
    bloom_buffer_extract: wgpu::Buffer,
    bloom_buffer_h: wgpu::Buffer,
    bloom_buffer_v: wgpu::Buffer,
    linear_sampler: wgpu::Sampler,
}

fn hud_ortho_uniform(w: f32, h: f32) -> CameraUniform {
    let proj = Mat4::orthographic_rh(0.0, w, h, 0.0, -1.0, 1.0);
    CameraUniform {
        view_proj: proj.to_cols_array_2d(),
        eye_pos: [0.0; 4],
        light_vp: Mat4::IDENTITY.to_cols_array_2d(),
    }
}

const HUD_MAX_VERTS: usize = 16384;
const HUD_MAX_INDICES: usize = 32768;

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let backends = if cfg!(target_arch = "wasm32") {
            wgpu::Backends::BROWSER_WEBGPU
        } else {
            wgpu::Backends::all()
        };
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("No suitable GPU adapter found");

        let required_limits = if cfg!(target_arch = "wasm32") {
            wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
        } else {
            wgpu::Limits::default()
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("sykla_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Light
        let light_dir = Vec3::new(-0.4, -0.45, -0.35).normalize();
        let light = LightUniform {
            direction: [light_dir.x, light_dir.y, light_dir.z, 0.0],
            color: [1.0, 0.88, 0.65, 1.0],
            ambient: [0.35, 0.32, 0.22, 1.0],
            fog_color: [0.72, 0.68, 0.52, 1.0],
        };

        // Uniform buffers
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::bytes_of(&CameraUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light_buffer"),
            contents: bytemuck::bytes_of(&light),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // --- Bind group layouts ---

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bgl"),
            entries: &[
                bgl_uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                bgl_uniform(1, wgpu::ShaderStages::FRAGMENT),
            ],
        });

        let material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material_bgl"),
            entries: &[bgl_uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });

        let textured_material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("textured_material_bgl"),
            entries: &[
                bgl_uniform(0, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        // Camera bind group
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // Shadow map texture
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_depth_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::Less),
            ..Default::default()
        });

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bg"),
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        // --- Bone matrix storage buffer (for cyclist skinning) ---

        let bone_buffer_size = (MAX_CYCLISTS * NUM_BONES * std::mem::size_of::<[[f32; 4]; 4]>()) as u64;
        let bone_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bone_matrix_buffer"),
            size: bone_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bone_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bone_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bone_bg"),
            layout: &bone_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_buffer.as_entire_binding(),
            }],
        });

        // Shadow cyclist uses the same buffer but at @group(1)
        let shadow_bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bone_bg"),
            layout: &bone_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_buffer.as_entire_binding(),
            }],
        });

        // --- Pipeline layouts ---

        let main_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main_layout"),
            bind_group_layouts: &[&camera_bgl, &material_bgl, &shadow_bgl],
            push_constant_ranges: &[],
        });

        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_layout"),
            bind_group_layouts: &[&camera_bgl],
            push_constant_ranges: &[],
        });

        // Cyclist layout: [camera, material, shadow, bones] — 4 groups
        let cyclist_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cyclist_layout"),
            bind_group_layouts: &[&camera_bgl, &material_bgl, &shadow_bgl, &bone_bgl],
            push_constant_ranges: &[],
        });

        // Shadow cyclist layout: [camera, bones] — 2 groups
        let shadow_cyclist_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_cyclist_layout"),
            bind_group_layouts: &[&camera_bgl, &bone_bgl],
            push_constant_ranges: &[],
        });

        // --- Pipelines ---

        let depth_stencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        };

        // Main non-instanced pipeline (terrain + ground)
        // No backface culling — on tight curves, inner terrain triangles can flip winding
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("main_pipeline"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Road pipeline — depth bias to prevent Z-fighting with terrain, clean shader
        let road_depth_stencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: -2,
                slope_scale: -2.0,
                clamp: 0.0,
            },
        };

        let pipeline_road = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("road_pipeline"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_road"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Road visible from both sides — path direction can flip winding
                ..Default::default()
            },
            depth_stencil: Some(road_depth_stencil),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Grass shell pipeline — alpha blended, no depth write, renders grass volume
        let pipeline_grass_shell =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("grass_shell_pipeline"),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_grass_shell"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_grass_shell"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Grass blade pipeline — instanced individual blades, alpha blended, no depth write
        let pipeline_grass_blade =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("grass_blade_pipeline"),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_grass_blade"),
                    buffers: &[Vertex::layout(), GrassBladeInstance::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_grass_blade"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Main instanced pipeline
        let pipeline_instanced = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("main_instanced_pipeline"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_instanced"),
                buffers: &[Vertex::layout(), InstanceData::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive,
            depth_stencil: Some(depth_stencil.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Shadow bias to reduce acne
        let shadow_bias = wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 2.0,
            clamp: 0.0,
        };

        let shadow_depth_stencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: shadow_bias,
        };

        // Shadow non-instanced pipeline — no backface cull to match main pipeline
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_pipeline"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(shadow_depth_stencil.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Shadow instanced pipeline
        let shadow_pipeline_instanced =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow_inst_pipeline"),
                layout: Some(&shadow_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_shadow_inst"),
                    buffers: &[Vertex::layout(), InstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive,
                depth_stencil: Some(shadow_depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Animated instance pipeline (wing flapping via vs_animated)
        let animated_primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None, // No backface cull — wings are thin flat surfaces
            ..Default::default()
        };

        let pipeline_animated =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("animated_pipeline"),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_animated"),
                    buffers: &[Vertex::layout(), AnimatedInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: animated_primitive,
                depth_stencil: Some(depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let shadow_pipeline_animated =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow_animated_pipeline"),
                layout: Some(&shadow_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_shadow_animated"),
                    buffers: &[Vertex::layout(), AnimatedInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive: animated_primitive,
                depth_stencil: Some(shadow_depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Cyclist instance pipeline (bone matrix skinning in vs_cyclist)
        let pipeline_cyclist =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("cyclist_pipeline"),
                layout: Some(&cyclist_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_cyclist"),
                    buffers: &[Vertex::layout(), CyclistInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_cyclist"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive,
                depth_stencil: Some(depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let shadow_pipeline_cyclist =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow_cyclist_pipeline"),
                layout: Some(&shadow_cyclist_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_shadow_cyclist"),
                    buffers: &[Vertex::layout(), CyclistInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive,
                depth_stencil: Some(shadow_depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Textured cyclist pipeline: [camera, textured_material, shadow, bones]
        let textured_cyclist_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("textured_cyclist_layout"),
            bind_group_layouts: &[&camera_bgl, &textured_material_bgl, &shadow_bgl, &bone_bgl],
            push_constant_ranges: &[],
        });

        let pipeline_textured_cyclist =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("textured_cyclist_pipeline"),
                layout: Some(&textured_cyclist_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_textured_cyclist"),
                    buffers: &[Vertex::layout(), CyclistInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_textured_cyclist"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive,
                depth_stencil: Some(depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Skin cyclist pipeline: same layout as cyclist [camera, material, shadow, bones]
        // Uses vs_skin_cyclist + fs_skin_cyclist for realistic skin rendering
        let pipeline_skin_cyclist =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("skin_cyclist_pipeline"),
                layout: Some(&cyclist_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_skin_cyclist"),
                    buffers: &[Vertex::layout(), CyclistInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_skin_cyclist"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive,
                depth_stencil: Some(depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Textured instanced pipeline (GLB trees with UV-mapped textures)
        let textured_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("textured_layout"),
            bind_group_layouts: &[&camera_bgl, &textured_material_bgl, &shadow_bgl],
            push_constant_ranges: &[],
        });

        let textured_primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None, // Foliage visible from both sides
            ..Default::default()
        };

        let pipeline_textured_instanced =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("textured_instanced_pipeline"),
                layout: Some(&textured_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_tree_instanced"),
                    buffers: &[Vertex::layout(), TreeInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_textured"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: textured_primitive,
                depth_stencil: Some(depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let shadow_pipeline_textured_instanced =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow_textured_inst_pipeline"),
                layout: Some(&shadow_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_shadow_tree_inst"),
                    buffers: &[Vertex::layout(), TreeInstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive: textured_primitive,
                depth_stencil: Some(shadow_depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Water pipeline (alpha blending, no backface cull, no depth write)
        let water_primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        };

        let pipeline_water =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("water_pipeline"),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_water"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_water"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: water_primitive,
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Emissive instanced pipeline (cabin windows — no lighting, just fog)
        let pipeline_emissive_instanced =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("emissive_instanced_pipeline"),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_instanced"),
                    buffers: &[Vertex::layout(), InstanceData::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_emissive"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive,
                depth_stencil: Some(depth_stencil.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // --- HUD pipeline ---

        let pipeline_hud =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("hud_pipeline"),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_hud"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_hud"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // HUD camera buffer (orthographic projection)
        let hud_uniform = hud_ortho_uniform(width as f32, height as f32);
        let hud_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hud_camera_buffer"),
            contents: bytemuck::bytes_of(&hud_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // HUD camera bind group (must be created while light_buffer is in scope)
        let hud_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud_camera_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: hud_camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // HUD dummy material bind group
        let hud_mat_uniform = MaterialUniform {
            base_color: [1.0, 1.0, 1.0, 1.0],
            mid_color: [1.0, 1.0, 1.0, 1.0],
            high_color: [1.0, 1.0, 1.0, 1.0],
            zone_params: [99999.0; 4],
            terrain_params: [0.0; 4],
        };
        let hud_mat_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hud_material_buffer"),
            contents: bytemuck::bytes_of(&hud_mat_uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let hud_material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud_material_bg"),
            layout: &material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: hud_mat_buffer.as_entire_binding(),
            }],
        });

        // Pre-allocated HUD vertex/index buffers
        let hud_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud_vertex_buffer"),
            size: (HUD_MAX_VERTS * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud_index_buffer"),
            size: (HUD_MAX_INDICES * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let depth_view = create_depth_texture(&device, width, height);

        let (hdr_view, _hdr_texture) = create_hdr_texture(&device, width, height);

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // --- Sky pipeline ---
        let sky_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky_bgl"),
            entries: &[bgl_uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let sky_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sky_buffer"),
            contents: bytemuck::bytes_of(&SkyUniforms::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sky_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky_bg"),
            layout: &sky_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sky_buffer.as_entire_binding(),
            }],
        });
        let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky_layout"),
            bind_group_layouts: &[&sky_bgl],
            push_constant_ranges: &[],
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_pipeline"),
            layout: Some(&sky_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- Bloom pipelines ---
        let bloom_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom_bgl"),
            entries: &[
                bgl_uniform(0, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bloom_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom_layout"),
            bind_group_layouts: &[&bloom_bgl],
            push_constant_ranges: &[],
        });

        let bloom_half_w = (width / 2).max(1);
        let bloom_half_h = (height / 2).max(1);
        let (bloom_extract_view, _bloom_extract_tex) = create_bloom_texture(&device, bloom_half_w, bloom_half_h, "bloom_extract");
        let (bloom_blur_temp_view, _bloom_blur_temp_tex) = create_bloom_texture(&device, bloom_half_w, bloom_half_h, "bloom_blur_temp");

        let bloom_extract_uniform = BloomUniforms { texel_size: [1.0 / width as f32, 1.0 / height as f32], direction: [0.0, 0.0] };
        let bloom_h_uniform = BloomUniforms { texel_size: [1.0 / bloom_half_w as f32, 1.0 / bloom_half_h as f32], direction: [1.0, 0.0] };
        let bloom_v_uniform = BloomUniforms { texel_size: [1.0 / bloom_half_w as f32, 1.0 / bloom_half_h as f32], direction: [0.0, 1.0] };

        let bloom_buffer_extract = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bloom_buffer_extract"),
            contents: bytemuck::bytes_of(&bloom_extract_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bloom_buffer_h = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bloom_buffer_h"),
            contents: bytemuck::bytes_of(&bloom_h_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bloom_buffer_v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bloom_buffer_v"),
            contents: bytemuck::bytes_of(&bloom_v_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bloom extract reads HDR texture
        let bloom_extract_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom_extract_bg"),
            layout: &bloom_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: bloom_buffer_extract.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&hdr_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&linear_sampler) },
            ],
        });
        // Bloom blur H reads bloom extract texture
        let bloom_blur_h_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom_blur_h_bg"),
            layout: &bloom_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: bloom_buffer_h.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&bloom_extract_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&linear_sampler) },
            ],
        });
        // Bloom blur V reads bloom blur temp texture
        let bloom_blur_v_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom_blur_v_bg"),
            layout: &bloom_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: bloom_buffer_v.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&bloom_blur_temp_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&linear_sampler) },
            ],
        });

        let bloom_extract_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bloom_extract_pipeline"),
            layout: Some(&bloom_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_bloom_extract"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let bloom_blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bloom_blur_pipeline"),
            layout: Some(&bloom_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_bloom_blur"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- Post-process pipeline ---
        let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post_bgl"),
            entries: &[
                bgl_uniform(0, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let post_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("post_buffer"),
            contents: bytemuck::bytes_of(&PostProcessUniforms::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let post_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post_bg"),
            layout: &post_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: post_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&hdr_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&linear_sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&bloom_extract_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&linear_sampler) },
            ],
        });
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post_layout"),
            bind_group_layouts: &[&post_bgl],
            push_constant_ranges: &[],
        });
        let post_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post_pipeline"),
            layout: Some(&post_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_post_process"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            device,
            queue,
            surface,
            config,
            pipeline,
            pipeline_road,
            pipeline_grass_shell,
            pipeline_grass_blade,
            pipeline_instanced,
            pipeline_animated,
            pipeline_cyclist,
            pipeline_textured_cyclist,
            pipeline_skin_cyclist,
            pipeline_water,
            pipeline_emissive_instanced,
            pipeline_textured_instanced,
            shadow_pipeline,
            shadow_pipeline_instanced,
            shadow_pipeline_animated,
            shadow_pipeline_cyclist,
            shadow_pipeline_textured_instanced,
            camera_bind_group,
            camera_buffer,
            shadow_bind_group,
            shadow_depth_view,
            material_bgl,
            textured_material_bgl,
            light_buffer,
            depth_view,
            light_dir,
            width,
            height,
            bone_buffer,
            bone_bind_group,
            shadow_bone_bind_group,
            pipeline_hud,
            hud_camera_buffer,
            hud_camera_bind_group,
            hud_vertex_buffer,
            hud_index_buffer,
            hud_num_indices: 0,
            hud_material_bind_group,
            sky_pipeline,
            sky_buffer,
            sky_bind_group,
            hdr_view,
            post_pipeline,
            post_buffer,
            post_bind_group,
            post_bgl,
            bloom_extract_pipeline,
            bloom_blur_pipeline,
            bloom_extract_view,
            bloom_blur_temp_view,
            bloom_extract_bind_group,
            bloom_blur_h_bind_group,
            bloom_blur_v_bind_group,
            bloom_bgl,
            bloom_buffer_extract,
            bloom_buffer_h,
            bloom_buffer_v,
            linear_sampler,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = create_depth_texture(&self.device, width, height);
            let hud_uniform = hud_ortho_uniform(width as f32, height as f32);
            self.queue.write_buffer(
                &self.hud_camera_buffer,
                0,
                bytemuck::bytes_of(&hud_uniform),
            );

            // Recreate HDR texture
            let (hdr_view, _hdr_tex) = create_hdr_texture(&self.device, width, height);
            self.hdr_view = hdr_view;

            // Recreate bloom textures
            let bloom_w = (width / 2).max(1);
            let bloom_h = (height / 2).max(1);
            let (bloom_extract_view, _) = create_bloom_texture(&self.device, bloom_w, bloom_h, "bloom_extract");
            let (bloom_blur_temp_view, _) = create_bloom_texture(&self.device, bloom_w, bloom_h, "bloom_blur_temp");
            self.bloom_extract_view = bloom_extract_view;
            self.bloom_blur_temp_view = bloom_blur_temp_view;

            // Update bloom uniforms with new texel sizes
            let bloom_texel = [1.0 / bloom_w as f32, 1.0 / bloom_h as f32];
            self.queue.write_buffer(&self.bloom_buffer_extract, 0, bytemuck::bytes_of(&BloomUniforms {
                texel_size: [1.0 / width as f32, 1.0 / height as f32],
                direction: [0.0, 0.0],
            }));
            self.queue.write_buffer(&self.bloom_buffer_h, 0, bytemuck::bytes_of(&BloomUniforms {
                texel_size: bloom_texel,
                direction: [1.0, 0.0],
            }));
            self.queue.write_buffer(&self.bloom_buffer_v, 0, bytemuck::bytes_of(&BloomUniforms {
                texel_size: bloom_texel,
                direction: [0.0, 1.0],
            }));

            // Recreate bind groups that reference textures
            self.bloom_extract_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom_extract_bg"),
                layout: &self.bloom_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.bloom_buffer_extract.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.hdr_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.linear_sampler) },
                ],
            });
            self.bloom_blur_h_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom_blur_h_bg"),
                layout: &self.bloom_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.bloom_buffer_h.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.bloom_extract_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.linear_sampler) },
                ],
            });
            self.bloom_blur_v_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom_blur_v_bg"),
                layout: &self.bloom_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.bloom_buffer_v.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.bloom_blur_temp_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.linear_sampler) },
                ],
            });
            self.post_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post_bg"),
                layout: &self.post_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.post_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.hdr_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.linear_sampler) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.bloom_extract_view) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.linear_sampler) },
                ],
            });
        }
    }

    pub fn create_material(&self, color: [f32; 4]) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color: color,
            mid_color: color,
            high_color: color,
            zone_params: [99999.0; 4],
            terrain_params: [0.0; 4],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material_bg"),
            layout: &self.material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    /// Create a cyclist material with optional per-bone coloring for bike parts.
    /// If `frame_color` and `component_color` are provided, terrain_params.x is set > 0
    /// to enable per-bone coloring in fs_cyclist (bone 20 = frame, 21/22 = components).
    pub fn create_cyclist_material(
        &self,
        base_color: [f32; 4],
        frame_color: Option<[f32; 3]>,
        component_color: Option<[f32; 3]>,
    ) -> wgpu::BindGroup {
        let has_bone_colors = frame_color.is_some() || component_color.is_some();
        let fc = frame_color.unwrap_or([base_color[0], base_color[1], base_color[2]]);
        let cc = component_color.unwrap_or([base_color[0], base_color[1], base_color[2]]);
        let uniform = MaterialUniform {
            base_color,
            mid_color: [fc[0], fc[1], fc[2], 1.0],
            high_color: [cc[0], cc[1], cc[2], 1.0],
            zone_params: [99999.0; 4],
            terrain_params: [if has_bone_colors { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cyclist_material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cyclist_material_bg"),
            layout: &self.material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    /// Create a foliage material with snow-on-normals flag (mid_color.w = 2.0).
    /// The shader detects mid_color.w > 1.5 and applies white snow to upward-facing surfaces.
    pub fn create_snow_foliage_material(&self, color: [f32; 4]) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color: color,
            mid_color: [color[0], color[1], color[2], 2.0], // flag for snow-on-normals
            high_color: color,
            zone_params: [99999.0; 4],
            terrain_params: [0.0; 4],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("snow_foliage_material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("snow_foliage_material_bg"),
            layout: &self.material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_terrain_material(
        &self,
        base: [f32; 4],
        mid: [f32; 4],
        high: [f32; 4],
        zones: [f32; 4],
        terrain_params: [f32; 4],
    ) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color: base,
            mid_color: mid,
            high_color: high,
            zone_params: zones,
            terrain_params,
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain_material_bg"),
            layout: &self.material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_road_material(&self, color: [f32; 4], markings: bool, is_gravel: bool) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color: color,
            mid_color: color,
            high_color: color,
            zone_params: [if markings { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
            terrain_params: [if is_gravel { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("road_material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("road_material_bg"),
            layout: &self.material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_grass_material(&self, params: &crate::grass::GrassParams) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color: [params.color_base[0], params.color_base[1], params.color_base[2], params.num_shells as f32],
            mid_color: [params.color_tip[0], params.color_tip[1], params.color_tip[2], params.shell_height],
            high_color: [params.density, params.wind_strength, params.wind_dir[0], params.wind_dir[1]],
            zone_params: [params.fade_start, params.fade_end, 0.0, 0.0],
            terrain_params: [0.0; 4],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grass_material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grass_material_bg"),
            layout: &self.material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_grass_blade_material(
        &self,
        config: &crate::grass_blades::GrassBladeConfig,
    ) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color: [config.color_base[0], config.color_base[1], config.color_base[2], 0.0],
            mid_color: [config.color_tip[0], config.color_tip[1], config.color_tip[2], 0.0],
            high_color: [config.wind_strength, 0.0, 0.7, 0.7],
            zone_params: [config.fade_start, config.fade_end, 0.0, 0.0],
            terrain_params: [0.0; 4],
        };
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("grass_blade_material_buffer"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grass_blade_material_bg"),
            layout: &self.material_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_grass_blade_instance_buffer(&self, max_instances: usize) -> wgpu::Buffer {
        let size = (max_instances * std::mem::size_of::<GrassBladeInstance>()) as u64;
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grass_blade_instance_buffer"),
            size: size.max(std::mem::size_of::<GrassBladeInstance>() as u64),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn set_fog_color(&self, color: [f32; 4]) {
        let offset = std::mem::offset_of!(LightUniform, fog_color) as u64;
        self.queue.write_buffer(&self.light_buffer, offset, bytemuck::bytes_of(&color));
    }

    pub fn set_light_direction_uniform(&self, dir: [f32; 3]) {
        let padded: [f32; 4] = [dir[0], dir[1], dir[2], 0.0];
        let offset = std::mem::offset_of!(LightUniform, direction) as u64;
        self.queue.write_buffer(&self.light_buffer, offset, bytemuck::bytes_of(&padded));
    }

    pub fn set_light_color(&self, color: [f32; 4]) {
        let offset = std::mem::offset_of!(LightUniform, color) as u64;
        self.queue.write_buffer(&self.light_buffer, offset, bytemuck::bytes_of(&color));
    }

    pub fn set_light_ambient(&self, ambient: [f32; 4]) {
        let offset = std::mem::offset_of!(LightUniform, ambient) as u64;
        self.queue.write_buffer(&self.light_buffer, offset, bytemuck::bytes_of(&ambient));
    }

    pub fn create_instance_buffer(&self, instances: &[InstanceData]) -> wgpu::Buffer {
        if instances.is_empty() {
            // wgpu doesn't allow 0-size buffers; create a tiny dummy
            return self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance_buffer_empty"),
                size: std::mem::size_of::<InstanceData>() as u64,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
        }
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("instance_buffer"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            })
    }

    pub fn create_animated_instance_buffer(&self, instances: &[AnimatedInstanceData]) -> wgpu::Buffer {
        if instances.is_empty() {
            return self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("anim_instance_buffer_empty"),
                size: std::mem::size_of::<AnimatedInstanceData>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("anim_instance_buffer"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            })
    }

    pub fn create_cyclist_instance_buffer(&self, instances: &[CyclistInstanceData]) -> wgpu::Buffer {
        if instances.is_empty() {
            return self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cyclist_instance_buffer_empty"),
                size: std::mem::size_of::<CyclistInstanceData>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cyclist_instance_buffer"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            })
    }

    pub fn create_tree_instance_buffer(&self, instances: &[TreeInstanceData]) -> wgpu::Buffer {
        if instances.is_empty() {
            return self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tree_instance_buffer_empty"),
                size: std::mem::size_of::<TreeInstanceData>() as u64,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
        }
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tree_instance_buffer"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            })
    }

    pub fn create_textured_material(
        &self,
        color: [f32; 4],
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color: color,
            mid_color: color,
            high_color: color,
            zone_params: [99999.0; 4],
            terrain_params: [0.0; 4],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("textured_material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textured_material_bg"),
            layout: &self.textured_material_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn create_textured_cyclist_material(
        &self,
        base_color: [f32; 4],
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            base_color,
            mid_color: base_color,
            high_color: base_color,
            zone_params: [99999.0; 4],
            terrain_params: [0.0; 4],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("textured_cyclist_material_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textured_cyclist_material_bg"),
            layout: &self.textured_material_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn create_gpu_texture(&self, rgba: &[u8], width: u32, height: u32) -> (wgpu::TextureView, wgpu::Sampler) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("jersey_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("jersey_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        (view, sampler)
    }

    pub fn update_hud(&mut self, vertices: &[Vertex], indices: &[u32]) {
        let vert_count = vertices.len().min(HUD_MAX_VERTS);
        let idx_count = indices.len().min(HUD_MAX_INDICES);
        if vert_count > 0 {
            self.queue.write_buffer(
                &self.hud_vertex_buffer,
                0,
                bytemuck::cast_slice(&vertices[..vert_count]),
            );
        }
        if idx_count > 0 {
            self.queue.write_buffer(
                &self.hud_index_buffer,
                0,
                bytemuck::cast_slice(&indices[..idx_count]),
            );
        }
        self.hud_num_indices = idx_count as u32;
    }

    pub fn update_camera(&self, camera: &Camera, time: f32) {
        let uniform = camera.uniform(self.light_dir, time);
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn update_bone_matrices(&self, matrices: &[[[f32; 4]; 4]]) {
        if !matrices.is_empty() {
            self.queue.write_buffer(
                &self.bone_buffer,
                0,
                bytemuck::cast_slice(matrices),
            );
        }
    }

    pub fn update_sky(&self, uniforms: &SkyUniforms) {
        self.queue
            .write_buffer(&self.sky_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    pub fn update_post_process(&self, uniforms: &PostProcessUniforms) {
        self.queue
            .write_buffer(&self.post_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    pub fn render(
        &self,
        draws: &[DrawCall],
        road_draws: &[DrawCall],
        grass_shell_draws: &[GrassShellDraw],
        grass_blade_draws: &[GrassBladeDrawCall],
        instanced_draws: &[InstancedDrawCall],
        animated_draws: &[AnimatedDrawCall],
        cyclist_models: &[CyclistModel],
        water_draws: &[DrawCall],
        emissive_instanced_draws: &[InstancedDrawCall],
        tree_models: &[TreeModel],
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let surface_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        // === Shadow pass ===
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // Non-instanced shadow draws (terrain + ground)
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            for call in draws {
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..1);
            }

            // Instanced shadow draws
            pass.set_pipeline(&self.shadow_pipeline_instanced);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            for call in instanced_draws {
                if call.instance_count == 0 { continue; }
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, call.instance_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..call.instance_count);
            }

            // Animated shadow draws
            pass.set_pipeline(&self.shadow_pipeline_animated);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            for call in animated_draws {
                if call.instance_count == 0 { continue; }
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, call.instance_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..call.instance_count);
            }

            // Cyclist shadow draws (layout: [camera, bones])
            pass.set_pipeline(&self.shadow_pipeline_cyclist);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.shadow_bone_bind_group, &[]);
            for model in cyclist_models {
                if model.instance_count == 0 { continue; }
                pass.set_vertex_buffer(1, model.instance_buffer.slice(..));
                for part in &model.parts {
                    pass.set_vertex_buffer(0, part.mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(part.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..part.mesh.num_indices, 0, 0..model.instance_count);
                }
            }

            // Textured tree shadow draws
            pass.set_pipeline(&self.shadow_pipeline_textured_instanced);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            for model in tree_models {
                if model.instance_count == 0 { continue; }
                pass.set_vertex_buffer(1, model.instance_buffer.slice(..));
                for part in &model.parts {
                    pass.set_vertex_buffer(0, part.mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(part.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..part.mesh.num_indices, 0, 0..model.instance_count);
                }
            }
        }

        // === Main pass → renders to HDR texture ===
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // Sky (fullscreen triangle, no depth write, renders first)
            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.sky_bind_group, &[]);
            pass.draw(0..3, 0..1);

            // Non-instanced main draws (terrain + ground)
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for call in draws {
                pass.set_bind_group(1, &call.material_bind_group, &[]);
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..1);
            }

            // Road draws
            pass.set_pipeline(&self.pipeline_road);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for call in road_draws {
                pass.set_bind_group(1, &call.material_bind_group, &[]);
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..1);
            }

            // Grass shell draws
            pass.set_pipeline(&self.pipeline_grass_shell);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for draw in grass_shell_draws {
                pass.set_bind_group(1, &draw.material_bind_group, &[]);
                pass.set_vertex_buffer(0, draw.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(draw.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.mesh.num_indices, 0, 0..draw.num_shells);
            }

            // Grass blade draws
            pass.set_pipeline(&self.pipeline_grass_blade);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for draw in grass_blade_draws {
                if draw.instance_count == 0 { continue; }
                pass.set_bind_group(1, &draw.material_bind_group, &[]);
                pass.set_vertex_buffer(0, draw.mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, draw.instance_buffer.slice(..));
                pass.set_index_buffer(draw.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.mesh.num_indices, 0, 0..draw.instance_count);
            }

            // Instanced main draws
            pass.set_pipeline(&self.pipeline_instanced);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for call in instanced_draws {
                if call.instance_count == 0 { continue; }
                pass.set_bind_group(1, &call.material_bind_group, &[]);
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, call.instance_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..call.instance_count);
            }

            // Animated main draws
            pass.set_pipeline(&self.pipeline_animated);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for call in animated_draws {
                if call.instance_count == 0 { continue; }
                pass.set_bind_group(1, &call.material_bind_group, &[]);
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, call.instance_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..call.instance_count);
            }

            // Cyclist solid parts
            pass.set_pipeline(&self.pipeline_cyclist);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_bind_group(3, &self.bone_bind_group, &[]);
            for model in cyclist_models {
                if model.instance_count == 0 { continue; }
                pass.set_vertex_buffer(1, model.instance_buffer.slice(..));
                for part in &model.parts {
                    if part.pipeline == CyclistPipeline::Solid {
                        pass.set_bind_group(1, &part.material_bind_group, &[]);
                        pass.set_vertex_buffer(0, part.mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(part.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..part.mesh.num_indices, 0, 0..model.instance_count);
                    }
                }
            }

            // Cyclist textured parts
            pass.set_pipeline(&self.pipeline_textured_cyclist);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_bind_group(3, &self.bone_bind_group, &[]);
            for model in cyclist_models {
                if model.instance_count == 0 { continue; }
                pass.set_vertex_buffer(1, model.instance_buffer.slice(..));
                for part in &model.parts {
                    if part.pipeline == CyclistPipeline::Textured {
                        pass.set_bind_group(1, &part.material_bind_group, &[]);
                        pass.set_vertex_buffer(0, part.mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(part.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..part.mesh.num_indices, 0, 0..model.instance_count);
                    }
                }
            }

            // Cyclist skin parts
            pass.set_pipeline(&self.pipeline_skin_cyclist);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_bind_group(3, &self.bone_bind_group, &[]);
            for model in cyclist_models {
                if model.instance_count == 0 { continue; }
                pass.set_vertex_buffer(1, model.instance_buffer.slice(..));
                for part in &model.parts {
                    if part.pipeline == CyclistPipeline::Skin {
                        pass.set_bind_group(1, &part.material_bind_group, &[]);
                        pass.set_vertex_buffer(0, part.mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(part.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..part.mesh.num_indices, 0, 0..model.instance_count);
                    }
                }
            }

            // Emissive instanced draws
            pass.set_pipeline(&self.pipeline_emissive_instanced);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for call in emissive_instanced_draws {
                if call.instance_count == 0 { continue; }
                pass.set_bind_group(1, &call.material_bind_group, &[]);
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, call.instance_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..call.instance_count);
            }

            // Textured tree main draws
            pass.set_pipeline(&self.pipeline_textured_instanced);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for model in tree_models {
                if model.instance_count == 0 { continue; }
                pass.set_vertex_buffer(1, model.instance_buffer.slice(..));
                for part in &model.parts {
                    pass.set_bind_group(1, &part.material_bind_group, &[]);
                    pass.set_vertex_buffer(0, part.mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(part.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..part.mesh.num_indices, 0, 0..model.instance_count);
                }
            }

            // Water draws (last, alpha blended)
            pass.set_pipeline(&self.pipeline_water);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for call in water_draws {
                pass.set_bind_group(1, &call.material_bind_group, &[]);
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..1);
            }
        }

        // === Bloom extract pass → reads HDR, writes bloom_extract (half res) ===
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom_extract_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_extract_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.bloom_extract_pipeline);
            pass.set_bind_group(0, &self.bloom_extract_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // === Bloom blur horizontal → reads bloom_extract, writes bloom_blur_temp ===
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom_blur_h_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_blur_temp_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.bloom_blur_pipeline);
            pass.set_bind_group(0, &self.bloom_blur_h_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // === Bloom blur vertical → reads bloom_blur_temp, writes bloom_extract ===
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom_blur_v_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_extract_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.bloom_blur_pipeline);
            pass.set_bind_group(0, &self.bloom_blur_v_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // === Post-process pass → reads HDR + bloom, writes to surface ===
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post_process_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.post_pipeline);
            pass.set_bind_group(0, &self.post_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // === HUD pass → writes directly to surface (after post-process) ===
        if self.hud_num_indices > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline_hud);
            pass.set_bind_group(0, &self.hud_camera_bind_group, &[]);
            pass.set_bind_group(1, &self.hud_material_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_vertex_buffer(0, self.hud_vertex_buffer.slice(..));
            pass.set_index_buffer(self.hud_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.hud_num_indices, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn create_hdr_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::TextureView, wgpu::Texture) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (view, texture)
}

fn create_bloom_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> (wgpu::TextureView, wgpu::Texture) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (view, texture)
}

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn bgl_uniform(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
