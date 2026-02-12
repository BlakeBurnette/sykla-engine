use crate::geo_math::haversine_distance;
use crate::heatmap::grid_to_lat_lng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// A detected route corridor from heat map analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedCorridor {
    pub points: Vec<(f64, f64)>,
    pub popularity_score: f64,
    pub is_loop: bool,
    pub distance_m: f64,
}

/// Configuration for corridor detection.
pub struct CorridorConfig {
    /// Minimum ride count for a cell to be considered active.
    pub min_ride_count: u32,
    /// Maximum distance (meters) between start and end to consider a loop.
    pub loop_threshold_m: f64,
    /// Minimum number of cells in a connected component to be a corridor.
    pub min_component_size: usize,
    /// Zoom level of the heatmap grid.
    pub zoom: u32,
}

impl Default for CorridorConfig {
    fn default() -> Self {
        Self {
            min_ride_count: 3,
            loop_threshold_m: 200.0,
            min_component_size: 10,
            zoom: 15,
        }
    }
}

/// Detect popular corridors from a heatmap grid.
///
/// Algorithm:
/// 1. Threshold: keep cells with ride_count >= min
/// 2. Connected component labeling via flood fill
/// 3. For each component: trace into a polyline (skeleton → centerline)
/// 4. Detect loops (start near end)
/// 5. Score by total ride count
pub fn detect_corridors(
    cells: &HashMap<(i32, i32), u32>,
    config: &CorridorConfig,
) -> Vec<DetectedCorridor> {
    // Step 1: Threshold
    let active: HashSet<(i32, i32)> = cells
        .iter()
        .filter(|(_, &count)| count >= config.min_ride_count)
        .map(|(&pos, _)| pos)
        .collect();

    if active.is_empty() {
        return Vec::new();
    }

    // Step 2: Connected component labeling (flood fill with 8-connectivity)
    let components = find_connected_components(&active);

    // Step 3-5: Process each component
    let mut corridors = Vec::new();

    for component in &components {
        if component.len() < config.min_component_size {
            continue;
        }

        // Compute popularity score (sum of ride counts)
        let popularity_score: f64 = component
            .iter()
            .filter_map(|pos| cells.get(pos))
            .map(|&c| c as f64)
            .sum();

        // Trace component into a polyline by walking the skeleton
        let polyline = trace_component(component, config.zoom);

        if polyline.len() < 2 {
            continue;
        }

        // Compute distance
        let mut distance_m = 0.0;
        for i in 1..polyline.len() {
            distance_m +=
                haversine_distance(polyline[i - 1].0, polyline[i - 1].1, polyline[i].0, polyline[i].1);
        }

        // Detect loop
        let first = polyline.first().unwrap();
        let last = polyline.last().unwrap();
        let start_end_dist = haversine_distance(first.0, first.1, last.0, last.1);
        let is_loop = start_end_dist < config.loop_threshold_m;

        corridors.push(DetectedCorridor {
            points: polyline,
            popularity_score,
            is_loop,
            distance_m,
        });
    }

    // Sort by popularity (highest first)
    corridors.sort_by(|a, b| b.popularity_score.partial_cmp(&a.popularity_score).unwrap());

    corridors
}

/// Find connected components using flood fill with 8-connectivity.
fn find_connected_components(cells: &HashSet<(i32, i32)>) -> Vec<Vec<(i32, i32)>> {
    let mut visited = HashSet::new();
    let mut components = Vec::new();

    for &cell in cells {
        if visited.contains(&cell) {
            continue;
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(cell);
        visited.insert(cell);

        while let Some(current) = queue.pop_front() {
            component.push(current);

            // 8-connectivity neighbors
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let neighbor = (current.0 + dx, current.1 + dy);
                    if cells.contains(&neighbor) && !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        components.push(component);
    }

    components
}

/// Trace a connected component into an ordered polyline.
///
/// Strategy: find the cell farthest from the centroid as the start,
/// then greedily walk to the nearest unvisited neighbor to build a path.
/// Finally convert grid coordinates to lat/lng.
fn trace_component(component: &[(i32, i32)], zoom: u32) -> Vec<(f64, f64)> {
    if component.is_empty() {
        return Vec::new();
    }
    if component.len() == 1 {
        let (lat, lng) = grid_to_lat_lng(component[0].0, component[0].1, zoom);
        return vec![(lat, lng)];
    }

    let cell_set: HashSet<(i32, i32)> = component.iter().copied().collect();

    // Find centroid
    let cx: f64 = component.iter().map(|c| c.0 as f64).sum::<f64>() / component.len() as f64;
    let cy: f64 = component.iter().map(|c| c.1 as f64).sum::<f64>() / component.len() as f64;

    // Start from the cell farthest from centroid
    let start = component
        .iter()
        .max_by(|a, b| {
            let da = (a.0 as f64 - cx).powi(2) + (a.1 as f64 - cy).powi(2);
            let db = (b.0 as f64 - cx).powi(2) + (b.1 as f64 - cy).powi(2);
            da.partial_cmp(&db).unwrap()
        })
        .unwrap();

    // Greedy walk: always pick the nearest unvisited neighbor
    let mut visited = HashSet::new();
    let mut path = Vec::new();
    let mut current = *start;

    loop {
        visited.insert(current);
        path.push(current);

        // Find nearest unvisited neighbor in the component
        let mut best: Option<(i32, i32)> = None;
        let mut best_dist = f64::MAX;

        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let neighbor = (current.0 + dx, current.1 + dy);
                if cell_set.contains(&neighbor) && !visited.contains(&neighbor) {
                    let dist = ((dx * dx + dy * dy) as f64).sqrt();
                    if dist < best_dist {
                        best_dist = dist;
                        best = Some(neighbor);
                    }
                }
            }
        }

        match best {
            Some(next) => current = next,
            None => break,
        }
    }

    // Convert to lat/lng
    path.iter()
        .map(|&(gx, gy)| grid_to_lat_lng(gx, gy, zoom))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_corridors_empty() {
        let cells = HashMap::new();
        let config = CorridorConfig::default();
        let corridors = detect_corridors(&cells, &config);
        assert!(corridors.is_empty());
    }

    #[test]
    fn test_detect_corridors_below_threshold() {
        let mut cells = HashMap::new();
        cells.insert((100, 200), 1); // below min_ride_count=3
        cells.insert((101, 200), 2);
        let config = CorridorConfig::default();
        let corridors = detect_corridors(&cells, &config);
        assert!(corridors.is_empty());
    }

    #[test]
    fn test_detect_corridors_straight_line() {
        let mut cells = HashMap::new();
        // Create a straight line of 15 cells, all with ride_count >= 3
        for x in 0..15 {
            cells.insert((1000 + x, 2000), 5);
        }
        let config = CorridorConfig {
            min_ride_count: 3,
            min_component_size: 5,
            ..Default::default()
        };
        let corridors = detect_corridors(&cells, &config);
        assert_eq!(corridors.len(), 1);
        assert!(!corridors[0].is_loop);
        assert!(corridors[0].distance_m > 0.0);
        assert!((corridors[0].popularity_score - 75.0).abs() < 0.1);
    }

    #[test]
    fn test_find_connected_components() {
        let mut cells = HashSet::new();
        // Component 1: (0,0), (1,0), (2,0)
        cells.insert((0, 0));
        cells.insert((1, 0));
        cells.insert((2, 0));
        // Component 2: (10,10), (11,10)
        cells.insert((10, 10));
        cells.insert((11, 10));

        let components = find_connected_components(&cells);
        assert_eq!(components.len(), 2);
    }

    #[test]
    fn test_corridors_sorted_by_popularity() {
        let mut cells = HashMap::new();
        // Component 1: low popularity
        for x in 0..15 {
            cells.insert((x, 0), 3);
        }
        // Component 2: high popularity (separated by a gap)
        for x in 100..115 {
            cells.insert((x, 0), 10);
        }

        let config = CorridorConfig {
            min_ride_count: 3,
            min_component_size: 5,
            ..Default::default()
        };
        let corridors = detect_corridors(&cells, &config);
        assert_eq!(corridors.len(), 2);
        assert!(corridors[0].popularity_score > corridors[1].popularity_score);
    }
}
