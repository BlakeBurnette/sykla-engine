use serde::{Deserialize, Serialize};

use crate::road_network::RoadNetwork;

/// All precomputed city geometry, loaded at runtime from a .sykla binary file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CityData {
    pub name: String,
    pub center_lat: f64,
    pub center_lng: f64,
    pub buildings: Vec<CityMesh>,
    pub landuse: Vec<CityMesh>,
    pub water: Vec<CityMesh>,
    pub roads: Vec<CityMesh>,
    /// Navigable road network graph (nodes at intersections, edges as road segments).
    #[serde(default)]
    pub road_network: Option<RoadNetwork>,
}

/// A single triangulated mesh for a city feature (building, park, water body, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CityMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// RGBA color for this feature.
    pub color: [f32; 4],
    /// Subtype discriminator for batching/styling.
    pub feature_subtype: FeatureSubtype,
}

/// Feature subtypes for buildings, land use, and water.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum FeatureSubtype {
    // Buildings
    Residential = 0,
    Commercial = 1,
    Industrial = 2,
    Retail = 3,
    Public = 4,
    BuildingOther = 5,

    // Building detail elements
    WindowGlass = 6,    // dark blue-gray reflective glass
    WindowFrame = 7,    // cream/white trim around windows
    Cornice = 8,        // stone ledge between floors
    Storefront = 9,     // ground-floor glass (commercial/retail)

    // Land use
    Park = 10,
    Forest = 11,
    Farmland = 12,
    Meadow = 13,
    LanduseOther = 14,

    // Water
    Lake = 20,
    Stream = 21,
    WaterOther = 22,

    // Roads
    Highway = 30,
    MainStreet = 31,
    Street = 32,
    Path = 33,
    RoadOther = 34,

    // Lane markings
    CenterLine = 35,
    EdgeLine = 36,

    // Parking
    ParkingLot = 40,
    ParkedCar = 41,
}

impl FeatureSubtype {
    /// Default RGBA color for this feature subtype (cartoon style, saturated).
    pub fn default_color(&self) -> [f32; 4] {
        match self {
            // Buildings — brick/material tones, clearly distinct at street level
            Self::Residential => [0.76, 0.42, 0.32, 1.0],  // red brick
            Self::Commercial => [0.55, 0.58, 0.65, 1.0],    // blue-gray steel/glass
            Self::Industrial => [0.62, 0.44, 0.30, 1.0],    // dark brick / rust
            Self::Retail => [0.82, 0.68, 0.42, 1.0],        // sandstone / tan stucco
            Self::Public => [0.85, 0.78, 0.55, 1.0],        // limestone / warm stone
            Self::BuildingOther => [0.70, 0.60, 0.52, 1.0], // warm brown

            // Building detail elements
            Self::WindowGlass => [0.15, 0.20, 0.32, 1.0],   // dark blue-gray glass (unlit)
            Self::WindowFrame => [0.88, 0.85, 0.80, 1.0],    // cream/off-white
            Self::Cornice => [0.72, 0.68, 0.62, 1.0],        // warm stone
            Self::Storefront => [0.12, 0.18, 0.28, 1.0],     // dark glass

            // Land use — vivid greens
            Self::Park => [0.35, 0.75, 0.30, 1.0],
            Self::Forest => [0.18, 0.48, 0.18, 1.0],
            Self::Farmland => [0.65, 0.78, 0.40, 1.0],
            Self::Meadow => [0.50, 0.78, 0.35, 1.0],
            Self::LanduseOther => [0.55, 0.70, 0.42, 1.0],

            // Water — vivid blues
            Self::Lake => [0.15, 0.48, 0.85, 1.0],
            Self::Stream => [0.12, 0.38, 0.72, 1.0],
            Self::WaterOther => [0.18, 0.42, 0.78, 1.0],

            // Roads — dark asphalt
            Self::Highway => [0.18, 0.18, 0.20, 1.0],    // fresh dark asphalt
            Self::MainStreet => [0.22, 0.22, 0.24, 1.0],  // dark asphalt
            Self::Street => [0.28, 0.28, 0.30, 1.0],      // medium asphalt
            Self::Path => [0.58, 0.54, 0.46, 1.0],        // sandy/gravel path
            Self::RoadOther => [0.32, 0.32, 0.34, 1.0],

            // Lane markings
            Self::CenterLine => [0.90, 0.75, 0.10, 1.0],  // yellow center line
            Self::EdgeLine => [0.92, 0.92, 0.92, 1.0],     // white edge line

            // Parking
            Self::ParkingLot => [0.32, 0.32, 0.35, 1.0],   // dark gray asphalt
            Self::ParkedCar => [0.55, 0.15, 0.15, 1.0],    // default red
        }
    }

    /// Default building height in meters for this subtype.
    pub fn default_height(&self) -> f32 {
        match self {
            Self::Residential => 7.0,
            Self::Commercial => 12.0,
            Self::Industrial => 8.0,
            Self::Retail => 5.0,
            Self::Public => 10.0,
            Self::BuildingOther => 7.0,
            Self::WindowGlass | Self::WindowFrame | Self::Cornice | Self::Storefront => 0.0,
            _ => 0.0, // land use, water, and roads are flat
        }
    }

    /// Default road half-width in meters for this subtype.
    pub fn default_road_half_width(&self) -> f32 {
        match self {
            Self::Highway => 8.0,
            Self::MainStreet => 6.0,
            Self::Street => 4.0,
            Self::Path => 1.5,
            Self::RoadOther => 3.5,
            Self::CenterLine => 0.06,
            Self::EdgeLine => 0.06,
            _ => 0.0,
        }
    }
}

/// Batch multiple CityMeshes of the same subtype into a single mesh for fewer draw calls.
pub fn batch_meshes(meshes: &[CityMesh]) -> Option<CityMesh> {
    if meshes.is_empty() {
        return None;
    }

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for mesh in meshes {
        let offset = vertices.len() as u32;
        vertices.extend_from_slice(&mesh.vertices);
        normals.extend_from_slice(&mesh.normals);
        indices.extend(mesh.indices.iter().map(|i| i + offset));
    }

    Some(CityMesh {
        vertices,
        normals,
        indices,
        color: meshes[0].color,
        feature_subtype: meshes[0].feature_subtype,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_subtype_colors() {
        let color = FeatureSubtype::Residential.default_color();
        assert_eq!(color[3], 1.0);
        assert!(color[0] > 0.0);
    }

    #[test]
    fn test_feature_subtype_heights() {
        assert_eq!(FeatureSubtype::Residential.default_height(), 7.0);
        assert_eq!(FeatureSubtype::Commercial.default_height(), 12.0);
        assert_eq!(FeatureSubtype::Park.default_height(), 0.0);
    }

    #[test]
    fn test_batch_meshes_empty() {
        assert!(batch_meshes(&[]).is_none());
    }

    #[test]
    fn test_batch_meshes_combines() {
        let m1 = CityMesh {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            color: [0.5, 0.5, 0.5, 1.0],
            feature_subtype: FeatureSubtype::Residential,
        };
        let m2 = CityMesh {
            vertices: vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 0.0, 1.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            color: [0.5, 0.5, 0.5, 1.0],
            feature_subtype: FeatureSubtype::Residential,
        };

        let batched = batch_meshes(&[m1, m2]).unwrap();
        assert_eq!(batched.vertices.len(), 6);
        assert_eq!(batched.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_city_data_roundtrip() {
        let city = CityData {
            name: "test".to_string(),
            center_lat: 35.78,
            center_lng: -78.80,
            buildings: vec![],
            landuse: vec![],
            water: vec![],
            roads: vec![],
            road_network: None,
        };
        let bytes = bincode::serialize(&city).unwrap();
        let decoded: CityData = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded.name, "test");
        assert_eq!(decoded.center_lat, 35.78);
    }
}
