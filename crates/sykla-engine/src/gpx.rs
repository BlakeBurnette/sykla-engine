use crate::dem::GeoOrigin;
use crate::terrain::{RoutePoint, SurfaceType};

struct GpsPoint {
    lat: f64,
    lng: f64,
}

/// Maximum distance between consecutive GPS points before treating as a gap (meters).
const MAX_SEGMENT_DIST: f64 = 100.0;

/// Minimum segment length to include (meters) — filters out isolated noise points.
const MIN_SEGMENT_LEN: f64 = 50.0;

/// Parse track points from GPX data. Extracts lat/lon from <trkpt> elements.
fn parse_gpx(gpx_data: &str) -> Vec<GpsPoint> {
    let mut points = Vec::new();
    for line in gpx_data.lines() {
        let line = line.trim();
        if !line.contains("<trkpt") {
            continue;
        }
        let lat = extract_attr(line, "lat");
        let lon = extract_attr(line, "lon");
        if let (Some(lat), Some(lon)) = (lat, lon) {
            points.push(GpsPoint { lat, lng: lon });
        }
    }
    points
}

fn extract_attr(line: &str, attr: &str) -> Option<f64> {
    let pattern = format!("{}=\"", attr);
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    rest[..end].parse().ok()
}

/// Project GPS lat/lon to local XZ coordinates in meters.
/// Origin at first point. X = east, Z = north.
fn project(points: &[GpsPoint]) -> Vec<(f64, f64)> {
    if points.is_empty() {
        return vec![];
    }
    let lat0_rad = points[0].lat.to_radians();
    let lng0 = points[0].lng;
    let lat0 = points[0].lat;

    points
        .iter()
        .map(|p| {
            let x = (p.lng - lng0) * lat0_rad.cos() * 111_320.0;
            let z = (p.lat - lat0) * 110_540.0;
            (x, z)
        })
        .collect()
}

/// Segment of continuous GPS track points.
struct TrackSegment {
    points: Vec<(f64, f64)>,
    length: f64,
}

/// Split projected points into continuous segments, filtering out gaps > MAX_SEGMENT_DIST.
/// Then concatenate all segments end-to-end into a single clean polyline, adjusting positions
/// so each segment starts where the previous one ended.
/// Returns (stitched_points, original_points, total_length).
/// `original_points` preserves the real GPS-projected coordinates for DEM sampling.
fn stitch_segments(projected: &[(f64, f64)]) -> (Vec<(f64, f64)>, Vec<(f64, f64)>, f64) {
    if projected.len() < 2 {
        return (projected.to_vec(), projected.to_vec(), 0.0);
    }

    // Step 1: Split into continuous segments, tracking original indices
    let mut segments: Vec<TrackSegment> = Vec::new();
    let mut seg_indices: Vec<Vec<usize>> = Vec::new();
    let mut cur_points = vec![projected[0]];
    let mut cur_indices = vec![0usize];
    let mut cur_len = 0.0;

    for i in 1..projected.len() {
        let dx = projected[i].0 - projected[i - 1].0;
        let dz = projected[i].1 - projected[i - 1].1;
        let dist = (dx * dx + dz * dz).sqrt();

        if dist <= MAX_SEGMENT_DIST {
            cur_len += dist;
            cur_points.push(projected[i]);
            cur_indices.push(i);
        } else {
            if cur_len >= MIN_SEGMENT_LEN && cur_points.len() >= 2 {
                seg_indices.push(std::mem::take(&mut cur_indices));
                segments.push(TrackSegment {
                    points: std::mem::take(&mut cur_points),
                    length: cur_len,
                });
            } else {
                cur_points.clear();
                cur_indices.clear();
            }
            cur_points.push(projected[i]);
            cur_indices.push(i);
            cur_len = 0.0;
        }
    }
    if cur_len >= MIN_SEGMENT_LEN && cur_points.len() >= 2 {
        seg_indices.push(cur_indices);
        segments.push(TrackSegment {
            points: cur_points,
            length: cur_len,
        });
    }

    if segments.is_empty() {
        return (vec![], vec![], 0.0);
    }

    // Step 2: Stitch segments end-to-end, preserving original coordinates
    let mut result = Vec::new();
    let mut originals = Vec::new();
    let mut total_len = 0.0;

    for (seg_idx, seg) in segments.iter().enumerate() {
        let indices = &seg_indices[seg_idx];

        if seg_idx == 0 {
            let origin = seg.points[0];
            for (k, p) in seg.points.iter().enumerate() {
                result.push((p.0 - origin.0, p.1 - origin.1));
                originals.push(projected[indices[k]]);
            }
        } else {
            let prev_end = *result.last().unwrap();
            let prev_dir = if result.len() >= 2 {
                let a = result[result.len() - 2];
                let b = result[result.len() - 1];
                let dx = b.0 - a.0;
                let dz = b.1 - a.1;
                let len = (dx * dx + dz * dz).sqrt().max(0.001);
                (dx / len, dz / len)
            } else {
                (0.0, 1.0)
            };

            let gap = 10.0;
            let connect_x = prev_end.0 + prev_dir.0 * gap;
            let connect_z = prev_end.1 + prev_dir.1 * gap;

            let seg_origin = seg.points[0];
            for (k, p) in seg.points.iter().enumerate() {
                result.push((
                    (p.0 - seg_origin.0) + connect_x,
                    (p.1 - seg_origin.1) + connect_z,
                ));
                originals.push(projected[indices[k]]);
            }
            total_len += gap;
        }
        total_len += seg.length;
    }

    (result, originals, total_len)
}

/// Interpolate position along a polyline at a given cumulative distance.
fn interp_pos(projected: &[(f64, f64)], cum_dist: &[f64], d: f64) -> (f64, f64) {
    if projected.len() < 2 {
        return projected.first().copied().unwrap_or((0.0, 0.0));
    }
    if d <= 0.0 {
        return projected[0];
    }
    for i in 1..cum_dist.len() {
        if cum_dist[i] >= d {
            let seg_len = cum_dist[i] - cum_dist[i - 1];
            let t = if seg_len < 0.001 {
                0.0
            } else {
                ((d - cum_dist[i - 1]) / seg_len).clamp(0.0, 1.0)
            };
            let px = projected[i - 1].0 + t * (projected[i].0 - projected[i - 1].0);
            let pz = projected[i - 1].1 + t * (projected[i].1 - projected[i - 1].1);
            return (px, pz);
        }
    }
    *projected.last().unwrap()
}

/// Interpolate elevation from waypoints at a given distance.
fn interpolate_elevation(waypoints: &[(f64, f64)], d: f64) -> f64 {
    if waypoints.is_empty() {
        return 100.0;
    }
    if d <= waypoints[0].0 {
        return waypoints[0].1;
    }
    for i in 1..waypoints.len() {
        if waypoints[i].0 >= d {
            let (d0, e0) = waypoints[i - 1];
            let (d1, e1) = waypoints[i];
            let t = if (d1 - d0).abs() < 0.001 {
                0.0
            } else {
                ((d - d0) / (d1 - d0)).clamp(0.0, 1.0)
            };
            return e0 + t * (e1 - e0);
        }
    }
    waypoints.last().unwrap().1
}

/// Generate RoutePoints from GPX track data + elevation waypoints.
/// GPS provides the real 2D path shape (turns, curves). Elevation waypoints
/// provide the height profile (scaled to match GPS path length).
/// Automatically filters out GPS teleportation gaps and stitches trail sections together.
pub fn generate_from_gpx(
    gpx_data: &str,
    elevation_waypoints: &[(f64, f64)],
) -> (Vec<RoutePoint>, Option<GeoOrigin>) {
    let gps = parse_gpx(gpx_data);
    if gps.len() < 2 {
        return (vec![], None);
    }

    let geo_origin = GeoOrigin::new(gps[0].lat, gps[0].lng);
    let all_projected = project(&gps);

    // Stitch together all continuous trail segments (removing driving gaps)
    // `originals` preserves pre-stitch GPS coordinates for DEM sampling
    let (projected, originals, _stitched_len) = stitch_segments(&all_projected);

    if projected.len() < 2 {
        return (vec![], None);
    }

    // Compute cumulative distance along stitched path
    let mut cum_dist = vec![0.0f64; projected.len()];
    for i in 1..projected.len() {
        let dx = projected[i].0 - projected[i - 1].0;
        let dz = projected[i].1 - projected[i - 1].1;
        cum_dist[i] = cum_dist[i - 1] + (dx * dx + dz * dz).sqrt();
    }
    let gps_total = cum_dist.last().copied().unwrap_or(1.0);
    let elev_total = elevation_waypoints.last().map(|e| e.0).unwrap_or(gps_total);

    // Resample at 5m intervals — finer spacing reduces terrain faceting on curves
    let num_points = (gps_total / 5.0).ceil() as usize + 1;
    let mut points = Vec::with_capacity(num_points);

    for i in 0..num_points {
        let d = (i as f64 * 5.0).min(gps_total);

        // Interpolate 2D position from stitched path (for rendering)
        let (px, pz) = interp_pos(&projected, &cum_dist, d);

        // Interpolate original GPS position (for DEM sampling)
        let (gx, gz) = interp_pos(&originals, &cum_dist, d);

        // Map GPS distance to elevation distance domain and interpolate
        let elev_d = d / gps_total * elev_total;
        let elevation = interpolate_elevation(elevation_waypoints, elev_d);

        // Forward direction: average of 5m ahead and behind for smoothing
        let d_ahead = (d + 5.0).min(gps_total);
        let d_behind = (d - 5.0).max(0.0);
        let (ax, az) = interp_pos(&projected, &cum_dist, d_ahead);
        let (bx, bz) = interp_pos(&projected, &cum_dist, d_behind);

        let fx = (ax - bx) as f32;
        let fz = (az - bz) as f32;
        let len = (fx * fx + fz * fz).sqrt().max(0.0001);

        points.push(RoutePoint {
            distance_m: d,
            elevation_m: elevation,
            pos_x: px as f32,
            pos_z: pz as f32,
            forward_x: fx / len,
            forward_z: fz / len,
            surface: SurfaceType::Paved,
            banking: 0.0,
            geo_x: gx as f32,
            geo_z: gz as f32,
        });
    }

    (points, Some(geo_origin))
}
