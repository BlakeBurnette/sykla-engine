use crate::gltf;
use crate::mesh::GpuMesh;
use crate::renderer::{Renderer, TreeMeshPart};

pub fn load_tree_species(
    renderer: &Renderer,
    glb_bytes: &[u8],
) -> Vec<TreeMeshPart> {
    let glb = gltf::parse_glb(glb_bytes).expect("failed to parse tree GLB");

    log::info!("  GLB: {} primitives, {} images", glb.primitives.len(), glb.images.len());
    for (i, prim) in glb.primitives.iter().enumerate() {
        let verts = &prim.mesh.vertices;
        if !verts.is_empty() {
            let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
            for v in verts {
                min_y = min_y.min(v.position[1]);
                max_y = max_y.max(v.position[1]);
            }
            log::info!("  Prim[{}]: {} verts, {} indices, Y=[{:.2}..{:.2}], color={:?}, tex={:?}",
                i, verts.len(), prim.mesh.indices.len(), min_y, max_y,
                prim.base_color, prim.base_color_texture);
        }
    }

    // Decode images → GPU textures
    let mut gpu_textures: Vec<(wgpu::TextureView, wgpu::Sampler)> = Vec::new();
    for image in &glb.images {
        log::info!("  Image: {} bytes, mime={}", image.data.len(), image.mime_type);
        let (view, sampler) = decode_png_to_texture(&renderer.device, &renderer.queue, &image.data);
        gpu_textures.push((view, sampler));
    }

    // Fallback 1x1 white texture for primitives without a texture
    let (fallback_view, fallback_sampler) = create_fallback_texture(&renderer.device, &renderer.queue);

    let mut parts = Vec::new();
    for prim in &glb.primitives {
        let gpu_mesh = GpuMesh::from_cpu(&renderer.device, &prim.mesh);

        let (view, sampler) = if let Some(tex_idx) = prim.base_color_texture {
            if let Some((v, s)) = gpu_textures.get(tex_idx) {
                (v, s)
            } else {
                (&fallback_view, &fallback_sampler)
            }
        } else {
            (&fallback_view, &fallback_sampler)
        };

        let bind_group = renderer.create_textured_material(prim.base_color, view, sampler);

        parts.push(TreeMeshPart {
            mesh: gpu_mesh,
            material_bind_group: bind_group,
        });
    }

    parts
}

fn decode_png_to_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    png_bytes: &[u8],
) -> (wgpu::TextureView, wgpu::Sampler) {
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder.read_info().expect("failed to read PNG header");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("failed to decode PNG frame");

    let width = info.width;
    let height = info.height;

    // Convert to RGBA8 if needed
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let pixels = info.buffer_size() / 3;
            let mut rgba = Vec::with_capacity(pixels * 4);
            for i in 0..pixels {
                rgba.push(buf[i * 3]);
                rgba.push(buf[i * 3 + 1]);
                rgba.push(buf[i * 3 + 2]);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::GrayscaleAlpha => {
            let pixels = info.buffer_size() / 2;
            let mut rgba = Vec::with_capacity(pixels * 4);
            for i in 0..pixels {
                let g = buf[i * 2];
                let a = buf[i * 2 + 1];
                rgba.extend_from_slice(&[g, g, g, a]);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let pixels = info.buffer_size();
            let mut rgba = Vec::with_capacity(pixels * 4);
            for i in 0..pixels {
                let g = buf[i];
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
            rgba
        }
        _ => {
            // Fallback: treat as RGBA
            buf[..info.buffer_size()].to_vec()
        }
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tree_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("tree_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (view, sampler)
}

fn create_fallback_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fallback_texture"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[255, 255, 255, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("fallback_sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    (view, sampler)
}
