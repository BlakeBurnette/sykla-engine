/// Equirectangular projection from lat/lng to local meters.
///
/// Centered on a reference point; accurate for city-scale areas (~20 km).
/// X = east (positive), Y = up (elevation), Z = north (positive).
#[derive(Debug, Clone)]
pub struct LocalProjection {
    pub center_lat: f64,
    pub center_lng: f64,
    cos_center_lat: f64,
}

/// Meters per degree of latitude (approximate, constant).
const METERS_PER_DEGREE: f64 = 111_320.0;

impl LocalProjection {
    pub fn new(center_lat: f64, center_lng: f64) -> Self {
        Self {
            center_lat,
            center_lng,
            cos_center_lat: center_lat.to_radians().cos(),
        }
    }

    /// Project lat/lng to local meters: (x_east, z_north).
    pub fn project(&self, lat: f64, lng: f64) -> (f64, f64) {
        let x = (lng - self.center_lng) * METERS_PER_DEGREE * self.cos_center_lat;
        let z = (lat - self.center_lat) * METERS_PER_DEGREE;
        (x, z)
    }

    /// Project lat/lng/elevation to [x, y, z] array (f32) for mesh vertices.
    /// Y = elevation, X = east, Z = north.
    pub fn project_3d(&self, lat: f64, lng: f64, elevation: f64) -> [f32; 3] {
        let (x, z) = self.project(lat, lng);
        [x as f32, elevation as f32, z as f32]
    }

    /// Project a slice of (lat, lng) coordinate pairs to local meters.
    pub fn project_ring(&self, coords: &[(f64, f64)]) -> Vec<(f64, f64)> {
        coords.iter().map(|&(lat, lng)| self.project(lat, lng)).collect()
    }

    /// Inverse: local meters back to lat/lng.
    pub fn unproject(&self, x: f64, z: f64) -> (f64, f64) {
        let lng = x / (METERS_PER_DEGREE * self.cos_center_lat) + self.center_lng;
        let lat = z / METERS_PER_DEGREE + self.center_lat;
        (lat, lng)
    }
}

/// Project a RoutePoint to [x, y, z] in local meters.
pub fn project_route_point(
    proj: &LocalProjection,
    point: &crate::types::RoutePoint,
) -> [f32; 3] {
    proj.project_3d(point.lat, point.lng, point.elevation_m)
}

/// Project all route points to local meters coordinates.
pub fn project_route(
    proj: &LocalProjection,
    route: &crate::types::Route,
) -> Vec<[f32; 3]> {
    route.points.iter().map(|p| project_route_point(proj, p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_projects_to_origin() {
        let proj = LocalProjection::new(35.78, -78.80);
        let (x, z) = proj.project(35.78, -78.80);
        assert!(x.abs() < 0.01);
        assert!(z.abs() < 0.01);
    }

    #[test]
    fn test_north_is_positive_z() {
        let proj = LocalProjection::new(35.78, -78.80);
        let (_, z) = proj.project(35.79, -78.80); // slightly north
        assert!(z > 0.0, "North should be positive Z, got {z}");
    }

    #[test]
    fn test_east_is_positive_x() {
        let proj = LocalProjection::new(35.78, -78.80);
        let (x, _) = proj.project(35.78, -78.79); // slightly east
        assert!(x > 0.0, "East should be positive X, got {x}");
    }

    #[test]
    fn test_approximate_distance() {
        let proj = LocalProjection::new(35.78, -78.80);
        // 0.01 degree of latitude ~ 1113 meters
        let (_, z) = proj.project(35.79, -78.80);
        assert!((z - 1113.2).abs() < 5.0, "Expected ~1113m, got {z}");
    }

    #[test]
    fn test_roundtrip() {
        let proj = LocalProjection::new(35.78, -78.80);
        let (x, z) = proj.project(35.785, -78.795);
        let (lat, lng) = proj.unproject(x, z);
        assert!((lat - 35.785).abs() < 1e-6);
        assert!((lng - (-78.795)).abs() < 1e-6);
    }

    #[test]
    fn test_project_3d() {
        let proj = LocalProjection::new(35.78, -78.80);
        let pos = proj.project_3d(35.78, -78.80, 100.0);
        assert!(pos[0].abs() < 0.01); // x ~ 0
        assert!((pos[1] - 100.0).abs() < 0.01); // y = elevation
        assert!(pos[2].abs() < 0.01); // z ~ 0
    }
}
