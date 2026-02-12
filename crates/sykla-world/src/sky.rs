/// Sky dome configuration for the cartoon world.
#[derive(Debug, Clone)]
pub struct SkyConfig {
    /// Top color (zenith) as [R, G, B, A] (0.0-1.0).
    pub zenith_color: [f32; 4],
    /// Horizon color as [R, G, B, A] (0.0-1.0).
    pub horizon_color: [f32; 4],
    /// Sun direction (normalized).
    pub sun_direction: [f32; 3],
    /// Sun color.
    pub sun_color: [f32; 4],
}

impl Default for SkyConfig {
    fn default() -> Self {
        Self {
            zenith_color: [0.4, 0.6, 0.9, 1.0],   // Light blue
            horizon_color: [0.7, 0.85, 1.0, 1.0],  // Pale blue/white
            sun_direction: [0.5, 0.8, 0.3],         // Upper right
            sun_color: [1.0, 0.95, 0.8, 1.0],       // Warm white
        }
    }
}

/// Generate vertices for a sky dome (hemisphere).
/// Returns (vertices, uvs, indices).
pub fn generate_sky_dome(radius: f32, segments: u32, rings: u32) -> (Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Top vertex
    vertices.push([0.0, radius, 0.0]);
    uvs.push([0.5, 0.0]);

    for ring in 1..=rings {
        let phi = std::f32::consts::FRAC_PI_2 * ring as f32 / rings as f32;
        let y = radius * phi.cos();
        let ring_radius = radius * phi.sin();

        for seg in 0..segments {
            let theta = 2.0 * std::f32::consts::PI * seg as f32 / segments as f32;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();

            vertices.push([x, y, z]);
            uvs.push([
                seg as f32 / segments as f32,
                ring as f32 / rings as f32,
            ]);
        }
    }

    // Top cap triangles
    for seg in 0..segments {
        let next = (seg + 1) % segments;
        indices.push(0);
        indices.push(1 + seg);
        indices.push(1 + next);
    }

    // Ring triangles
    for ring in 0..rings - 1 {
        let ring_start = 1 + ring * segments;
        let next_ring_start = 1 + (ring + 1) * segments;

        for seg in 0..segments {
            let next = (seg + 1) % segments;

            indices.push(ring_start + seg);
            indices.push(next_ring_start + seg);
            indices.push(ring_start + next);

            indices.push(ring_start + next);
            indices.push(next_ring_start + seg);
            indices.push(next_ring_start + next);
        }
    }

    (vertices, uvs, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sky_dome_generation() {
        let (verts, uvs, indices) = generate_sky_dome(500.0, 16, 8);
        assert!(!verts.is_empty());
        assert_eq!(verts.len(), uvs.len());
        assert!(!indices.is_empty());
        // Top vertex + rings * segments
        assert_eq!(verts.len(), 1 + 8 * 16);
    }
}
