//! Minimal GLB (glTF binary) parser — no external crate dependency.
//!
//! Supports: meshes with POSITION, NORMAL, TEXCOORD_0, indexed primitives,
//! and PBR base color factor. No textures, cameras, skins, morph targets.

use serde::Deserialize;
use std::fmt;

use crate::mesh::{CpuMesh, Vertex};

// ── Public types ─────────────────────────────────────────────────

#[derive(Debug)]
pub enum GltfError {
    TooSmall,
    BadMagic,
    UnsupportedVersion(u32),
    MissingJsonChunk,
    MissingBinChunk,
    Json(String),
    Accessor(String),
}

impl fmt::Display for GltfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GltfError::TooSmall => write!(f, "GLB data too small"),
            GltfError::BadMagic => write!(f, "Not a GLB file (bad magic)"),
            GltfError::UnsupportedVersion(v) => write!(f, "Unsupported GLB version: {v}"),
            GltfError::MissingJsonChunk => write!(f, "Missing JSON chunk"),
            GltfError::MissingBinChunk => write!(f, "Missing BIN chunk"),
            GltfError::Json(e) => write!(f, "JSON parse error: {e}"),
            GltfError::Accessor(e) => write!(f, "Accessor error: {e}"),
        }
    }
}

pub struct GlbData {
    pub primitives: Vec<GltfPrimitive>,
    pub images: Vec<GltfImage>,
}

pub struct GltfImage {
    pub data: Vec<u8>,
    pub mime_type: String,
}

pub struct GltfPrimitive {
    pub mesh: CpuMesh,
    pub base_color: [f32; 4],
    pub base_color_texture: Option<usize>,
}

// ── Internal JSON schema ─────────────────────────────────────────

#[derive(Deserialize)]
struct GltfJson {
    #[serde(default)]
    accessors: Vec<Accessor>,
    #[serde(default, rename = "bufferViews")]
    buffer_views: Vec<BufferView>,
    #[serde(default)]
    meshes: Vec<Mesh>,
    #[serde(default)]
    materials: Vec<Material>,
    #[serde(default)]
    textures: Vec<GltfTexture>,
    #[serde(default)]
    images: Vec<GltfImageJson>,
}

#[derive(Deserialize)]
struct GltfTexture {
    #[serde(default)]
    source: Option<usize>,
}

#[derive(Deserialize)]
struct GltfImageJson {
    #[serde(default, rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(default, rename = "bufferView")]
    buffer_view: Option<usize>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Accessor {
    #[serde(default, rename = "bufferView")]
    buffer_view: usize,
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    accessor_type: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct BufferView {
    #[serde(default)]
    buffer: usize,
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    #[serde(default, rename = "byteStride")]
    byte_stride: Option<usize>,
}

#[derive(Deserialize)]
struct Mesh {
    primitives: Vec<Primitive>,
}

#[derive(Deserialize)]
struct Primitive {
    #[serde(default)]
    attributes: PrimitiveAttributes,
    #[serde(default)]
    indices: Option<usize>,
    #[serde(default)]
    material: Option<usize>,
}

#[derive(Deserialize, Default)]
struct PrimitiveAttributes {
    #[serde(default, rename = "POSITION")]
    position: Option<usize>,
    #[serde(default, rename = "NORMAL")]
    normal: Option<usize>,
    #[serde(default, rename = "TEXCOORD_0")]
    texcoord_0: Option<usize>,
}

#[derive(Deserialize)]
struct Material {
    #[serde(default, rename = "pbrMetallicRoughness")]
    pbr: Option<PbrMetallicRoughness>,
}

#[derive(Deserialize)]
struct PbrMetallicRoughness {
    #[serde(default = "default_base_color", rename = "baseColorFactor")]
    base_color_factor: [f32; 4],
    #[serde(default, rename = "baseColorTexture")]
    base_color_texture: Option<TextureInfo>,
}

#[derive(Deserialize)]
struct TextureInfo {
    index: usize,
}

fn default_base_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

// ── GLB constants ────────────────────────────────────────────────

const GLB_MAGIC: u32 = 0x46546C67;
const CHUNK_JSON: u32 = 0x4E4F534A;
const CHUNK_BIN: u32 = 0x004E4942;

// Component types
const FLOAT: u32 = 5126;
const UNSIGNED_BYTE: u32 = 5121;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;

// ── Public API ───────────────────────────────────────────────────

pub fn parse_glb(data: &[u8]) -> Result<GlbData, GltfError> {
    if data.len() < 12 {
        return Err(GltfError::TooSmall);
    }

    // Header
    let magic = u32_le(data, 0);
    let version = u32_le(data, 4);
    let _total_length = u32_le(data, 8);

    if magic != GLB_MAGIC {
        return Err(GltfError::BadMagic);
    }
    if version != 2 {
        return Err(GltfError::UnsupportedVersion(version));
    }

    // Chunks
    let mut offset = 12usize;
    let mut json_bytes: Option<&[u8]> = None;
    let mut bin_bytes: Option<&[u8]> = None;

    while offset + 8 <= data.len() {
        let chunk_len = u32_le(data, offset) as usize;
        let chunk_type = u32_le(data, offset + 4);
        let chunk_data = &data[offset + 8..offset + 8 + chunk_len];

        match chunk_type {
            CHUNK_JSON => json_bytes = Some(chunk_data),
            CHUNK_BIN => bin_bytes = Some(chunk_data),
            _ => {} // skip unknown chunks
        }

        offset += 8 + chunk_len;
        // Chunks are 4-byte aligned
        offset = (offset + 3) & !3;
    }

    let json_bytes = json_bytes.ok_or(GltfError::MissingJsonChunk)?;
    let bin = bin_bytes.ok_or(GltfError::MissingBinChunk)?;

    let gltf: GltfJson =
        serde_json::from_slice(json_bytes).map_err(|e| GltfError::Json(e.to_string()))?;

    // Extract embedded images from buffer views
    let mut images = Vec::new();
    for img_json in &gltf.images {
        if let Some(bv_idx) = img_json.buffer_view {
            if let Some(view) = gltf.buffer_views.get(bv_idx) {
                let start = view.byte_offset;
                let end = start + view.byte_length;
                if end <= bin.len() {
                    images.push(GltfImage {
                        data: bin[start..end].to_vec(),
                        mime_type: img_json.mime_type.clone().unwrap_or_default(),
                    });
                }
            }
        }
    }

    // Parse all mesh primitives
    let mut primitives = Vec::new();

    for mesh in &gltf.meshes {
        for prim in &mesh.primitives {
            let pos_idx = prim
                .attributes
                .position
                .ok_or_else(|| GltfError::Accessor("Missing POSITION attribute".into()))?;

            let positions = read_accessor_vec3(&gltf.accessors, &gltf.buffer_views, bin, pos_idx)?;
            let count = positions.len();

            let normals = if let Some(idx) = prim.attributes.normal {
                read_accessor_vec3(&gltf.accessors, &gltf.buffer_views, bin, idx)?
            } else {
                // Generate flat normals
                vec![[0.0, 1.0, 0.0]; count]
            };

            let uvs = if let Some(idx) = prim.attributes.texcoord_0 {
                read_accessor_vec2(&gltf.accessors, &gltf.buffer_views, bin, idx)?
            } else {
                vec![[0.0, 0.0]; count]
            };

            let indices = if let Some(idx) = prim.indices {
                read_indices(&gltf.accessors, &gltf.buffer_views, bin, idx)?
            } else {
                (0..count as u32).collect()
            };

            let mat = prim
                .material
                .and_then(|mi| gltf.materials.get(mi));
            let pbr = mat.and_then(|m| m.pbr.as_ref());

            let base_color = pbr
                .map(|p| p.base_color_factor)
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            // Resolve baseColorTexture → image index
            let base_color_texture = pbr
                .and_then(|p| p.base_color_texture.as_ref())
                .and_then(|ti| gltf.textures.get(ti.index))
                .and_then(|t| t.source);

            let vertices: Vec<Vertex> = (0..count)
                .map(|i| Vertex {
                    position: positions[i],
                    normal: normals[i],
                    uv: uvs[i],
                })
                .collect();

            primitives.push(GltfPrimitive {
                mesh: CpuMesh { vertices, indices },
                base_color,
                base_color_texture,
            });
        }
    }

    Ok(GlbData { primitives, images })
}

// ── Accessor helpers ─────────────────────────────────────────────

fn read_accessor_vec3(
    accessors: &[Accessor],
    views: &[BufferView],
    bin: &[u8],
    idx: usize,
) -> Result<Vec<[f32; 3]>, GltfError> {
    let acc = accessors
        .get(idx)
        .ok_or_else(|| GltfError::Accessor(format!("Accessor {idx} out of range")))?;
    if acc.component_type != FLOAT {
        return Err(GltfError::Accessor(format!(
            "Expected float component type, got {}",
            acc.component_type
        )));
    }
    let view = views
        .get(acc.buffer_view)
        .ok_or_else(|| GltfError::Accessor("BufferView out of range".into()))?;

    let stride = view.byte_stride.unwrap_or(12); // 3 * f32
    let base = view.byte_offset + acc.byte_offset;
    let mut out = Vec::with_capacity(acc.count);

    for i in 0..acc.count {
        let off = base + i * stride;
        if off + 12 > bin.len() {
            return Err(GltfError::Accessor("Read past end of BIN chunk".into()));
        }
        let x = f32_le(bin, off);
        let y = f32_le(bin, off + 4);
        let z = f32_le(bin, off + 8);
        out.push([x, y, z]);
    }

    Ok(out)
}

fn read_accessor_vec2(
    accessors: &[Accessor],
    views: &[BufferView],
    bin: &[u8],
    idx: usize,
) -> Result<Vec<[f32; 2]>, GltfError> {
    let acc = accessors
        .get(idx)
        .ok_or_else(|| GltfError::Accessor(format!("Accessor {idx} out of range")))?;
    if acc.component_type != FLOAT {
        return Err(GltfError::Accessor(format!(
            "Expected float component type, got {}",
            acc.component_type
        )));
    }
    let view = views
        .get(acc.buffer_view)
        .ok_or_else(|| GltfError::Accessor("BufferView out of range".into()))?;

    let stride = view.byte_stride.unwrap_or(8); // 2 * f32
    let base = view.byte_offset + acc.byte_offset;
    let mut out = Vec::with_capacity(acc.count);

    for i in 0..acc.count {
        let off = base + i * stride;
        if off + 8 > bin.len() {
            return Err(GltfError::Accessor("Read past end of BIN chunk".into()));
        }
        let u = f32_le(bin, off);
        let v = f32_le(bin, off + 4);
        out.push([u, v]);
    }

    Ok(out)
}

fn read_indices(
    accessors: &[Accessor],
    views: &[BufferView],
    bin: &[u8],
    idx: usize,
) -> Result<Vec<u32>, GltfError> {
    let acc = accessors
        .get(idx)
        .ok_or_else(|| GltfError::Accessor(format!("Accessor {idx} out of range")))?;
    let view = views
        .get(acc.buffer_view)
        .ok_or_else(|| GltfError::Accessor("BufferView out of range".into()))?;

    let base = view.byte_offset + acc.byte_offset;
    let mut out = Vec::with_capacity(acc.count);

    match acc.component_type {
        UNSIGNED_BYTE => {
            for i in 0..acc.count {
                let off = base + i;
                if off >= bin.len() {
                    return Err(GltfError::Accessor("Read past end of BIN chunk".into()));
                }
                out.push(bin[off] as u32);
            }
        }
        UNSIGNED_SHORT => {
            for i in 0..acc.count {
                let off = base + i * 2;
                if off + 2 > bin.len() {
                    return Err(GltfError::Accessor("Read past end of BIN chunk".into()));
                }
                out.push(u16::from_le_bytes([bin[off], bin[off + 1]]) as u32);
            }
        }
        UNSIGNED_INT => {
            for i in 0..acc.count {
                let off = base + i * 4;
                if off + 4 > bin.len() {
                    return Err(GltfError::Accessor("Read past end of BIN chunk".into()));
                }
                out.push(u32_le(bin, off));
            }
        }
        ct => {
            return Err(GltfError::Accessor(format!(
                "Unsupported index component type: {ct}"
            )));
        }
    }

    Ok(out)
}

// ── Byte reading helpers ─────────────────────────────────────────

fn u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn f32_le(data: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}
