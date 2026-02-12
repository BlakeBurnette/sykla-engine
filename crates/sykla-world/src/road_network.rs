use serde::{Deserialize, Serialize};

use crate::city::FeatureSubtype;

/// A navigable road network graph: nodes at intersections, edges as road segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadNetwork {
    pub nodes: Vec<RoadNode>,
    pub edges: Vec<RoadEdge>,
}

/// A node in the road network (intersection or dead-end).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadNode {
    /// Position in local meters (x, z).
    pub position: [f32; 2],
    /// Indices of incident edges.
    pub edge_ids: Vec<u32>,
}

/// An edge in the road network (a road segment between two nodes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadEdge {
    /// Index of node at the start of this edge.
    pub node_a: u32,
    /// Index of node at the end of this edge.
    pub node_b: u32,
    /// Detailed centerline polyline in local meters (x, z).
    pub centerline: Vec<[f32; 2]>,
    /// Road subtype (Highway, Street, etc.)
    pub subtype: FeatureSubtype,
    /// Total length in meters.
    pub length_m: f32,
    /// Street name (from OSM `name` tag).
    #[serde(default)]
    pub name: Option<String>,
    /// Whether this edge is part of a roundabout.
    #[serde(default)]
    pub is_roundabout: bool,
}

impl RoadEdge {
    /// Interpolate a position along the centerline.
    /// `progress` is 0.0 at node_a and 1.0 at node_b.
    pub fn sample_position(&self, progress: f32) -> [f32; 2] {
        let progress = progress.clamp(0.0, 1.0);
        if self.centerline.len() < 2 {
            return self.centerline.first().copied().unwrap_or([0.0, 0.0]);
        }

        let target_dist = progress * self.length_m;
        let mut accumulated = 0.0_f32;

        for i in 0..self.centerline.len() - 1 {
            let a = self.centerline[i];
            let b = self.centerline[i + 1];
            let dx = b[0] - a[0];
            let dz = b[1] - a[1];
            let seg_len = (dx * dx + dz * dz).sqrt();

            if accumulated + seg_len >= target_dist {
                let t = if seg_len > 1e-6 {
                    (target_dist - accumulated) / seg_len
                } else {
                    0.0
                };
                return [a[0] + dx * t, a[1] + dz * t];
            }
            accumulated += seg_len;
        }

        // At or past the end
        *self.centerline.last().unwrap()
    }

    /// Get the forward direction (unit vector) at a point along the centerline.
    /// `progress` is 0.0 at node_a and 1.0 at node_b.
    pub fn sample_direction(&self, progress: f32) -> [f32; 2] {
        let progress = progress.clamp(0.0, 1.0);
        if self.centerline.len() < 2 {
            return [0.0, 1.0]; // default forward
        }

        let target_dist = progress * self.length_m;
        let mut accumulated = 0.0_f32;

        for i in 0..self.centerline.len() - 1 {
            let a = self.centerline[i];
            let b = self.centerline[i + 1];
            let dx = b[0] - a[0];
            let dz = b[1] - a[1];
            let seg_len = (dx * dx + dz * dz).sqrt();

            if accumulated + seg_len >= target_dist || i == self.centerline.len() - 2 {
                if seg_len > 1e-6 {
                    return [dx / seg_len, dz / seg_len];
                }
                break;
            }
            accumulated += seg_len;
        }

        // Fallback: use first and last point
        let a = self.centerline[0];
        let b = *self.centerline.last().unwrap();
        let dx = b[0] - a[0];
        let dz = b[1] - a[1];
        let len = (dx * dx + dz * dz).sqrt();
        if len > 1e-6 {
            [dx / len, dz / len]
        } else {
            [0.0, 1.0]
        }
    }
}

impl RoadNetwork {
    /// Find the closest edge to a point (x, z).
    /// Returns (edge_index, progress_along_edge, squared_distance).
    pub fn nearest_edge(&self, x: f32, z: f32) -> Option<(u32, f32, f32)> {
        if self.edges.is_empty() {
            return None;
        }

        let mut best_edge = 0u32;
        let mut best_progress = 0.0f32;
        let mut best_dist_sq = f32::MAX;

        for (idx, edge) in self.edges.iter().enumerate() {
            let (progress, dist_sq) = closest_point_on_polyline(&edge.centerline, x, z);
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                // Convert polyline-distance progress to 0..1
                let total_len = edge.length_m;
                best_progress = if total_len > 1e-6 {
                    (progress / total_len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                best_edge = idx as u32;
            }
        }

        Some((best_edge, best_progress, best_dist_sq))
    }
}

/// Find the closest point on a polyline to (px, pz).
/// Returns (distance_along_polyline, squared_distance_to_point).
fn closest_point_on_polyline(polyline: &[[f32; 2]], px: f32, pz: f32) -> (f32, f32) {
    let mut best_dist_sq = f32::MAX;
    let mut best_along = 0.0f32;
    let mut accumulated = 0.0f32;

    for i in 0..polyline.len().saturating_sub(1) {
        let a = polyline[i];
        let b = polyline[i + 1];

        let dx = b[0] - a[0];
        let dz = b[1] - a[1];
        let seg_len_sq = dx * dx + dz * dz;
        let seg_len = seg_len_sq.sqrt();

        // Project point onto segment
        let t = if seg_len_sq > 1e-12 {
            let apx = px - a[0];
            let apz = pz - a[1];
            ((apx * dx + apz * dz) / seg_len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let closest_x = a[0] + dx * t;
        let closest_z = a[1] + dz * t;
        let dist_sq = (px - closest_x) * (px - closest_x) + (pz - closest_z) * (pz - closest_z);

        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_along = accumulated + t * seg_len;
        }

        accumulated += seg_len;
    }

    (best_along, best_dist_sq)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_straight_edge() -> RoadEdge {
        RoadEdge {
            node_a: 0,
            node_b: 1,
            centerline: vec![[0.0, 0.0], [100.0, 0.0]],
            subtype: FeatureSubtype::Street,
            length_m: 100.0,
            name: Some("Test St".to_string()),
            is_roundabout: false,
        }
    }

    #[test]
    fn test_sample_position_endpoints() {
        let edge = make_straight_edge();
        let start = edge.sample_position(0.0);
        let end = edge.sample_position(1.0);
        assert!((start[0]).abs() < 0.01);
        assert!((end[0] - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_sample_position_midpoint() {
        let edge = make_straight_edge();
        let mid = edge.sample_position(0.5);
        assert!((mid[0] - 50.0).abs() < 0.01);
        assert!((mid[1]).abs() < 0.01);
    }

    #[test]
    fn test_sample_direction() {
        let edge = make_straight_edge();
        let dir = edge.sample_direction(0.5);
        assert!((dir[0] - 1.0).abs() < 0.01);
        assert!((dir[1]).abs() < 0.01);
    }

    #[test]
    fn test_nearest_edge() {
        let network = RoadNetwork {
            nodes: vec![
                RoadNode { position: [0.0, 0.0], edge_ids: vec![0] },
                RoadNode { position: [100.0, 0.0], edge_ids: vec![0] },
            ],
            edges: vec![make_straight_edge()],
        };
        let (idx, progress, dist_sq) = network.nearest_edge(50.0, 10.0).unwrap();
        assert_eq!(idx, 0);
        assert!((progress - 0.5).abs() < 0.01);
        assert!((dist_sq - 100.0).abs() < 0.01); // 10^2
    }
}
