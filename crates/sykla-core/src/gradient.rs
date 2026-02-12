use crate::types::RoutePoint;

/// Calculate gradient (grade %) between consecutive route points.
/// Uses a smoothing window to reduce jitter from GPS noise.
///
/// `window_size` is the number of points to average over (must be odd, minimum 1).
pub fn calculate_grades(points: &mut [RoutePoint], window_size: usize) {
    let n = points.len();
    if n < 2 {
        for p in points.iter_mut() {
            p.grade_percent = Some(0.0);
        }
        return;
    }

    // First calculate raw grades
    let mut raw_grades = vec![0.0f64; n];
    for i in 1..n {
        let d_distance = points[i].distance_from_start_m - points[i - 1].distance_from_start_m;
        let d_elevation = points[i].elevation_m - points[i - 1].elevation_m;

        if d_distance > 0.5 {
            // Need at least 0.5m horizontal to calculate meaningful grade
            raw_grades[i] = (d_elevation / d_distance) * 100.0;
        }
    }
    raw_grades[0] = raw_grades.get(1).copied().unwrap_or(0.0);

    // Apply moving average smoothing
    let half = window_size / 2;
    for i in 0..n {
        let start = i.saturating_sub(half);
        let end = (i + half + 1).min(n);
        let sum: f64 = raw_grades[start..end].iter().sum();
        let count = (end - start) as f64;
        let smoothed = sum / count;

        // Clamp to realistic values (-40% to +40%)
        points[i].grade_percent = Some(smoothed.clamp(-40.0, 40.0));
    }
}

/// Get the grade at a specific distance along the route (linear interpolation).
pub fn grade_at_distance(points: &[RoutePoint], distance_m: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    if distance_m <= 0.0 {
        return points[0].grade_percent.unwrap_or(0.0);
    }
    if distance_m >= points.last().unwrap().distance_from_start_m {
        return points.last().unwrap().grade_percent.unwrap_or(0.0);
    }

    // Binary search for the segment containing this distance
    let idx = points
        .partition_point(|p| p.distance_from_start_m <= distance_m)
        .saturating_sub(1);

    if idx + 1 >= points.len() {
        return points[idx].grade_percent.unwrap_or(0.0);
    }

    let p1 = &points[idx];
    let p2 = &points[idx + 1];
    let seg_len = p2.distance_from_start_m - p1.distance_from_start_m;

    if seg_len < 0.01 {
        return p1.grade_percent.unwrap_or(0.0);
    }

    let t = (distance_m - p1.distance_from_start_m) / seg_len;
    let g1 = p1.grade_percent.unwrap_or(0.0);
    let g2 = p2.grade_percent.unwrap_or(0.0);

    g1 + t * (g2 - g1)
}

/// Get the elevation at a specific distance along the route (linear interpolation).
pub fn elevation_at_distance(points: &[RoutePoint], distance_m: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    if distance_m <= 0.0 {
        return points[0].elevation_m;
    }
    if distance_m >= points.last().unwrap().distance_from_start_m {
        return points.last().unwrap().elevation_m;
    }

    let idx = points
        .partition_point(|p| p.distance_from_start_m <= distance_m)
        .saturating_sub(1);

    if idx + 1 >= points.len() {
        return points[idx].elevation_m;
    }

    let p1 = &points[idx];
    let p2 = &points[idx + 1];
    let seg_len = p2.distance_from_start_m - p1.distance_from_start_m;

    if seg_len < 0.01 {
        return p1.elevation_m;
    }

    let t = (distance_m - p1.distance_from_start_m) / seg_len;
    p1.elevation_m + t * (p2.elevation_m - p1.elevation_m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoutePoint;

    fn make_points() -> Vec<RoutePoint> {
        vec![
            RoutePoint {
                lat: 0.0,
                lng: 0.0,
                elevation_m: 100.0,
                distance_from_start_m: 0.0,
                grade_percent: None,
            },
            RoutePoint {
                lat: 0.0,
                lng: 0.0,
                elevation_m: 110.0,
                distance_from_start_m: 100.0,
                grade_percent: None,
            },
            RoutePoint {
                lat: 0.0,
                lng: 0.0,
                elevation_m: 120.0,
                distance_from_start_m: 200.0,
                grade_percent: None,
            },
            RoutePoint {
                lat: 0.0,
                lng: 0.0,
                elevation_m: 115.0,
                distance_from_start_m: 300.0,
                grade_percent: None,
            },
            RoutePoint {
                lat: 0.0,
                lng: 0.0,
                elevation_m: 100.0,
                distance_from_start_m: 400.0,
                grade_percent: None,
            },
        ]
    }

    #[test]
    fn test_calculate_grades() {
        let mut points = make_points();
        calculate_grades(&mut points, 1);

        // 10m rise over 100m = 10%
        assert!((points[1].grade_percent.unwrap() - 10.0).abs() < 0.1);
        // -5m over 100m = -5%
        assert!((points[3].grade_percent.unwrap() - (-5.0)).abs() < 0.1);
    }

    #[test]
    fn test_grade_at_distance_interpolation() {
        let mut points = make_points();
        calculate_grades(&mut points, 1);

        let grade = grade_at_distance(&points, 50.0);
        // Between point 0 and 1, grade is 10%
        assert!((grade - 10.0).abs() < 1.0);
    }

    #[test]
    fn test_grade_at_distance_boundary() {
        let mut points = make_points();
        calculate_grades(&mut points, 1);

        // Before start
        let g0 = grade_at_distance(&points, -10.0);
        assert!(g0.is_finite());

        // After end
        let gn = grade_at_distance(&points, 500.0);
        assert!(gn.is_finite());
    }

    #[test]
    fn test_elevation_at_distance() {
        let points = make_points();
        let e = elevation_at_distance(&points, 150.0);
        // Halfway between 110 and 120
        assert!((e - 115.0).abs() < 0.1);
    }
}
