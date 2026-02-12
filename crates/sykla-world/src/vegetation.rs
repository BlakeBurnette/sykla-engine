use sykla_core::types::RoutePoint;

/// Position and type of a vegetation element (tree, bush, etc.).
#[derive(Debug, Clone)]
pub struct VegetationInstance {
    pub position: [f32; 3],
    pub scale: f32,
    pub kind: VegetationType,
}

#[derive(Debug, Clone, Copy)]
pub enum VegetationType {
    Tree,
    Bush,
}

/// Configuration for vegetation placement.
#[derive(Debug, Clone)]
pub struct VegetationConfig {
    /// Average spacing between trees (meters along route).
    pub tree_spacing: f32,
    /// Distance from road center for tree placement.
    pub min_distance: f32,
    pub max_distance: f32,
    /// Elevation scale (should match terrain).
    pub elevation_scale: f32,
    /// Pseudo-random seed.
    pub seed: u64,
}

impl Default for VegetationConfig {
    fn default() -> Self {
        Self {
            tree_spacing: 15.0,
            min_distance: 6.0,
            max_distance: 40.0,
            elevation_scale: 1.0,
            seed: 42,
        }
    }
}

/// Generate vegetation instances along the route.
/// Uses a simple deterministic pseudo-random placement.
pub fn generate_vegetation(
    points: &[RoutePoint],
    config: &VegetationConfig,
) -> Vec<VegetationInstance> {
    if points.len() < 2 {
        return vec![];
    }

    let total_dist = points.last().unwrap().distance_from_start_m as f32;
    let n_trees = (total_dist / config.tree_spacing) as usize;

    let mut instances = Vec::with_capacity(n_trees * 2);
    let mut rng_state = config.seed;

    for i in 0..n_trees {
        let along = i as f32 * config.tree_spacing;

        // Find elevation at this distance
        let elevation = interpolate_elevation(points, along as f64, config.elevation_scale);

        // Place trees on both sides
        for side in [-1.0f32, 1.0] {
            rng_state = simple_hash(rng_state);
            let rand_offset = (rng_state % 1000) as f32 / 1000.0;
            let x_dist =
                config.min_distance + rand_offset * (config.max_distance - config.min_distance);

            rng_state = simple_hash(rng_state);
            let scale = 0.8 + (rng_state % 500) as f32 / 1000.0; // 0.8 to 1.3

            rng_state = simple_hash(rng_state);
            let kind = if rng_state % 4 == 0 {
                VegetationType::Bush
            } else {
                VegetationType::Tree
            };

            instances.push(VegetationInstance {
                position: [side * x_dist, elevation, along],
                scale,
                kind,
            });
        }
    }

    instances
}

fn interpolate_elevation(points: &[RoutePoint], distance: f64, scale: f32) -> f32 {
    let elevation = sykla_core::gradient::elevation_at_distance(points, distance);
    elevation as f32 * scale
}

/// Simple deterministic hash for pseudo-random placement.
fn simple_hash(mut state: u64) -> u64 {
    state = state.wrapping_mul(6364136223846793005);
    state = state.wrapping_add(1442695040888963407);
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use sykla_core::types::RoutePoint;

    #[test]
    fn test_generate_vegetation() {
        let points = vec![
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 100.0, distance_from_start_m: 0.0, grade_percent: Some(0.0) },
            RoutePoint { lat: 0.0, lng: 0.0, elevation_m: 105.0, distance_from_start_m: 200.0, grade_percent: Some(2.5) },
        ];

        let veg = generate_vegetation(&points, &VegetationConfig::default());
        assert!(!veg.is_empty());
        // Trees should be on both sides of the road
        let left_count = veg.iter().filter(|v| v.position[0] < 0.0).count();
        let right_count = veg.iter().filter(|v| v.position[0] > 0.0).count();
        assert!(left_count > 0);
        assert!(right_count > 0);
    }
}
