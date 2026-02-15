/// Grass shell parameters for volumetric grass via shell texturing.
///
/// Shell texturing re-renders the terrain mesh N times at increasing Y offsets.
/// A hash-based density mask discards fragments at higher layers, creating
/// the illusion of individual grass blades.
pub struct GrassParams {
    pub num_shells: u32,
    pub shell_height: f32,
    pub color_base: [f32; 3],
    pub color_tip: [f32; 3],
    pub density: f32,
    pub wind_strength: f32,
    pub wind_dir: [f32; 2],
    pub fade_start: f32,
    pub fade_end: f32,
}

impl GrassParams {
    /// Returns grass parameters for the given biome type, or None if the biome has no grass.
    /// Biome types: 1=piedmont, 2=alpine, 3=desert (no grass), 4=coastal, 5=forest
    pub fn for_biome(biome_type: f32) -> Option<Self> {
        if biome_type < 0.5 || (biome_type > 2.5 && biome_type < 3.5) {
            return None;
        }

        let num_shells = if cfg!(target_arch = "wasm32") { 6 } else { 8 };

        Some(if biome_type < 1.5 {
            // Piedmont — lush maintained grass
            Self {
                num_shells,
                shell_height: 0.50,
                color_base: [0.20, 0.35, 0.10],
                color_tip: [0.40, 0.55, 0.20],
                density: 0.7,
                wind_strength: 0.4,
                wind_dir: [0.7, 0.7],
                fade_start: 50.0,
                fade_end: 100.0,
            }
        } else if biome_type < 2.5 {
            // Alpine — hardy mountain meadow
            Self {
                num_shells,
                shell_height: 0.55,
                color_base: [0.18, 0.32, 0.08],
                color_tip: [0.35, 0.50, 0.18],
                density: 0.5,
                wind_strength: 0.6,
                wind_dir: [0.6, 0.8],
                fade_start: 50.0,
                fade_end: 100.0,
            }
        } else if biome_type < 4.5 {
            // Coastal — salt-tolerant grass
            Self {
                num_shells,
                shell_height: 0.60,
                color_base: [0.25, 0.38, 0.12],
                color_tip: [0.50, 0.60, 0.28],
                density: 0.6,
                wind_strength: 0.7,
                wind_dir: [0.5, 0.87],
                fade_start: 50.0,
                fade_end: 100.0,
            }
        } else {
            // Forest — shade-tolerant understory
            Self {
                num_shells,
                shell_height: 0.40,
                color_base: [0.16, 0.28, 0.08],
                color_tip: [0.28, 0.42, 0.14],
                density: 0.5,
                wind_strength: 0.25,
                wind_dir: [0.7, 0.7],
                fade_start: 50.0,
                fade_end: 100.0,
            }
        })
    }
}
