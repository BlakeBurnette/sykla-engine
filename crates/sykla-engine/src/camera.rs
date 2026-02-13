use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub eye_pos: [f32; 4],
    pub light_vp: [[f32; 4]; 4],
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

        // Light orthographic projection centered on camera target
        let light_dir_n = light_dir.normalize();
        let light_pos = self.target - light_dir_n * 200.0;
        let light_view = Mat4::look_at_rh(light_pos, self.target, Vec3::Y);
        let light_proj = Mat4::orthographic_rh(-100.0, 100.0, -100.0, 100.0, 1.0, 500.0);

        CameraUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            eye_pos: [self.eye.x, self.eye.y, self.eye.z, time],
            light_vp: (light_proj * light_view).to_cols_array_2d(),
        }
    }
}
