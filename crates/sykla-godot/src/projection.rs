use godot::prelude::*;
use sykla_core::projection::LocalProjection;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct SyklaProjection {
    projection: Option<LocalProjection>,
}

#[godot_api]
impl IRefCounted for SyklaProjection {
    fn init(_base: Base<RefCounted>) -> Self {
        Self { projection: None }
    }
}

#[godot_api]
impl SyklaProjection {
    /// Initialize the projection centered on a lat/lng.
    #[func]
    fn set_center(&mut self, lat: f64, lng: f64) {
        self.projection = Some(LocalProjection::new(lat, lng));
    }

    /// Project lat/lng/elevation to Godot Vector3 (x=east, y=up, z=north).
    #[func]
    fn project_3d(&self, lat: f64, lng: f64, elevation: f64) -> Vector3 {
        match &self.projection {
            Some(proj) => {
                let p = proj.project_3d(lat, lng, elevation);
                Vector3::new(p[0], p[1], p[2])
            }
            None => {
                godot_error!("SyklaProjection::project_3d: call set_center() first");
                Vector3::ZERO
            }
        }
    }

    /// Inverse projection: local (x, z) back to (lat, lng) as Vector2.
    #[func]
    fn unproject(&self, x: f64, z: f64) -> Vector2 {
        match &self.projection {
            Some(proj) => {
                let (lat, lng) = proj.unproject(x, z);
                Vector2::new(lat as f32, lng as f32)
            }
            None => {
                godot_error!("SyklaProjection::unproject: call set_center() first");
                Vector2::ZERO
            }
        }
    }
}
