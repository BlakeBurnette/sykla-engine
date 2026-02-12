use crate::parser::BuildingFeature;
use crate::projection::CityProjection;
use sykla_world::city::{CityMesh, FeatureSubtype};

// ---------------------------------------------------------------------------
// Color palettes — 4 variants per building subtype
// ---------------------------------------------------------------------------

const RESIDENTIAL_PALETTE: [[f32; 4]; 4] = [
    [0.76, 0.42, 0.32, 1.0], // red brick
    [0.78, 0.65, 0.50, 1.0], // tan brick
    [0.88, 0.84, 0.76, 1.0], // cream stucco
    [0.58, 0.42, 0.30, 1.0], // brown brick
];

const COMMERCIAL_PALETTE: [[f32; 4]; 4] = [
    [0.55, 0.58, 0.65, 1.0], // blue-gray steel
    [0.70, 0.70, 0.72, 1.0], // light gray
    [0.48, 0.50, 0.55, 1.0], // charcoal
    [0.75, 0.73, 0.70, 1.0], // warm gray
];

const INDUSTRIAL_PALETTE: [[f32; 4]; 4] = [
    [0.62, 0.44, 0.30, 1.0], // dark brick / rust
    [0.55, 0.52, 0.48, 1.0], // concrete
    [0.65, 0.60, 0.55, 1.0], // light concrete
    [0.45, 0.42, 0.40, 1.0], // dark concrete
];

const RETAIL_PALETTE: [[f32; 4]; 4] = [
    [0.82, 0.68, 0.42, 1.0], // sandstone
    [0.85, 0.80, 0.70, 1.0], // cream
    [0.72, 0.68, 0.60, 1.0], // stone
    [0.88, 0.82, 0.65, 1.0], // pale gold
];

const PUBLIC_PALETTE: [[f32; 4]; 4] = [
    [0.85, 0.78, 0.55, 1.0], // limestone
    [0.80, 0.75, 0.65, 1.0], // warm stone
    [0.75, 0.70, 0.60, 1.0], // darker stone
    [0.88, 0.82, 0.72, 1.0], // light limestone
];

const OTHER_PALETTE: [[f32; 4]; 4] = [
    [0.70, 0.60, 0.52, 1.0], // warm brown
    [0.65, 0.58, 0.48, 1.0], // medium brown
    [0.75, 0.68, 0.58, 1.0], // light brown
    [0.68, 0.62, 0.52, 1.0], // walnut
];

fn pick_wall_color(subtype: FeatureSubtype, hash: u32) -> [f32; 4] {
    let palette = match subtype {
        FeatureSubtype::Residential => &RESIDENTIAL_PALETTE[..],
        FeatureSubtype::Commercial => &COMMERCIAL_PALETTE[..],
        FeatureSubtype::Industrial => &INDUSTRIAL_PALETTE[..],
        FeatureSubtype::Retail => &RETAIL_PALETTE[..],
        FeatureSubtype::Public => &PUBLIC_PALETTE[..],
        _ => &OTHER_PALETTE[..],
    };
    palette[(hash % palette.len() as u32) as usize]
}

fn darken(c: [f32; 4], f: f32) -> [f32; 4] {
    [c[0] * f, c[1] * f, c[2] * f, c[3]]
}

fn centroid_hash(projected: &[(f64, f64)]) -> u32 {
    let n = projected.len() as f64;
    let cx: f64 = projected.iter().map(|(x, _)| x).sum::<f64>() / n;
    let cz: f64 = projected.iter().map(|(_, z)| z).sum::<f64>() / n;
    let a = ((cx * 73.856 + 19.143) as f32).to_bits();
    let b = ((cz * 37.292 + 47.857) as f32).to_bits();
    a.wrapping_mul(2654435761).wrapping_add(b.wrapping_mul(2246822519))
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const GROUND_FLOOR_HEIGHT: f32 = 4.0;
const UPPER_FLOOR_HEIGHT: f32 = 3.5;
const BAY_WIDTH: f32 = 3.0;
const MAX_BAYS: usize = 8;

/// Window fills 40% of bay width × 45% of floor height — small punched windows.
/// At residential scale (7m, 2 floors, ~3m bays) this gives ~1.2m × 1.6m windows.
const WINDOW_WIDTH_FRAC: f32 = 0.40;
const WINDOW_HEIGHT_FRAC: f32 = 0.45;

/// Storefront fills 80% of bay width × 70% of ground floor height.
const STOREFRONT_WIDTH_FRAC: f32 = 0.80;
const STOREFRONT_HEIGHT_FRAC: f32 = 0.70;

const MIN_WALL_WIDTH_FOR_WINDOWS: f32 = 2.5;
const CORNICE_HEIGHT: f32 = 0.30;
const CORNICE_PROTRUSION: f32 = 0.12;
const PARAPET_HEIGHT_COMMERCIAL: f32 = 1.0;
const PARAPET_HEIGHT_RETAIL: f32 = 0.5;
const GABLE_PITCH_DEG: f32 = 25.0;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn generate_building_meshes(
    features: &[BuildingFeature],
    proj: &CityProjection,
) -> Vec<CityMesh> {
    features
        .iter()
        .flat_map(|feature| generate_single_building(feature, proj))
        .collect()
}

// ---------------------------------------------------------------------------
// Per-building generation
// ---------------------------------------------------------------------------

/// Generate all meshes for one building using non-overlapping wall strips.
///
/// Each floor band on each wall is decomposed into:
///   top strip (wall) | window row: [pilaster, glass, pilaster, glass, ...] | bottom strip (wall)
///
/// Zero overlap between wall and glass geometry → no z-fighting.
fn generate_single_building(
    feature: &BuildingFeature,
    proj: &CityProjection,
) -> Vec<CityMesh> {
    let mut projected: Vec<(f64, f64)> = proj.project_ring(&feature.ring);
    if projected.len() < 3 {
        return vec![];
    }
    // Ensure counter-clockwise winding so wall normals point outward.
    ensure_ccw(&mut projected);

    let height = feature.height;
    let subtype = feature.subtype;
    let hash = centroid_hash(&projected);
    let wall_color = pick_wall_color(subtype, hash);

    // Deduplicate closing vertex
    let n = projected.len();
    let ring_len = if n > 1
        && (projected[0].0 - projected[n - 1].0).abs() < 1e-6
        && (projected[0].1 - projected[n - 1].1).abs() < 1e-6
    {
        n - 1
    } else {
        n
    };
    let ring = &projected[..ring_len];

    let num_floors = compute_num_floors(height);
    let floor_heights = compute_floor_heights(num_floors, height);
    let is_commercial_ground = matches!(subtype, FeatureSubtype::Commercial | FeatureSubtype::Retail);

    let mut wall_v: Vec<[f32; 3]> = Vec::new();
    let mut wall_n: Vec<[f32; 3]> = Vec::new();
    let mut wall_i: Vec<u32> = Vec::new();

    let mut glass_v: Vec<[f32; 3]> = Vec::new();
    let mut glass_n: Vec<[f32; 3]> = Vec::new();
    let mut glass_i: Vec<u32> = Vec::new();

    let mut cornice_v: Vec<[f32; 3]> = Vec::new();
    let mut cornice_n: Vec<[f32; 3]> = Vec::new();
    let mut cornice_i: Vec<u32> = Vec::new();

    let mut store_v: Vec<[f32; 3]> = Vec::new();
    let mut store_n: Vec<[f32; 3]> = Vec::new();
    let mut store_i: Vec<u32> = Vec::new();

    for edge_idx in 0..ring_len {
        let j = (edge_idx + 1) % ring_len;
        let (x0, z0) = (ring[edge_idx].0 as f32, ring[edge_idx].1 as f32);
        let (x1, z1) = (ring[j].0 as f32, ring[j].1 as f32);

        let dx = x1 - x0;
        let dz = z1 - z0;
        let wall_width = (dx * dx + dz * dz).sqrt();
        if wall_width < 0.1 {
            continue;
        }

        let nx = dz / wall_width;
        let nz = -dx / wall_width;
        let normal = [nx, 0.0, nz];
        let tx = dx / wall_width;
        let tz = dz / wall_width;

        // Narrow wall — solid quad, no windows
        if wall_width < MIN_WALL_WIDTH_FOR_WINDOWS {
            push_quad(&mut wall_v, &mut wall_n, &mut wall_i,
                x0, z0, x1, z1, 0.0, height, normal);
            continue;
        }

        let num_bays = ((wall_width / BAY_WIDTH).round() as usize).clamp(1, MAX_BAYS);
        let bay_w = wall_width / num_bays as f32;

        for (floor_idx, &(fb, ft)) in floor_heights.iter().enumerate() {
            let fh = ft - fb;
            let is_ground = floor_idx == 0;

            // Choose window vs storefront dimensions
            let (win_w_frac, win_h_frac, target_glass, target_store) = if is_ground && is_commercial_ground {
                (STOREFRONT_WIDTH_FRAC, STOREFRONT_HEIGHT_FRAC, false, true)
            } else {
                (WINDOW_WIDTH_FRAC, WINDOW_HEIGHT_FRAC, true, false)
            };

            let win_w = bay_w * win_w_frac;
            let win_h = fh * win_h_frac;
            let h_margin = (bay_w - win_w) / 2.0;
            let v_margin = (fh - win_h) / 2.0;
            let win_bot = fb + v_margin;
            let win_top = fb + v_margin + win_h;

            // --- Bottom wall strip (full wall width, below windows) ---
            emit_wall_strip(&mut wall_v, &mut wall_n, &mut wall_i,
                x0, z0, tx, tz, 0.0, wall_width, fb, win_bot, normal);

            // --- Top wall strip (full wall width, above windows) ---
            emit_wall_strip(&mut wall_v, &mut wall_n, &mut wall_i,
                x0, z0, tx, tz, 0.0, wall_width, win_top, ft, normal);

            // --- Window row: alternating pilasters and glass ---
            for bay in 0..num_bays {
                let bay_start = bay as f32 * bay_w;

                // Left pilaster (wall strip before window)
                emit_wall_strip(&mut wall_v, &mut wall_n, &mut wall_i,
                    x0, z0, tx, tz,
                    bay_start, bay_start + h_margin,
                    win_bot, win_top, normal);

                // Glass/storefront quad
                let glass_buf = if target_store { &mut store_v } else { &mut glass_v };
                let glass_nbuf = if target_store { &mut store_n } else { &mut glass_n };
                let glass_ibuf = if target_store { &mut store_i } else { &mut glass_i };

                let gl = bay_start + h_margin;
                let gr = bay_start + h_margin + win_w;
                let gx0 = x0 + tx * gl;
                let gz0 = z0 + tz * gl;
                let gx1 = x0 + tx * gr;
                let gz1 = z0 + tz * gr;
                push_quad(glass_buf, glass_nbuf, glass_ibuf,
                    gx0, gz0, gx1, gz1, win_bot, win_top, normal);

                // Right pilaster (wall strip after window)
                emit_wall_strip(&mut wall_v, &mut wall_n, &mut wall_i,
                    x0, z0, tx, tz,
                    bay_start + h_margin + win_w, (bay + 1) as f32 * bay_w,
                    win_bot, win_top, normal);
            }
        }

        // --- Cornices ---
        // Floor-line cornices (multi-story only)
        if num_floors >= 2 {
            for fi in 1..floor_heights.len() {
                push_cornice(&mut cornice_v, &mut cornice_n, &mut cornice_i,
                    x0, z0, x1, z1, floor_heights[fi].0, nx, nz,
                    CORNICE_HEIGHT, CORNICE_PROTRUSION, normal);
            }
        }
        // Roofline cornice
        push_cornice(&mut cornice_v, &mut cornice_n, &mut cornice_i,
            x0, z0, x1, z1, height, nx, nz,
            CORNICE_HEIGHT, CORNICE_PROTRUSION, normal);
    }

    // --- Roof ---
    let roof_meshes = generate_roof(ring, height, subtype, wall_color);

    // --- Assemble ---
    let mut result: Vec<CityMesh> = Vec::new();

    if !wall_v.is_empty() {
        result.push(CityMesh {
            vertices: wall_v, normals: wall_n, indices: wall_i,
            color: wall_color,
            feature_subtype: subtype,
        });
    }
    if !glass_v.is_empty() {
        result.push(CityMesh {
            vertices: glass_v, normals: glass_n, indices: glass_i,
            color: FeatureSubtype::WindowGlass.default_color(),
            feature_subtype: FeatureSubtype::WindowGlass,
        });
    }
    if !cornice_v.is_empty() {
        result.push(CityMesh {
            vertices: cornice_v, normals: cornice_n, indices: cornice_i,
            color: FeatureSubtype::Cornice.default_color(),
            feature_subtype: FeatureSubtype::Cornice,
        });
    }
    if !store_v.is_empty() {
        result.push(CityMesh {
            vertices: store_v, normals: store_n, indices: store_i,
            color: FeatureSubtype::Storefront.default_color(),
            feature_subtype: FeatureSubtype::Storefront,
        });
    }

    result.extend(roof_meshes);
    result
}

// ---------------------------------------------------------------------------
// Floor helpers
// ---------------------------------------------------------------------------

fn compute_num_floors(height: f32) -> usize {
    if height <= GROUND_FLOOR_HEIGHT + 0.5 {
        1
    } else {
        1 + ((height - GROUND_FLOOR_HEIGHT) / UPPER_FLOOR_HEIGHT).round() as usize
    }
}

fn compute_floor_heights(num_floors: usize, total_height: f32) -> Vec<(f32, f32)> {
    let mut floors = Vec::with_capacity(num_floors);
    if num_floors == 1 {
        floors.push((0.0, total_height));
    } else {
        floors.push((0.0, GROUND_FLOOR_HEIGHT));
        let remaining = total_height - GROUND_FLOOR_HEIGHT;
        let upper_h = remaining / (num_floors - 1) as f32;
        for i in 0..(num_floors - 1) {
            let base = GROUND_FLOOR_HEIGHT + i as f32 * upper_h;
            floors.push((base, base + upper_h));
        }
    }
    floors
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Emit a wall strip along a wall edge from parametric t0 to t1.
fn emit_wall_strip(
    verts: &mut Vec<[f32; 3]>,
    norms: &mut Vec<[f32; 3]>,
    idxs: &mut Vec<u32>,
    wall_x0: f32, wall_z0: f32,
    tx: f32, tz: f32,
    t0: f32, t1: f32,
    y_bot: f32, y_top: f32,
    normal: [f32; 3],
) {
    if (t1 - t0) < 0.001 || (y_top - y_bot) < 0.001 {
        return;
    }
    let sx0 = wall_x0 + tx * t0;
    let sz0 = wall_z0 + tz * t0;
    let sx1 = wall_x0 + tx * t1;
    let sz1 = wall_z0 + tz * t1;
    push_quad(verts, norms, idxs, sx0, sz0, sx1, sz1, y_bot, y_top, normal);
}

fn push_quad(
    v: &mut Vec<[f32; 3]>, n: &mut Vec<[f32; 3]>, i: &mut Vec<u32>,
    x0: f32, z0: f32, x1: f32, z1: f32,
    yb: f32, yt: f32, normal: [f32; 3],
) {
    if (yt - yb).abs() < 0.001 { return; }
    let b = v.len() as u32;
    v.push([x0, yb, z0]);
    v.push([x1, yb, z1]);
    v.push([x1, yt, z1]);
    v.push([x0, yt, z0]);
    n.extend_from_slice(&[normal; 4]);
    i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
}

/// Cornice: front face + top face (2 quads). Creates a visible ledge with shadow.
fn push_cornice(
    v: &mut Vec<[f32; 3]>, n: &mut Vec<[f32; 3]>, i: &mut Vec<u32>,
    x0: f32, z0: f32, x1: f32, z1: f32,
    y_center: f32, nx: f32, nz: f32,
    height: f32, protrusion: f32,
    _wall_normal: [f32; 3],
) {
    let hh = height / 2.0;
    let yb = y_center - hh;
    let yt = y_center + hh;
    let ox0 = x0 + nx * protrusion;
    let oz0 = z0 + nz * protrusion;
    let ox1 = x1 + nx * protrusion;
    let oz1 = z1 + nz * protrusion;

    // Front face
    let b = v.len() as u32;
    v.push([ox0, yb, oz0]); v.push([ox1, yb, oz1]);
    v.push([ox1, yt, oz1]); v.push([ox0, yt, oz0]);
    n.extend_from_slice(&[[nx, 0.0, nz]; 4]);
    i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);

    // Top face
    let b = v.len() as u32;
    v.push([x0, yt, z0]); v.push([x1, yt, z1]);
    v.push([ox1, yt, oz1]); v.push([ox0, yt, oz0]);
    n.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
    i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
}

fn push_triangle(
    v: &mut Vec<[f32; 3]>, n: &mut Vec<[f32; 3]>, i: &mut Vec<u32>,
    a: [f32; 3], b: [f32; 3], c: [f32; 3], normal: [f32; 3],
) {
    let base = v.len() as u32;
    v.push(a); v.push(b); v.push(c);
    n.extend_from_slice(&[normal; 3]);
    i.extend_from_slice(&[base, base + 1, base + 2]);
}

fn slope_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ab = [b[0]-a[0], b[1]-a[1], b[2]-a[2]];
    let ac = [c[0]-a[0], c[1]-a[1], c[2]-a[2]];
    let x = ab[1]*ac[2] - ab[2]*ac[1];
    let y = ab[2]*ac[0] - ab[0]*ac[2];
    let z = ab[0]*ac[1] - ab[1]*ac[0];
    let len = (x*x + y*y + z*z).sqrt();
    if len > 0.0001 { [x/len, y/len, z/len] } else { [0.0, 1.0, 0.0] }
}

// ---------------------------------------------------------------------------
// Roof generation
// ---------------------------------------------------------------------------

fn generate_roof(
    ring: &[(f64, f64)], height: f32, subtype: FeatureSubtype, wall_color: [f32; 4],
) -> Vec<CityMesh> {
    match subtype {
        FeatureSubtype::Residential | FeatureSubtype::BuildingOther | FeatureSubtype::Public =>
            generate_gable_roof(ring, height, wall_color),
        FeatureSubtype::Commercial =>
            generate_parapet_roof(ring, height, wall_color, PARAPET_HEIGHT_COMMERCIAL),
        FeatureSubtype::Retail =>
            generate_parapet_roof(ring, height, wall_color, PARAPET_HEIGHT_RETAIL),
        _ => generate_flat_roof(ring, height, wall_color),
    }
}

fn generate_flat_roof(ring: &[(f64, f64)], h: f32, wc: [f32; 4]) -> Vec<CityMesh> {
    let flat: Vec<f64> = ring.iter().flat_map(|(x, z)| [*x, *z]).collect();
    let tri = match earcutr::earcut(&flat, &[], 2) {
        Ok(idx) if !idx.is_empty() => idx,
        _ => return vec![],
    };
    let rc = darken(wc, 0.70);
    let mut verts = Vec::with_capacity(ring.len());
    let mut norms = Vec::with_capacity(ring.len());
    for &(x, z) in ring {
        verts.push([x as f32, h, z as f32]);
        norms.push([0.0, 1.0, 0.0]);
    }
    vec![CityMesh {
        vertices: verts, normals: norms,
        indices: tri.iter().map(|&i| i as u32).collect(),
        color: rc, feature_subtype: FeatureSubtype::BuildingOther,
    }]
}

fn generate_gable_roof(ring: &[(f64, f64)], height: f32, wc: [f32; 4]) -> Vec<CityMesh> {
    let rc = darken(wc, 0.70);
    let (mut mnx, mut mxx, mut mnz, mut mxz) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, z) in ring {
        mnx = mnx.min(x); mxx = mxx.max(x);
        mnz = mnz.min(z); mxz = mxz.max(z);
    }
    let wx = (mxx - mnx) as f32;
    let wz = (mxz - mnz) as f32;
    let pitch = GABLE_PITCH_DEG * std::f32::consts::PI / 180.0;
    let (along_x, perp_half) = if wx >= wz { (true, wz / 2.0) } else { (false, wx / 2.0) };
    let ry = height + perp_half * pitch.tan();
    let cx = ((mnx + mxx) / 2.0) as f32;
    let cz = ((mnz + mxz) / 2.0) as f32;

    let mut v = Vec::new();
    let mut n = Vec::new();
    let mut ix = Vec::new();

    let (mnx, mxx, mnz, mxz) = (mnx as f32, mxx as f32, mnz as f32, mxz as f32);

    if along_x {
        let r0 = [mnx, ry, cz]; let r1 = [mxx, ry, cz];
        let nw = [mnx, height, mnz]; let ne = [mxx, height, mnz];
        let se = [mxx, height, mxz]; let sw = [mnx, height, mxz];
        let sn = slope_normal(r0, r1, nw);
        push_triangle(&mut v, &mut n, &mut ix, r0, r1, ne, sn);
        push_triangle(&mut v, &mut n, &mut ix, r0, ne, nw, sn);
        let ss = slope_normal(r1, r0, se);
        push_triangle(&mut v, &mut n, &mut ix, r1, r0, sw, ss);
        push_triangle(&mut v, &mut n, &mut ix, r1, sw, se, ss);
        push_triangle(&mut v, &mut n, &mut ix, r0, nw, sw, [-1.0, 0.0, 0.0]);
        push_triangle(&mut v, &mut n, &mut ix, r1, se, ne, [1.0, 0.0, 0.0]);
    } else {
        let r0 = [cx, ry, mnz]; let r1 = [cx, ry, mxz];
        let nw = [mnx, height, mnz]; let ne = [mxx, height, mnz];
        let se = [mxx, height, mxz]; let sw = [mnx, height, mxz];
        let sl = slope_normal(r1, r0, sw);
        push_triangle(&mut v, &mut n, &mut ix, r1, r0, nw, sl);
        push_triangle(&mut v, &mut n, &mut ix, r1, nw, sw, sl);
        let sr = slope_normal(r0, r1, ne);
        push_triangle(&mut v, &mut n, &mut ix, r0, r1, se, sr);
        push_triangle(&mut v, &mut n, &mut ix, r0, se, ne, sr);
        push_triangle(&mut v, &mut n, &mut ix, r0, ne, nw, [0.0, 0.0, -1.0]);
        push_triangle(&mut v, &mut n, &mut ix, r1, sw, se, [0.0, 0.0, 1.0]);
    }

    if v.is_empty() { return vec![]; }
    vec![CityMesh { vertices: v, normals: n, indices: ix, color: rc,
        feature_subtype: FeatureSubtype::BuildingOther }]
}

fn generate_parapet_roof(
    ring: &[(f64, f64)], height: f32, wc: [f32; 4], ph: f32,
) -> Vec<CityMesh> {
    let mut meshes = generate_flat_roof(ring, height + ph, wc);
    let pc = darken(wc, 0.80);
    let mut v = Vec::new(); let mut n = Vec::new(); let mut i = Vec::new();
    for idx in 0..ring.len() {
        let j = (idx + 1) % ring.len();
        let (x0, z0) = (ring[idx].0 as f32, ring[idx].1 as f32);
        let (x1, z1) = (ring[j].0 as f32, ring[j].1 as f32);
        let dx = x1-x0; let dz = z1-z0;
        let len = (dx*dx + dz*dz).sqrt();
        if len < 0.01 { continue; }
        push_quad(&mut v, &mut n, &mut i, x0, z0, x1, z1,
            height, height + ph, [dz/len, 0.0, -dx/len]);
    }
    if !v.is_empty() {
        meshes.push(CityMesh { vertices: v, normals: n, indices: i,
            color: pc, feature_subtype: FeatureSubtype::BuildingOther });
    }
    meshes
}

// ---------------------------------------------------------------------------
// Polygon helpers
// ---------------------------------------------------------------------------

pub fn ensure_ccw(ring: &mut Vec<(f64, f64)>) {
    let area = signed_area(ring);
    if area > 0.0 { ring.reverse(); }
}

fn signed_area(ring: &[(f64, f64)]) -> f64 {
    let n = ring.len();
    let mut a = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        a += ring[i].0 * ring[j].1 - ring[j].0 * ring[i].1;
    }
    a / 2.0
}
