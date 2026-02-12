use crate::parser::RoadFeature;
use crate::projection::CityProjection;
use sykla_world::city::{CityMesh, FeatureSubtype};

/// Generate flat road strip meshes + lane marking meshes from parsed OSM road features.
pub fn generate_road_meshes(
    features: &[RoadFeature],
    proj: &CityProjection,
) -> Vec<CityMesh> {
    let mut meshes = Vec::new();
    for f in features {
        let projected = project_centerline(f, proj);
        if projected.len() < 2 {
            continue;
        }

        // Road surface
        if let Some(surface) = generate_strip(
            &projected,
            f.subtype.default_road_half_width(),
            0.05,
            f.subtype.default_color(),
            f.subtype,
        ) {
            meshes.push(surface);
        }

        // Lane markings (not for paths)
        if matches!(f.subtype, FeatureSubtype::Path) {
            continue;
        }

        // Yellow center line — solid for all non-path roads
        let line_half_width = 0.06; // 12cm wide line
        let line_y = 0.06; // above road surface
        if let Some(center) = generate_strip(
            &projected,
            line_half_width,
            line_y,
            FeatureSubtype::CenterLine.default_color(),
            FeatureSubtype::CenterLine,
        ) {
            meshes.push(center);
        }

        // White edge lines — for highways and main streets
        if matches!(f.subtype, FeatureSubtype::Highway | FeatureSubtype::MainStreet) {
            let road_hw = f.subtype.default_road_half_width();
            // Edge lines sit 0.3m inside the road edge
            let edge_offset = road_hw - 0.3;

            // Left edge line
            if let Some(left) = generate_offset_strip(
                &projected,
                edge_offset,
                line_half_width,
                line_y,
                FeatureSubtype::EdgeLine.default_color(),
                FeatureSubtype::EdgeLine,
            ) {
                meshes.push(left);
            }

            // Right edge line
            if let Some(right) = generate_offset_strip(
                &projected,
                -edge_offset,
                line_half_width,
                line_y,
                FeatureSubtype::EdgeLine.default_color(),
                FeatureSubtype::EdgeLine,
            ) {
                meshes.push(right);
            }
        }
    }
    meshes
}

/// Project a road feature's centerline to local meters.
fn project_centerline(
    feature: &RoadFeature,
    proj: &CityProjection,
) -> Vec<(f64, f64)> {
    feature
        .centerline
        .iter()
        .map(|&(lat, lng)| proj.project(lat, lng))
        .collect()
}

/// Generate a flat strip centered on the projected centerline.
fn generate_strip(
    projected: &[(f64, f64)],
    half_width: f32,
    y: f32,
    color: [f32; 4],
    subtype: FeatureSubtype,
) -> Option<CityMesh> {
    if half_width <= 0.0 || projected.len() < 2 {
        return None;
    }

    let n = projected.len();
    let mut vertices = Vec::with_capacity(n * 2);
    let mut normals = Vec::with_capacity(n * 2);
    let mut indices = Vec::with_capacity((n - 1) * 6);

    for i in 0..n {
        let (dx, dz) = direction_at(projected, i);
        let len = (dx * dx + dz * dz).sqrt();

        if len < 1e-9 {
            let (px, pz) = projected[i];
            vertices.push([px as f32 - half_width, y, pz as f32]);
            vertices.push([px as f32 + half_width, y, pz as f32]);
        } else {
            let perp_x = -dz / len * half_width as f64;
            let perp_z = dx / len * half_width as f64;
            let (cx, cz) = projected[i];
            vertices.push([(cx + perp_x) as f32, y, (cz + perp_z) as f32]);
            vertices.push([(cx - perp_x) as f32, y, (cz - perp_z) as f32]);
        }

        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
    }

    for i in 0..(n - 1) {
        let base = (i * 2) as u32;
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 1);
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    if vertices.is_empty() {
        return None;
    }

    Some(CityMesh {
        vertices,
        normals,
        indices,
        color,
        feature_subtype: subtype,
    })
}

/// Generate a strip offset laterally from the centerline (for edge lines).
/// `lateral_offset` is positive for left, negative for right.
fn generate_offset_strip(
    projected: &[(f64, f64)],
    lateral_offset: f32,
    half_width: f32,
    y: f32,
    color: [f32; 4],
    subtype: FeatureSubtype,
) -> Option<CityMesh> {
    if projected.len() < 2 {
        return None;
    }

    // Build a new centerline offset laterally from the original
    let n = projected.len();
    let mut offset_line: Vec<(f64, f64)> = Vec::with_capacity(n);

    for i in 0..n {
        let (dx, dz) = direction_at(projected, i);
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-9 {
            offset_line.push(projected[i]);
        } else {
            let perp_x = -dz / len * lateral_offset as f64;
            let perp_z = dx / len * lateral_offset as f64;
            let (cx, cz) = projected[i];
            offset_line.push((cx + perp_x, cz + perp_z));
        }
    }

    generate_strip(&offset_line, half_width, y, color, subtype)
}

/// Compute direction vector at point i along the polyline.
fn direction_at(projected: &[(f64, f64)], i: usize) -> (f64, f64) {
    let n = projected.len();
    if i == 0 {
        (projected[1].0 - projected[0].0, projected[1].1 - projected[0].1)
    } else if i == n - 1 {
        (
            projected[n - 1].0 - projected[n - 2].0,
            projected[n - 1].1 - projected[n - 2].1,
        )
    } else {
        (
            projected[i + 1].0 - projected[i - 1].0,
            projected[i + 1].1 - projected[i - 1].1,
        )
    }
}
