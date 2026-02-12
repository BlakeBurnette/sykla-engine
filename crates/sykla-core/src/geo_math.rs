use std::f64::consts::PI;

const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Haversine distance in meters between two lat/lng points.
pub fn haversine_distance(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let a = (dlat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (dlng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_M * c
}

/// Bearing in degrees (0-360) from point 1 to point 2.
pub fn bearing(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let dlng = (lng2 - lng1).to_radians();

    let y = dlng.sin() * lat2_rad.cos();
    let x = lat1_rad.cos() * lat2_rad.sin() - lat1_rad.sin() * lat2_rad.cos() * dlng.cos();

    let bearing_rad = y.atan2(x);
    (bearing_rad.to_degrees() + 360.0) % 360.0
}

/// Calculate cumulative distances for a sequence of (lat, lng) points.
/// Returns a Vec of distances from start in meters (first element is 0.0).
pub fn cumulative_distances(points: &[(f64, f64)]) -> Vec<f64> {
    let mut distances = Vec::with_capacity(points.len());
    distances.push(0.0);
    for i in 1..points.len() {
        let d = haversine_distance(points[i - 1].0, points[i - 1].1, points[i].0, points[i].1);
        distances.push(distances[i - 1] + d);
    }
    distances
}

/// Interpolate a lat/lng position along a line segment by fraction t (0.0 to 1.0).
pub fn interpolate_point(lat1: f64, lng1: f64, lat2: f64, lng2: f64, t: f64) -> (f64, f64) {
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let lng1_rad = lng1.to_radians();
    let lng2_rad = lng2.to_radians();

    let d = haversine_distance(lat1, lng1, lat2, lng2) / EARTH_RADIUS_M;

    if d < 1e-12 {
        return (lat1, lng1);
    }

    let a = ((1.0 - t) * d).sin() / d.sin();
    let b = (t * d).sin() / d.sin();

    let x = a * lat1_rad.cos() * lng1_rad.cos() + b * lat2_rad.cos() * lng2_rad.cos();
    let y = a * lat1_rad.cos() * lng1_rad.sin() + b * lat2_rad.cos() * lng2_rad.sin();
    let z = a * lat1_rad.sin() + b * lat2_rad.sin();

    let lat = z.atan2((x * x + y * y).sqrt());
    let lng = y.atan2(x);

    (lat * 180.0 / PI, lng * 180.0 / PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haversine_known_distance() {
        // Raleigh, NC to Durham, NC — approximately 35 km
        let d = haversine_distance(35.7796, -78.6382, 35.9940, -78.8986);
        assert!((d - 31_500.0).abs() < 2000.0, "Expected ~31.5 km, got {d}");
    }

    #[test]
    fn test_haversine_same_point() {
        let d = haversine_distance(35.0, -78.0, 35.0, -78.0);
        assert!(d < 0.001);
    }

    #[test]
    fn test_bearing_north() {
        let b = bearing(35.0, -78.0, 36.0, -78.0);
        assert!((b - 0.0).abs() < 1.0, "Expected ~0°, got {b}");
    }

    #[test]
    fn test_bearing_east() {
        let b = bearing(0.0, 0.0, 0.0, 1.0);
        assert!((b - 90.0).abs() < 1.0, "Expected ~90°, got {b}");
    }

    #[test]
    fn test_cumulative_distances() {
        let points = vec![(0.0, 0.0), (0.0, 0.001), (0.0, 0.002)];
        let dists = cumulative_distances(&points);
        assert_eq!(dists.len(), 3);
        assert_eq!(dists[0], 0.0);
        assert!(dists[1] > 0.0);
        assert!((dists[2] - 2.0 * dists[1]).abs() < 1.0);
    }
}
