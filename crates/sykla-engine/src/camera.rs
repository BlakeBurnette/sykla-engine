use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CameraMode {
    ThirdPersonClose, // tight over-shoulder (default)
    ThirdPersonFar,   // wide cinematic
    FirstPerson,      // rider's eye level
}

impl CameraMode {
    /// Returns (behind, up, lateral, look_ahead, fov_degrees).
    pub fn camera_params(self) -> (f32, f32, f32, f32, f32) {
        match self {
            Self::ThirdPersonClose => (2.5, 1.5, -0.3, 10.0, 60.0),
            Self::ThirdPersonFar   => (5.0, 2.5, -1.0, 20.0, 60.0),
            Self::FirstPerson      => (0.0, 1.7,  0.0, 20.0, 75.0),
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::ThirdPersonClose => Self::ThirdPersonFar,
            Self::ThirdPersonFar   => Self::FirstPerson,
            Self::FirstPerson      => Self::ThirdPersonClose,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ThirdPersonClose => "3PV NEAR",
            Self::ThirdPersonFar   => "3PV FAR",
            Self::FirstPerson      => "FPV",
        }
    }
}

pub const NUM_CASCADES: usize = 4;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub eye_pos: [f32; 4],
    pub light_vp: [[[f32; 4]; 4]; NUM_CASCADES],
    pub cascade_splits: [f32; 4],
    pub view: [[f32; 4]; 4],
}

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            eye: Vec3::new(0.0, 120.0, -50.0),
            target: Vec3::new(0.0, 100.0, 100.0),
            up: Vec3::Y,
            aspect,
            fov_y: 60.0_f32.to_radians(),
            near: 0.5,
            far: 5000.0,
        }
    }

    pub fn uniform(&self, light_dir: Vec3, time: f32) -> CameraUniform {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far);

        let light_dir_n = light_dir.normalize();

        // Cascade split distances (practical split scheme, lambda=0.5)
        let splits = cascade_splits(self.near, self.far, NUM_CASCADES, 0.5);

        // Compute light VP matrix for each cascade
        let inv_view_proj = (proj * view).inverse();
        let mut light_vps = [[[0.0f32; 4]; 4]; NUM_CASCADES];

        for i in 0..NUM_CASCADES {
            let near_z = if i == 0 { self.near } else { splits[i - 1] };
            let far_z = splits[i];
            light_vps[i] = cascade_light_vp(inv_view_proj, light_dir_n, near_z, far_z, self.near, self.far)
                .to_cols_array_2d();
        }

        CameraUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            eye_pos: [self.eye.x, self.eye.y, self.eye.z, time],
            light_vp: light_vps,
            cascade_splits: [splits[0], splits[1], splits[2], splits[3]],
            view: view.to_cols_array_2d(),
        }
    }
}

/// Practical split scheme (mix of logarithmic and linear).
fn cascade_splits(near: f32, far: f32, num_cascades: usize, lambda: f32) -> [f32; NUM_CASCADES] {
    // Cap effective far to avoid wasting resolution on fogged-out terrain
    let effective_far = far.min(1200.0);
    let mut splits = [0.0f32; NUM_CASCADES];
    for i in 0..num_cascades {
        let p = (i + 1) as f32 / num_cascades as f32;
        let log_split = near * (effective_far / near).powf(p);
        let linear_split = near + (effective_far - near) * p;
        splits[i] = lambda * log_split + (1.0 - lambda) * linear_split;
    }
    splits
}

/// Build a tight orthographic light-space matrix for a single cascade.
fn cascade_light_vp(
    inv_view_proj: Mat4,
    light_dir: Vec3,
    near_z: f32,
    far_z: f32,
    cam_near: f32,
    cam_far: f32,
) -> Mat4 {
    // Compute the 8 frustum corners for this cascade slice in world space
    let mut corners = [Vec3::ZERO; 8];
    let ndc_near = 1.0 - 2.0 * (near_z - cam_near) / (cam_far - cam_near);
    let ndc_far = 1.0 - 2.0 * (far_z - cam_near) / (cam_far - cam_near);

    let mut idx = 0;
    for &z in &[ndc_near, ndc_far] {
        for &y in &[-1.0f32, 1.0] {
            for &x in &[-1.0f32, 1.0] {
                let clip = Vec4::new(x, y, z, 1.0);
                let world = inv_view_proj * clip;
                corners[idx] = world.truncate() / world.w;
                idx += 1;
            }
        }
    }

    // Center of the frustum slice
    let center = corners.iter().copied().fold(Vec3::ZERO, |a, b| a + b) / 8.0;

    // Light view matrix looking at the center from the light direction
    let light_pos = center - light_dir * 200.0;
    let light_view = Mat4::look_at_rh(light_pos, center, Vec3::Y);

    // Find AABB in light space
    let mut min_ls = Vec3::splat(f32::MAX);
    let mut max_ls = Vec3::splat(f32::MIN);
    for c in &corners {
        let ls = (light_view * Vec4::new(c.x, c.y, c.z, 1.0)).truncate();
        min_ls = min_ls.min(ls);
        max_ls = max_ls.max(ls);
    }

    // Pad slightly to avoid shadow swimming
    let pad = 2.0;
    min_ls -= Vec3::splat(pad);
    max_ls += Vec3::splat(pad);

    let light_proj = Mat4::orthographic_rh(
        min_ls.x, max_ls.x,
        min_ls.y, max_ls.y,
        -max_ls.z - 100.0, -min_ls.z + 100.0,
    );

    light_proj * light_view
}
