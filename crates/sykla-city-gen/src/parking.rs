use crate::parser::PolygonFeature;
use crate::projection::CityProjection;
use sykla_world::city::{CityMesh, FeatureSubtype};
use sykla_world::road_network::RoadNetwork;

// ---------------------------------------------------------------------------
// Car color palette
// ---------------------------------------------------------------------------

const CAR_PALETTE: [[f32; 4]; 6] = [
    [0.72, 0.12, 0.12, 1.0], // red
    [0.15, 0.25, 0.55, 1.0], // blue
    [0.90, 0.90, 0.88, 1.0], // white
    [0.12, 0.12, 0.14, 1.0], // black
    [0.70, 0.70, 0.72, 1.0], // silver
    [0.18, 0.42, 0.22, 1.0], // green
];

// Car dimensions in meters
const CAR_LENGTH: f32 = 4.5;
const CAR_WIDTH: f32 = 1.8;
const CAR_HEIGHT: f32 = 1.4;

// Parked car placement
const SAMPLE_SPACING: f32 = 8.0;
const INTERSECTION_MARGIN: f32 = 10.0;
const CAR_BASE_Y: f32 = 0.05;

// Parking lot line markings
const LINE_SPACING: f32 = 2.7;
const LINE_LENGTH: f32 = 5.0;
const LINE_HALF_WIDTH: f32 = 0.05;
const LINE_Y: f32 = 0.04;

// ---------------------------------------------------------------------------
// Parking lot meshes (flat polygon + line markings)
// ---------------------------------------------------------------------------

/// Generate flat parking lot meshes from parsed OSM parking features.
pub fn generate_parking_meshes(
    features: &[PolygonFeature],
    proj: &CityProjection,
) -> Vec<CityMesh> {
    let mut meshes = Vec::new();
    for f in features {
        if let Some(lot) = generate_flat_parking(f, proj) {
            // Generate line markings inside the lot
            let markings = generate_lot_markings(f, proj);
            meshes.push(lot);
            meshes.extend(markings);
        }
    }
    meshes
}

fn generate_flat_parking(
    feature: &PolygonFeature,
    proj: &CityProjection,
) -> Option<CityMesh> {
    let projected_outer: Vec<(f64, f64)> = proj.project_ring(&feature.ring);
    if projected_outer.len() < 3 {
        return None;
    }

    let projected_holes: Vec<Vec<(f64, f64)>> = feature
        .holes
        .iter()
        .map(|h| proj.project_ring(h))
        .collect();

    let mut flat_coords: Vec<f64> = Vec::new();
    let mut hole_indices: Vec<usize> = Vec::new();

    for (x, z) in &projected_outer {
        flat_coords.push(*x);
        flat_coords.push(*z);
    }

    for hole in &projected_holes {
        hole_indices.push(flat_coords.len() / 2);
        for (x, z) in hole {
            flat_coords.push(*x);
            flat_coords.push(*z);
        }
    }

    let tri_indices = earcutr::earcut(&flat_coords, &hole_indices, 2).ok()?;
    if tri_indices.is_empty() {
        return None;
    }

    let total_points = flat_coords.len() / 2;
    let mut vertices = Vec::with_capacity(total_points);
    let mut normals = Vec::with_capacity(total_points);

    for i in 0..total_points {
        let x = flat_coords[i * 2] as f32;
        let z = flat_coords[i * 2 + 1] as f32;
        vertices.push([x, 0.03, z]); // slightly above ground
        normals.push([0.0, 1.0, 0.0]);
    }

    let indices: Vec<u32> = tri_indices.iter().map(|&i| i as u32).collect();

    Some(CityMesh {
        vertices,
        normals,
        indices,
        color: FeatureSubtype::ParkingLot.default_color(),
        feature_subtype: FeatureSubtype::ParkingLot,
    })
}

/// Generate white parking space line markings inside a lot.
fn generate_lot_markings(
    feature: &PolygonFeature,
    proj: &CityProjection,
) -> Vec<CityMesh> {
    let projected: Vec<(f64, f64)> = proj.project_ring(&feature.ring);
    if projected.len() < 3 {
        return vec![];
    }

    // Compute bounding box
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_z = f64::MAX;
    let mut max_z = f64::MIN;
    for &(x, z) in &projected {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }

    let width = (max_x - min_x) as f32;
    let depth = (max_z - min_z) as f32;

    // Lines run along the longest axis
    let along_x = width >= depth;

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    if along_x {
        // Lines are perpendicular to X (run in Z), spaced along X
        let num_lines = ((width - 1.0) / LINE_SPACING).floor() as i32;
        let start_x = min_x as f32 + (width - num_lines as f32 * LINE_SPACING) / 2.0;

        for i in 0..=num_lines {
            let cx = start_x + i as f32 * LINE_SPACING;
            let cz = (min_z as f32 + max_z as f32) / 2.0;
            let half_len = (LINE_LENGTH / 2.0).min(depth / 2.0 - 0.5);
            if half_len < 0.5 {
                continue;
            }

            // Check that center is inside the polygon (rough check)
            if !point_in_polygon(cx as f64, cz as f64, &projected) {
                continue;
            }

            push_flat_quad(
                &mut vertices, &mut normals, &mut indices,
                cx - LINE_HALF_WIDTH, cz - half_len,
                cx + LINE_HALF_WIDTH, cz + half_len,
                LINE_Y,
            );
        }
    } else {
        // Lines are perpendicular to Z (run in X), spaced along Z
        let num_lines = ((depth - 1.0) / LINE_SPACING).floor() as i32;
        let start_z = min_z as f32 + (depth - num_lines as f32 * LINE_SPACING) / 2.0;

        for i in 0..=num_lines {
            let cz = start_z + i as f32 * LINE_SPACING;
            let cx = (min_x as f32 + max_x as f32) / 2.0;
            let half_len = (LINE_LENGTH / 2.0).min(width / 2.0 - 0.5);
            if half_len < 0.5 {
                continue;
            }

            if !point_in_polygon(cx as f64, cz as f64, &projected) {
                continue;
            }

            push_flat_quad(
                &mut vertices, &mut normals, &mut indices,
                cx - half_len, cz - LINE_HALF_WIDTH,
                cx + half_len, cz + LINE_HALF_WIDTH,
                LINE_Y,
            );
        }
    }

    if vertices.is_empty() {
        return vec![];
    }

    vec![CityMesh {
        vertices,
        normals,
        indices,
        color: FeatureSubtype::EdgeLine.default_color(),
        feature_subtype: FeatureSubtype::EdgeLine,
    }]
}

/// Simple point-in-polygon test (ray casting).
fn point_in_polygon(px: f64, pz: f64, ring: &[(f64, f64)]) -> bool {
    let n = ring.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, zi) = ring[i];
        let (xj, zj) = ring[j];
        if ((zi > pz) != (zj > pz)) && (px < (xj - xi) * (pz - zi) / (zj - zi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------------
// Parked cars along streets
// ---------------------------------------------------------------------------

/// Generate parked car box meshes along streets in the road network.
pub fn generate_parked_cars(road_network: &RoadNetwork) -> Vec<CityMesh> {
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let mut car_colors: Vec<([f32; 4], usize, usize)> = Vec::new(); // (color, vert_start, vert_count)

    for edge in &road_network.edges {
        // Only park on streets and main streets (not highways, paths, roundabouts)
        if !matches!(edge.subtype, FeatureSubtype::Street | FeatureSubtype::MainStreet) {
            continue;
        }
        if edge.is_roundabout {
            continue;
        }
        if edge.length_m < INTERSECTION_MARGIN * 2.0 + SAMPLE_SPACING {
            continue; // too short for any cars
        }

        let road_hw = edge.subtype.default_road_half_width();
        let offset_dist = road_hw + 1.5; // car center offset from road center

        // Sample positions along the edge, avoiding intersection zones
        let start_t = INTERSECTION_MARGIN / edge.length_m;
        let end_t = 1.0 - INTERSECTION_MARGIN / edge.length_m;
        let step_t = SAMPLE_SPACING / edge.length_m;

        let mut t = start_t;
        while t <= end_t {
            let pos = edge.sample_position(t);
            let dir = edge.sample_direction(t);

            // Hash position to decide skip/place (~50% occupancy)
            let h = pos_hash(pos[0], pos[1]);
            if h % 100 >= 50 {
                t += step_t;
                continue;
            }

            // Perpendicular offset (right side of road)
            let perp = [-dir[1], dir[0]];
            let cx = pos[0] + perp[0] * offset_dist;
            let cz = pos[1] + perp[1] * offset_dist;

            // Pick car color from palette
            let color_idx = (h / 100) as usize % CAR_PALETTE.len();
            let color = CAR_PALETTE[color_idx];

            let vert_start = vertices.len();
            push_oriented_box(
                &mut vertices,
                &mut normals,
                &mut indices,
                cx, cz,
                dir,
                CAR_LENGTH / 2.0,
                CAR_WIDTH / 2.0,
                CAR_HEIGHT,
            );
            let vert_count = vertices.len() - vert_start;
            car_colors.push((color, vert_start, vert_count));

            t += step_t;
        }
    }

    if vertices.is_empty() {
        return vec![];
    }

    // Group cars by color for fewer draw calls
    group_cars_by_color(vertices, normals, indices, &car_colors)
}

/// Group car meshes by color into separate CityMesh instances.
fn group_cars_by_color(
    all_verts: Vec<[f32; 3]>,
    all_norms: Vec<[f32; 3]>,
    all_indices: Vec<u32>,
    cars: &[([f32; 4], usize, usize)],
) -> Vec<CityMesh> {
    // Each car is 24 verts, 36 indices (6 faces × 2 triangles × 3 indices)
    let verts_per_car = 24;
    let indices_per_car = 36;

    // Group by quantized color
    let mut groups: std::collections::HashMap<[u8; 4], (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>, [f32; 4])> =
        std::collections::HashMap::new();

    for (car_idx, &(color, vert_start, _vert_count)) in cars.iter().enumerate() {
        let key = [
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            (color[3] * 255.0) as u8,
        ];

        let entry = groups.entry(key).or_insert_with(|| (Vec::new(), Vec::new(), Vec::new(), color));

        let idx_start = car_idx * indices_per_car;
        let idx_end = (idx_start + indices_per_car).min(all_indices.len());

        let base_offset = entry.0.len() as u32;
        let vert_base = vert_start as u32;

        // Copy vertices and normals
        let v_end = (vert_start + verts_per_car).min(all_verts.len());
        entry.0.extend_from_slice(&all_verts[vert_start..v_end]);
        entry.1.extend_from_slice(&all_norms[vert_start..v_end]);

        // Rebase indices
        for &idx in &all_indices[idx_start..idx_end] {
            entry.2.push(idx - vert_base + base_offset);
        }
    }

    groups
        .into_values()
        .filter(|(v, _, _, _)| !v.is_empty())
        .map(|(v, n, i, color)| CityMesh {
            vertices: v,
            normals: n,
            indices: i,
            color,
            feature_subtype: FeatureSubtype::ParkedCar,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Push an axis-aligned flat quad (Y-up) to the mesh buffers.
fn push_flat_quad(
    v: &mut Vec<[f32; 3]>,
    n: &mut Vec<[f32; 3]>,
    i: &mut Vec<u32>,
    x0: f32, z0: f32,
    x1: f32, z1: f32,
    y: f32,
) {
    let b = v.len() as u32;
    v.push([x0, y, z0]);
    v.push([x1, y, z0]);
    v.push([x1, y, z1]);
    v.push([x0, y, z1]);
    n.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
    i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
}

/// Push an oriented 3D box (6 faces, 24 verts, 36 indices).
///
/// The box is centered at (cx, base_y + height/2, cz), oriented along `dir`.
fn push_oriented_box(
    v: &mut Vec<[f32; 3]>,
    n: &mut Vec<[f32; 3]>,
    i: &mut Vec<u32>,
    cx: f32, cz: f32,
    dir: [f32; 2],
    half_len: f32,
    half_w: f32,
    height: f32,
) {
    let base = v.len() as u32;
    let yb = CAR_BASE_Y;
    let yt = CAR_BASE_Y + height;

    // Forward and right vectors
    let fx = dir[0];
    let fz = dir[1];
    let rx = -fz; // perpendicular right
    let rz = fx;

    // 8 corner positions
    let corners: [[f32; 3]; 8] = [
        // Bottom face (y = yb)
        [cx - fx * half_len - rx * half_w, yb, cz - fz * half_len - rz * half_w], // 0: back-left-bottom
        [cx + fx * half_len - rx * half_w, yb, cz + fz * half_len - rz * half_w], // 1: front-left-bottom
        [cx + fx * half_len + rx * half_w, yb, cz + fz * half_len + rz * half_w], // 2: front-right-bottom
        [cx - fx * half_len + rx * half_w, yb, cz - fz * half_len + rz * half_w], // 3: back-right-bottom
        // Top face (y = yt)
        [cx - fx * half_len - rx * half_w, yt, cz - fz * half_len - rz * half_w], // 4: back-left-top
        [cx + fx * half_len - rx * half_w, yt, cz + fz * half_len - rz * half_w], // 5: front-left-top
        [cx + fx * half_len + rx * half_w, yt, cz + fz * half_len + rz * half_w], // 6: front-right-top
        [cx - fx * half_len + rx * half_w, yt, cz - fz * half_len + rz * half_w], // 7: back-right-top
    ];

    // 6 faces with outward normals (4 verts each = 24 verts total)
    let faces: [([usize; 4], [f32; 3]); 6] = [
        // Front face (+forward direction)
        ([1, 2, 6, 5], [fx, 0.0, fz]),
        // Back face (-forward direction)
        ([3, 0, 4, 7], [-fx, 0.0, -fz]),
        // Right face (+right direction)
        ([2, 3, 7, 6], [rx, 0.0, rz]),
        // Left face (-right direction)
        ([0, 1, 5, 4], [-rx, 0.0, -rz]),
        // Top face (+Y)
        ([4, 5, 6, 7], [0.0, 1.0, 0.0]),
        // Bottom face (-Y)
        ([3, 2, 1, 0], [0.0, -1.0, 0.0]),
    ];

    for (corner_ids, normal) in &faces {
        let fb = v.len() as u32;
        for &ci in corner_ids {
            v.push(corners[ci]);
            n.push(*normal);
        }
        i.extend_from_slice(&[fb, fb + 1, fb + 2, fb, fb + 2, fb + 3]);
    }

    // Sanity: we added exactly 24 verts and 36 indices
    debug_assert_eq!(v.len() as u32, base + 24);
}

/// Deterministic hash from 2D position.
fn pos_hash(x: f32, z: f32) -> u32 {
    let a = (x * 73.856 + 19.143).to_bits();
    let b = (z * 37.292 + 47.857).to_bits();
    a.wrapping_mul(2654435761).wrapping_add(b.wrapping_mul(2246822519))
}
