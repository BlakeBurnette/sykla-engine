use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUniform};
use crate::mesh::{GpuMesh, Vertex};
use crate::vegetation::InstanceData;
use crate::wildlife::AnimatedInstanceData;

const SHADOW_MAP_SIZE: u32 = 4096;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightUniform {
    pub direction: [f32; 4],
    pub color: [f32; 4],
    pub ambient: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
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

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    // Main pass pipelines
    pipeline: wgpu::RenderPipeline,
    pipeline_road: wgpu::RenderPipeline,
    pipeline_instanced: wgpu::RenderPipeline,
    pipeline_animated: wgpu::RenderPipeline,
    pipeline_water: wgpu::RenderPipeline,
    // Shadow pass pipelines
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_pipeline_instanced: wgpu::RenderPipeline,
    shadow_pipeline_animated: wgpu::RenderPipeline,
    // Bind groups and buffers
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    shadow_depth_view: wgpu::TextureView,
    material_bgl: wgpu::BindGroupLayout,
    depth_view: wgpu::TextureView,
    pub light_dir: Vec3,
    pub width: u32,
    pub height: u32,
    // HUD
    pipeline_hud: wgpu::RenderPipeline,
    hud_camera_buffer: wgpu::Buffer,
    hud_camera_bind_group: wgpu::BindGroup,
    hud_vertex_buffer: wgpu::Buffer,
    hud_index_buffer: wgpu::Buffer,
    hud_num_indices: u32,
    hud_material_bind_group: wgpu::BindGroup,
}

fn hud_ortho_uniform(w: f32, h: f32) -> CameraUniform {
    let proj = Mat4::orthographic_rh(0.0, w, h, 0.0, -1.0, 1.0);
    CameraUniform {
        view_proj: proj.to_cols_array_2d(),
        eye_pos: [0.0; 4],
        light_vp: Mat4::IDENTITY.to_cols_array_2d(),
    }
}

const HUD_MAX_VERTS: usize = 4096;
const HUD_MAX_INDICES: usize = 8192;

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
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

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("sykla_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
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
            usage: wgpu::BufferUsages::UNIFORM,
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
            entries: &[bgl_uniform(0, wgpu::ShaderStages::FRAGMENT)],
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
                    format: config.format,
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
                    format: config.format,
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
                    format: config.format,
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
                        format: config.format,
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
                depth_stencil: Some(shadow_depth_stencil),
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
                        format: config.format,
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

        Self {
            device,
            queue,
            surface,
            config,
            pipeline,
            pipeline_road,
            pipeline_instanced,
            pipeline_animated,
            pipeline_water,
            shadow_pipeline,
            shadow_pipeline_instanced,
            shadow_pipeline_animated,
            camera_bind_group,
            camera_buffer,
            shadow_bind_group,
            shadow_depth_view,
            material_bgl,
            depth_view,
            light_dir,
            width,
            height,
            pipeline_hud,
            hud_camera_buffer,
            hud_camera_bind_group,
            hud_vertex_buffer,
            hud_index_buffer,
            hud_num_indices: 0,
            hud_material_bind_group,
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
        }
    }

    pub fn create_material(&self, color: [f32; 4]) -> wgpu::BindGroup {
        let uniform = MaterialUniform { base_color: color };
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

    pub fn render(
        &self,
        draws: &[DrawCall],
        road_draws: &[DrawCall],
        instanced_draws: &[InstancedDrawCall],
        animated_draws: &[AnimatedDrawCall],
        water_draws: &[DrawCall],
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
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
            // Road omitted from shadow pass — flat on terrain, shouldn't cast shadows

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
        }

        // === Main pass ===
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.52,
                            g: 0.70,
                            b: 0.82,
                            a: 1.0,
                        }),
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

            // Road draws — depth-biased pipeline, clean shader
            pass.set_pipeline(&self.pipeline_road);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            for call in road_draws {
                pass.set_bind_group(1, &call.material_bind_group, &[]);
                pass.set_vertex_buffer(0, call.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(call.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..call.mesh.num_indices, 0, 0..1);
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

        // === HUD pass ===
        if self.hud_num_indices > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
