use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct PostProcessUniforms {
    pub exposure: f32,
    pub bloom_intensity: f32,
    pub vignette_intensity: f32,
    pub saturation: f32,
    pub color_temperature: f32,
    pub ssao_intensity: f32,
    pub _pad: [f32; 2],
}

impl Default for PostProcessUniforms {
    fn default() -> Self {
        Self {
            exposure: 1.2,
            bloom_intensity: 0.15,
            vignette_intensity: 0.15,
            saturation: 1.05,
            color_temperature: 6500.0,
            ssao_intensity: 0.5,
            _pad: [0.0; 2],
        }
    }
}
