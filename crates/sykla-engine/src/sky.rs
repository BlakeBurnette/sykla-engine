use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::camera::Camera;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SkyUniforms {
    pub inv_view_proj: [[f32; 4]; 4],
    pub sun_direction: [f32; 4],
    pub sun_color: [f32; 4],
    pub sky_zenith: [f32; 4],
    pub sky_horizon: [f32; 4],
}

pub struct SkyState {
    pub time_of_day: f32,
    pub latitude: f32,
    pub cloud_coverage: f32,
    pub haze_density: f32,
}

impl SkyState {
    pub fn new(time_of_day: f32, latitude: f32) -> Self {
        Self {
            time_of_day,
            latitude,
            cloud_coverage: 0.3,
            haze_density: 0.15,
        }
    }

    pub fn uniforms(&self, camera: &Camera) -> SkyUniforms {
        let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
        let proj = Mat4::perspective_rh(camera.fov_y, camera.aspect, camera.near, camera.far);
        let inv_view_proj = (proj * view).inverse();

        let sun_dir = sun_direction(self.time_of_day, self.latitude);
        let sun_elevation = sun_dir.y;
        let (zenith, horizon, sun_color) = sky_colors(sun_elevation);

        SkyUniforms {
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            sun_direction: [sun_dir.x, sun_dir.y, sun_dir.z, 0.0],
            sun_color: [sun_color.x, sun_color.y, sun_color.z, 1.0],
            sky_zenith: [zenith.x, zenith.y, zenith.z, self.cloud_coverage],
            sky_horizon: [horizon.x, horizon.y, horizon.z, self.haze_density],
        }
    }
}

/// Simplified solar position from time of day and latitude.
/// Returns a normalized direction vector pointing TOWARD the sun.
pub fn sun_direction(time_of_day: f32, latitude_deg: f32) -> Vec3 {
    let lat = latitude_deg.to_radians();
    // Simplified declination (assume near summer solstice for nice lighting)
    let declination = 23.44_f32.to_radians() * 0.7;

    // Hour angle: 0 at noon, negative morning, positive afternoon
    let hour_angle = ((time_of_day - 12.0) * 15.0).to_radians();

    // Solar elevation
    let sin_elev = lat.sin() * declination.sin() + lat.cos() * declination.cos() * hour_angle.cos();
    let elev = sin_elev.asin();

    // Solar azimuth (simplified)
    let cos_az = (declination.sin() - lat.sin() * sin_elev) / (lat.cos() * elev.cos() + 0.001);
    let az = cos_az.clamp(-1.0, 1.0).acos();
    let az = if hour_angle > 0.0 { az } else { -az };

    Vec3::new(
        -az.sin() * elev.cos(),
        elev.sin(),
        -az.cos() * elev.cos(),
    )
    .normalize()
}

/// Returns (zenith_color, horizon_color, sun_color) based on sun elevation.
pub fn sky_colors(sun_elevation: f32) -> (Vec3, Vec3, Vec3) {
    // Sun below horizon → dusk/dawn colors
    if sun_elevation < 0.0 {
        let t = (sun_elevation / -0.3).clamp(0.0, 1.0); // 0=horizon, 1=deep night
        let zenith = Vec3::lerp(Vec3::new(0.15, 0.15, 0.35), Vec3::new(0.02, 0.02, 0.08), t);
        let horizon = Vec3::lerp(Vec3::new(0.6, 0.35, 0.2), Vec3::new(0.05, 0.05, 0.1), t);
        let sun = Vec3::lerp(Vec3::new(1.0, 0.4, 0.1), Vec3::new(0.1, 0.05, 0.02), t);
        return (zenith, horizon, sun);
    }

    // Low sun (golden hour): elevation 0-15 degrees
    let low_t = (sun_elevation / 0.26).clamp(0.0, 1.0); // 0.26 rad ≈ 15 deg

    let zenith_low = Vec3::new(0.25, 0.35, 0.65);
    let zenith_high = Vec3::new(0.15, 0.30, 0.65);
    let zenith = Vec3::lerp(zenith_low, zenith_high, low_t);

    let horizon_low = Vec3::new(0.85, 0.65, 0.45);
    let horizon_high = Vec3::new(0.65, 0.72, 0.82);
    let horizon = Vec3::lerp(horizon_low, horizon_high, low_t);

    let sun_low = Vec3::new(1.0, 0.65, 0.3);
    let sun_high = Vec3::new(1.0, 0.95, 0.85);
    let sun_color = Vec3::lerp(sun_low, sun_high, low_t);

    (zenith, horizon, sun_color)
}
