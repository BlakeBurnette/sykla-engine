use crate::parser::PolygonFeature;
use crate::projection::CityProjection;
use sykla_world::city::CityMesh;

/// Generate flat land use meshes from parsed OSM land use features.
pub fn generate_landuse_meshes(
    features: &[PolygonFeature],
    proj: &CityProjection,
) -> Vec<CityMesh> {
    features
        .iter()
        .filter_map(|f| generate_flat_polygon(f, proj, 0.01)) // slightly above ground
        .collect()
}

/// Generate flat park meshes (same as land use but from park features).
pub fn generate_park_meshes(
    features: &[PolygonFeature],
    proj: &CityProjection,
) -> Vec<CityMesh> {
    features
        .iter()
        .filter_map(|f| generate_flat_polygon(f, proj, 0.02)) // slightly above landuse
        .collect()
}

fn generate_flat_polygon(
    feature: &PolygonFeature,
    proj: &CityProjection,
    y_offset: f32,
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

    // Build flat coordinate array for earcutr
    let mut flat_coords: Vec<f64> = Vec::new();
    let mut hole_indices: Vec<usize> = Vec::new();

    // Outer ring
    for (x, z) in &projected_outer {
        flat_coords.push(*x);
        flat_coords.push(*z);
    }

    // Inner rings (holes)
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

    // Build all vertices from flat_coords
    let total_points = flat_coords.len() / 2;
    let mut vertices = Vec::with_capacity(total_points);
    let mut normals = Vec::with_capacity(total_points);

    for i in 0..total_points {
        let x = flat_coords[i * 2] as f32;
        let z = flat_coords[i * 2 + 1] as f32;
        vertices.push([x, y_offset, z]);
        normals.push([0.0, 1.0, 0.0]);
    }

    let indices: Vec<u32> = tri_indices.iter().map(|&i| i as u32).collect();
    let color = feature.subtype.default_color();

    Some(CityMesh {
        vertices,
        normals,
        indices,
        color,
        feature_subtype: feature.subtype,
    })
}
