use sykla_core::projection::LocalProjection;

/// Wrapper around sykla-core's LocalProjection for city generation.
pub struct CityProjection {
    inner: LocalProjection,
}

impl CityProjection {
    pub fn new(center_lat: f64, center_lng: f64) -> Self {
        Self {
            inner: LocalProjection::new(center_lat, center_lng),
        }
    }

    /// Project a ring of (lat, lng) coords to local meters (x, z) pairs.
    pub fn project_ring(&self, coords: &[(f64, f64)]) -> Vec<(f64, f64)> {
        self.inner.project_ring(coords)
    }

    /// Project a single (lat, lng) to (x, z) in local meters.
    pub fn project(&self, lat: f64, lng: f64) -> (f64, f64) {
        self.inner.project(lat, lng)
    }
}
