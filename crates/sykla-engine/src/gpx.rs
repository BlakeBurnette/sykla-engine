use crate::dem::GeoOrigin;
use crate::terrain::{RoutePoint, SurfaceType};

#[derive(Clone)]
pub(crate) struct GpsPoint {
    lat: f64,
    lng: f64,
    ele: Option<f64>,
}

/// Maximum distance between consecutive GPS points before treating as a gap (meters).
const MAX_SEGMENT_DIST: f64 = 100.0;

/// Minimum segment length to include (meters) — filters out isolated noise points.
const MIN_SEGMENT_LEN: f64 = 50.0;

/// Parse track points from GPX data. Extracts lat/lon and optional elevation from <trkpt> elements.
/// Handles both self-closing `<trkpt ... />` and open `<trkpt ...><ele>...</ele></trkpt>`.
fn parse_gpx(gpx_data: &str) -> Vec<GpsPoint> {
    let mut points = Vec::new();
    let lines: Vec<&str> = gpx_data.lines().map(|l| l.trim()).collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if !line.contains("<trkpt") {
            i += 1;
            continue;
        }
        let lat = extract_attr(line, "lat");
        let lon = extract_attr(line, "lon");
        if let (Some(lat), Some(lon)) = (lat, lon) {
            // Check for <ele> on the same line first
            let mut ele = extract_element_value(line, "ele");
            // If not self-closing and no inline ele, look ahead for <ele> child
            if ele.is_none() && !line.ends_with("/>") {
                let mut j = i + 1;
                while j < lines.len() {
                    if lines[j].contains("</trkpt>") {
                        break;
                    }
                    if let Some(e) = extract_element_value(lines[j], "ele") {
                        ele = Some(e);
                        break;
                    }
                    j += 1;
                }
            }
            points.push(GpsPoint { lat, lng: lon, ele });
        }
        i += 1;
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

/// Extract text content from a simple XML element like `<ele>123.45</ele>`.
fn extract_element_value(line: &str, tag: &str) -> Option<f64> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = line.find(&open)? + open.len();
    let end = line.find(&close)?;
    line[start..end].trim().parse().ok()
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
/// Returns (stitched_points, original_points, stitched_elevations, total_length).
/// `original_points` preserves the real GPS-projected coordinates for DEM sampling.
fn stitch_segments(
    projected: &[(f64, f64)],
    elevations: &[Option<f64>],
) -> (Vec<(f64, f64)>, Vec<(f64, f64)>, Vec<Option<f64>>, f64) {
    if projected.len() < 2 {
        return (projected.to_vec(), projected.to_vec(), elevations.to_vec(), 0.0);
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
        return (vec![], vec![], vec![], 0.0);
    }

    // Step 2: Stitch segments end-to-end, preserving original coordinates and elevations
    let mut result = Vec::new();
    let mut originals = Vec::new();
    let mut stitched_elev = Vec::new();
    let mut total_len = 0.0;

    for (seg_idx, seg) in segments.iter().enumerate() {
        let indices = &seg_indices[seg_idx];

        if seg_idx == 0 {
            let origin = seg.points[0];
            for (k, p) in seg.points.iter().enumerate() {
                result.push((p.0 - origin.0, p.1 - origin.1));
                originals.push(projected[indices[k]]);
                stitched_elev.push(elevations[indices[k]]);
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
                stitched_elev.push(elevations[indices[k]]);
            }
            total_len += gap;
        }
        total_len += seg.length;
    }

    (result, originals, stitched_elev, total_len)
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

/// Interpolate elevation from GPX track data at a given cumulative distance.
/// Returns None if the surrounding track points lack elevation data.
fn interp_elevation_from_track(
    elevations: &[Option<f64>],
    cum_dist: &[f64],
    d: f64,
) -> Option<f64> {
    if elevations.len() < 2 {
        return elevations.first().copied().flatten();
    }
    if d <= 0.0 {
        return elevations[0];
    }
    for i in 1..cum_dist.len() {
        if cum_dist[i] >= d {
            let e0 = elevations[i - 1]?;
            let e1 = elevations[i]?;
            let seg_len = cum_dist[i] - cum_dist[i - 1];
            let t = if seg_len < 0.001 {
                0.0
            } else {
                ((d - cum_dist[i - 1]) / seg_len).clamp(0.0, 1.0)
            };
            return Some(e0 + t * (e1 - e0));
        }
    }
    *elevations.last().unwrap()
}

/// Generate RoutePoints from GPX track data + elevation waypoints.
/// If the GPX file contains `<ele>` tags, those are used as ground truth for elevation.
/// Otherwise falls back to the provided elevation_waypoints for backward compatibility.
/// Automatically filters out GPS teleportation gaps and stitches trail sections together.
/// Filter GPS anomalies: remove points with impossible jumps, sharp reversals,
/// or elevation spikes. GPS trackers can produce wild outliers from signal bounce,
/// tunnel re-acquisition, or multipath interference.
fn filter_anomalies(points: &[GpsPoint]) -> Vec<GpsPoint> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut clean = Vec::with_capacity(points.len());
    clean.push(points[0].clone());

    for i in 1..points.len() {
        let prev = clean.last().unwrap();
        let cur = &points[i];

        // Distance check: haversine distance between consecutive points
        let dlat = (cur.lat - prev.lat).to_radians();
        let dlng = (cur.lng - prev.lng).to_radians();
        let a = (dlat / 2.0).sin().powi(2)
            + prev.lat.to_radians().cos() * cur.lat.to_radians().cos() * (dlng / 2.0).sin().powi(2);
        let dist_m = 6_371_000.0 * 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        // Skip points that teleport > 500m (GPS glitch or tracker restarting)
        if dist_m > 500.0 {
            log::warn!("GPS anomaly: {:.0}m jump at point {}, skipping", dist_m, i);
            continue;
        }

        // Skip near-duplicate points (< 1m apart — GPS jitter while stationary)
        if dist_m < 1.0 {
            continue;
        }

        // Angle check: reject sharp reversals (> 150° turn at cycling speed)
        // Only check if we have enough points
        if clean.len() >= 2 {
            let pp = &clean[clean.len() - 2];
            // Vectors: prev→pp_prev and prev→cur
            let ax = prev.lng - pp.lng;
            let az = prev.lat - pp.lat;
            let bx = cur.lng - prev.lng;
            let bz = cur.lat - prev.lat;
            let dot = ax * bx + az * bz;
            let mag_a = (ax * ax + az * az).sqrt();
            let mag_b = (bx * bx + bz * bz).sqrt();
            if mag_a > 0.0 && mag_b > 0.0 {
                let cos_angle = (dot / (mag_a * mag_b)).clamp(-1.0, 1.0);
                let angle_deg = cos_angle.acos().to_degrees();
                // 0° = straight ahead (parallel vectors), 180° = full reversal
                if angle_deg > 150.0 && dist_m > 10.0 {
                    log::warn!("GPS anomaly: {:.0}° reversal at point {}, skipping", angle_deg, i);
                    continue;
                }
            }
        }

        // Elevation spike check: reject > 20m elevation change in one step
        if let (Some(prev_e), Some(cur_e)) = (prev.ele, cur.ele) {
            if (cur_e - prev_e).abs() > 20.0 {
                log::warn!("GPS anomaly: {:.1}m elevation spike at point {}, skipping", cur_e - prev_e, i);
                continue;
            }
        }

        clean.push(cur.clone());
    }

    let removed = points.len() - clean.len();
    if removed > 0 {
        log::info!("GPS anomaly filter: removed {} of {} points", removed, points.len());
    }
    clean
}

/// Merge multiple GPX tracks into an averaged, refined trail.
/// Each new ride contributes to smoothing the canonical path.
/// Returns merged GPS points sorted by distance along the trail.
pub(crate) fn merge_gpx_tracks(base_gpx: &str, ride_gpx: &[&str]) -> Vec<GpsPoint> {
    let mut base = filter_anomalies(&parse_gpx(base_gpx));
    if base.is_empty() {
        return base;
    }

    for ride_data in ride_gpx {
        let ride = filter_anomalies(&parse_gpx(ride_data));
        if ride.is_empty() {
            continue;
        }

        // For each base point, find the nearest ride point and average toward it.
        // Weight: base has accumulated trust, new ride is one more sample.
        // This progressively refines the trail with each ride.
        for bp in &mut base {
            let mut best_dist = f64::MAX;
            let mut best_lat = bp.lat;
            let mut best_lng = bp.lng;
            let mut best_ele = bp.ele;

            for rp in &ride {
                let dlat = bp.lat - rp.lat;
                let dlng = bp.lng - rp.lng;
                let d = dlat * dlat + dlng * dlng; // approximate, fine for matching
                if d < best_dist {
                    best_dist = d;
                    best_lat = rp.lat;
                    best_lng = rp.lng;
                    best_ele = rp.ele;
                }
            }

            // Only merge if the nearest ride point is within ~30m
            let match_dist_m = best_dist.sqrt() * 110_000.0; // rough degrees→meters
            if match_dist_m < 30.0 {
                // Weighted average: 80% existing, 20% new ride (gradual refinement)
                bp.lat = bp.lat * 0.8 + best_lat * 0.2;
                bp.lng = bp.lng * 0.8 + best_lng * 0.2;
                if let (Some(be), Some(re)) = (bp.ele, best_ele) {
                    bp.ele = Some(be * 0.8 + re * 0.2);
                }
            }
        }
    }

    base
}

pub fn generate_from_gpx(
    gpx_data: &str,
    elevation_waypoints: &[(f64, f64)],
) -> (Vec<RoutePoint>, Option<GeoOrigin>) {
    let gps = filter_anomalies(&parse_gpx(gpx_data));
    if gps.len() < 2 {
        return (vec![], None);
    }

    let geo_origin = GeoOrigin::new(gps[0].lat, gps[0].lng);
    let gps_elevations: Vec<Option<f64>> = gps.iter().map(|p| p.ele).collect();
    let has_gpx_elevation = gps_elevations.iter().any(|e| e.is_some());
    let all_projected = project(&gps);

    // Stitch together all continuous trail segments (removing driving gaps)
    // `originals` preserves pre-stitch GPS coordinates for DEM sampling
    let (projected, originals, stitched_elev, _stitched_len) =
        stitch_segments(&all_projected, &gps_elevations);

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

    // Resample at 5m intervals — finer spacing reduces terrain faceting on curves
    let num_points = (gps_total / 5.0).ceil() as usize + 1;
    let mut points = Vec::with_capacity(num_points);

    for i in 0..num_points {
        let d = (i as f64 * 5.0).min(gps_total);

        // Interpolate 2D position from stitched path (for rendering)
        let (px, pz) = interp_pos(&projected, &cum_dist, d);

        // Interpolate original GPS position (for DEM sampling)
        let (gx, gz) = interp_pos(&originals, &cum_dist, d);

        // Elevation: use GPX <ele> data if available, otherwise fall back to waypoints
        let elevation = if has_gpx_elevation {
            interp_elevation_from_track(&stitched_elev, &cum_dist, d)
                .unwrap_or_else(|| {
                    let elev_total = elevation_waypoints.last().map(|e| e.0).unwrap_or(gps_total);
                    let elev_d = d / gps_total * elev_total;
                    interpolate_elevation(elevation_waypoints, elev_d)
                })
        } else {
            let elev_total = elevation_waypoints.last().map(|e| e.0).unwrap_or(gps_total);
            let elev_d = d / gps_total * elev_total;
            interpolate_elevation(elevation_waypoints, elev_d)
        };

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
            curvature: 0.0,
        });
    }

    // Position smoothing: moving-average on XZ to straighten angular kinks
    // from sparse GPX data. Window of 20 points (100m) smooths out noise
    // while preserving the overall path shape.
    let pos_half = 10; // 10 points each side = 100m window
    let raw_x: Vec<f32> = points.iter().map(|p| p.pos_x).collect();
    let raw_z: Vec<f32> = points.iter().map(|p| p.pos_z).collect();
    let raw_gx: Vec<f32> = points.iter().map(|p| p.geo_x).collect();
    let raw_gz: Vec<f32> = points.iter().map(|p| p.geo_z).collect();
    for i in 0..points.len() {
        let lo = if i >= pos_half { i - pos_half } else { 0 };
        let hi = (i + pos_half + 1).min(points.len());
        let n = (hi - lo) as f32;
        let sx: f32 = raw_x[lo..hi].iter().sum();
        let sz: f32 = raw_z[lo..hi].iter().sum();
        let sgx: f32 = raw_gx[lo..hi].iter().sum();
        let sgz: f32 = raw_gz[lo..hi].iter().sum();
        points[i].pos_x = sx / n;
        points[i].pos_z = sz / n;
        points[i].geo_x = sgx / n;
        points[i].geo_z = sgz / n;
    }

    // Recompute forward directions after position smoothing
    for i in 0..points.len() {
        let ahead = (i + 1).min(points.len() - 1);
        let behind = if i > 0 { i - 1 } else { 0 };
        let fx = points[ahead].pos_x - points[behind].pos_x;
        let fz = points[ahead].pos_z - points[behind].pos_z;
        let len = (fx * fx + fz * fz).sqrt().max(0.0001);
        points[i].forward_x = fx / len;
        points[i].forward_z = fz / len;
    }

    // Elevation smoothing: moving-average to eliminate jagged spikes.
    // Window of 10 points (50m at 5m spacing).
    let raw_elev: Vec<f64> = points.iter().map(|p| p.elevation_m).collect();
    let half_win = 5;
    for i in 0..points.len() {
        let lo = if i >= half_win { i - half_win } else { 0 };
        let hi = (i + half_win + 1).min(points.len());
        let sum: f64 = raw_elev[lo..hi].iter().sum();
        points[i].elevation_m = sum / (hi - lo) as f64;
    }

    // Safety: clamp maximum gradient to ±3% (ATT is a rail trail, max ~3% at I-40 underpass)
    let max_grade = 0.03;
    for i in 1..points.len() {
        let d_dist = points[i].distance_m - points[i - 1].distance_m;
        if d_dist > 0.1 {
            let d_elev = points[i].elevation_m - points[i - 1].elevation_m;
            let grade = d_elev / d_dist;
            if grade.abs() > max_grade {
                points[i].elevation_m =
                    points[i - 1].elevation_m + d_dist * grade.clamp(-max_grade, max_grade);
            }
        }
    }

    (points, Some(geo_origin))
}
