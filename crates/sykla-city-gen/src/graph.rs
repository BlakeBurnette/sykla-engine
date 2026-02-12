use crate::parser::RoadFeature;
use crate::projection::CityProjection;
use sykla_world::city::FeatureSubtype;
use sykla_world::road_network::{RoadEdge, RoadNetwork, RoadNode};

/// Snap radius in meters — points within this distance are merged into one node.
const SNAP_RADIUS: f64 = 5.0;
/// Minimum edge length in meters — shorter edges are discarded.
const MIN_EDGE_LENGTH: f32 = 2.0;

/// Returns true if a road subtype is navigable (drivable roads only, no sidewalks/paths).
fn is_navigable(subtype: FeatureSubtype) -> bool {
    matches!(
        subtype,
        FeatureSubtype::Highway
            | FeatureSubtype::MainStreet
            | FeatureSubtype::Street
    )
}

/// Build a navigable road network graph from parsed OSM road features.
///
/// Algorithm:
/// 1. Project all road centerlines to local meters
/// 2. Collect all endpoints (first + last of each road)
/// 3. Spatial snapping: points within SNAP_RADIUS across different roads = same node
/// 4. T-intersection detection: if a road's endpoint falls near another road's interior,
///    split that road at the closest point
/// 5. Build edges with polylines between nodes
/// 6. Build nodes with adjacency lists
/// 7. Filter out degenerate edges
pub fn build_road_network(
    features: &[RoadFeature],
    proj: &CityProjection,
) -> RoadNetwork {
    // Step 1: Project all centerlines to local meters (navigable roads only)
    let projected: Vec<ProjectedRoad> = features
        .iter()
        .filter(|f| is_navigable(f.subtype))
        .map(|f| {
            let pts: Vec<[f64; 2]> = f
                .centerline
                .iter()
                .map(|&(lat, lng)| {
                    let (x, z) = proj.project(lat, lng);
                    [x, z]
                })
                .collect();
            ProjectedRoad {
                points: pts,
                subtype: f.subtype,
                name: f.name.clone(),
                is_roundabout: f.is_roundabout,
            }
        })
        .filter(|r| r.points.len() >= 2)
        .collect();

    if projected.is_empty() {
        return RoadNetwork {
            nodes: vec![],
            edges: vec![],
        };
    }

    // Step 2-4: Collect all node positions and split roads at T-intersections
    let split_roads = split_at_intersections(&projected);

    // Step 5-6: Build graph from split road segments
    let mut node_positions: Vec<[f64; 2]> = Vec::new();

    // Find or create a node for a position
    let mut find_or_create_node = |pos: [f64; 2], nodes: &mut Vec<[f64; 2]>| -> u32 {
        for (i, n) in nodes.iter().enumerate() {
            let dx = n[0] - pos[0];
            let dz = n[1] - pos[1];
            if dx * dx + dz * dz < SNAP_RADIUS * SNAP_RADIUS {
                return i as u32;
            }
        }
        let idx = nodes.len() as u32;
        nodes.push(pos);
        idx
    };

    let mut edges: Vec<RoadEdge> = Vec::new();

    for seg in &split_roads {
        if seg.points.len() < 2 {
            continue;
        }

        let start = seg.points[0];
        let end = *seg.points.last().unwrap();

        let node_a = find_or_create_node(start, &mut node_positions);
        let node_b = find_or_create_node(end, &mut node_positions);

        // Skip self-loops
        if node_a == node_b {
            continue;
        }

        let centerline: Vec<[f32; 2]> = seg
            .points
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32])
            .collect();

        let length_m = polyline_length(&centerline);

        if length_m < MIN_EDGE_LENGTH {
            continue;
        }

        edges.push(RoadEdge {
            node_a,
            node_b,
            centerline,
            subtype: seg.subtype,
            length_m,
            name: seg.name.clone(),
            is_roundabout: seg.is_roundabout,
        });
    }

    // Step 7: Build node adjacency lists
    let mut nodes: Vec<RoadNode> = node_positions
        .iter()
        .map(|p| RoadNode {
            position: [p[0] as f32, p[1] as f32],
            edge_ids: Vec::new(),
        })
        .collect();

    for (edge_idx, edge) in edges.iter().enumerate() {
        nodes[edge.node_a as usize].edge_ids.push(edge_idx as u32);
        nodes[edge.node_b as usize].edge_ids.push(edge_idx as u32);
    }

    // Remove isolated nodes (no edges)
    // Instead of removing (which shifts indices), just leave them — they're harmless.

    RoadNetwork { nodes, edges }
}

struct ProjectedRoad {
    points: Vec<[f64; 2]>,
    subtype: FeatureSubtype,
    name: Option<String>,
    is_roundabout: bool,
}

struct RoadSegment {
    points: Vec<[f64; 2]>,
    subtype: FeatureSubtype,
    name: Option<String>,
    is_roundabout: bool,
}

/// Split roads at T-intersections where one road's endpoint is near another road's interior.
fn split_at_intersections(roads: &[ProjectedRoad]) -> Vec<RoadSegment> {
    // Collect all road endpoints
    let mut endpoints: Vec<[f64; 2]> = Vec::new();
    for road in roads {
        if let Some(first) = road.points.first() {
            endpoints.push(*first);
        }
        if let Some(last) = road.points.last() {
            endpoints.push(*last);
        }
    }

    // For each road, find split points where other roads' endpoints hit its interior
    let mut segments: Vec<RoadSegment> = Vec::new();

    for road in roads {
        // Find all split distances along this road
        let mut split_distances: Vec<f64> = Vec::new();

        for ep in &endpoints {
            // Check if this endpoint is near the interior of this road (not its own endpoints)
            let first = road.points[0];
            let last = *road.points.last().unwrap();

            let d_first = dist_sq_2d(ep, &first);
            let d_last = dist_sq_2d(ep, &last);

            // Skip if this endpoint matches the road's own endpoints
            if d_first < SNAP_RADIUS * SNAP_RADIUS || d_last < SNAP_RADIUS * SNAP_RADIUS {
                continue;
            }

            // Find closest point on the road interior
            let (along, dist_sq) = closest_point_on_polyline_f64(&road.points, ep[0], ep[1]);
            if dist_sq < SNAP_RADIUS * SNAP_RADIUS {
                split_distances.push(along);
            }
        }

        if split_distances.is_empty() {
            // No splits — keep the road as-is
            segments.push(RoadSegment {
                points: road.points.clone(),
                subtype: road.subtype,
                name: road.name.clone(),
                is_roundabout: road.is_roundabout,
            });
        } else {
            // Sort splits and create sub-segments
            split_distances.sort_by(|a, b| a.partial_cmp(b).unwrap());
            split_distances.dedup_by(|a, b| (*a - *b).abs() < 1.0);

            let sub_segments = split_polyline_at_distances(&road.points, &split_distances);
            for pts in sub_segments {
                if pts.len() >= 2 {
                    segments.push(RoadSegment {
                        points: pts,
                        subtype: road.subtype,
                        name: road.name.clone(),
                        is_roundabout: road.is_roundabout,
                    });
                }
            }
        }
    }

    segments
}

/// Split a polyline at specified distances along it, returning sub-polylines.
fn split_polyline_at_distances(
    polyline: &[[f64; 2]],
    distances: &[f64],
) -> Vec<Vec<[f64; 2]>> {
    if polyline.len() < 2 || distances.is_empty() {
        return vec![polyline.to_vec()];
    }

    let mut result: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut current_segment: Vec<[f64; 2]> = vec![polyline[0]];
    let mut accumulated = 0.0_f64;
    let mut dist_idx = 0;

    for i in 0..polyline.len() - 1 {
        let a = polyline[i];
        let b = polyline[i + 1];
        let dx = b[0] - a[0];
        let dz = b[1] - a[1];
        let seg_len = (dx * dx + dz * dz).sqrt();

        // Check if any split distances fall within this segment
        while dist_idx < distances.len() {
            let target = distances[dist_idx];
            if target <= accumulated + seg_len + 1e-6 {
                // Split point is within this segment
                let t = if seg_len > 1e-9 {
                    ((target - accumulated) / seg_len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let split_pt = [a[0] + dx * t, a[1] + dz * t];

                current_segment.push(split_pt);
                result.push(current_segment);
                current_segment = vec![split_pt];
                dist_idx += 1;
            } else {
                break;
            }
        }

        current_segment.push(b);
        accumulated += seg_len;
    }

    if current_segment.len() >= 2 {
        result.push(current_segment);
    }

    result
}

fn dist_sq_2d(a: &[f64; 2], b: &[f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dz = a[1] - b[1];
    dx * dx + dz * dz
}

/// Find closest point on a polyline (f64 version for graph building).
/// Returns (distance_along_polyline, squared_distance).
fn closest_point_on_polyline_f64(polyline: &[[f64; 2]], px: f64, pz: f64) -> (f64, f64) {
    let mut best_dist_sq = f64::MAX;
    let mut best_along = 0.0_f64;
    let mut accumulated = 0.0_f64;

    for i in 0..polyline.len().saturating_sub(1) {
        let a = polyline[i];
        let b = polyline[i + 1];

        let dx = b[0] - a[0];
        let dz = b[1] - a[1];
        let seg_len_sq = dx * dx + dz * dz;
        let seg_len = seg_len_sq.sqrt();

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

fn polyline_length(pts: &[[f32; 2]]) -> f32 {
    let mut len = 0.0f32;
    for i in 0..pts.len().saturating_sub(1) {
        let dx = pts[i + 1][0] - pts[i][0];
        let dz = pts[i + 1][1] - pts[i][1];
        len += (dx * dx + dz * dz).sqrt();
    }
    len
}
