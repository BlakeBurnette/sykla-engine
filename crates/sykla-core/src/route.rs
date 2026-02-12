use crate::geo_math::interpolate_point;
use crate::types::{Route, RoutePoint};

impl Route {
    /// Get the lat/lng coordinates at a specific distance along the route.
    pub fn position_at_distance(&self, distance_m: f64) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }
        if distance_m <= 0.0 {
            let p = &self.points[0];
            return Some((p.lat, p.lng));
        }
        if distance_m >= self.total_distance_m {
            let p = self.points.last().unwrap();
            return Some((p.lat, p.lng));
        }

        let idx = self
            .points
            .partition_point(|p| p.distance_from_start_m <= distance_m)
            .saturating_sub(1);

        if idx + 1 >= self.points.len() {
            let p = &self.points[idx];
            return Some((p.lat, p.lng));
        }

        let p1 = &self.points[idx];
        let p2 = &self.points[idx + 1];
        let seg_len = p2.distance_from_start_m - p1.distance_from_start_m;

        if seg_len < 0.01 {
            return Some((p1.lat, p1.lng));
        }

        let t = (distance_m - p1.distance_from_start_m) / seg_len;
        Some(interpolate_point(p1.lat, p1.lng, p2.lat, p2.lng, t))
    }

    /// Get the start coordinates of the route.
    pub fn start_position(&self) -> Option<(f64, f64)> {
        self.points.first().map(|p| (p.lat, p.lng))
    }

    /// Downsample route to at most `max_points` points, preserving start and end.
    pub fn downsample(&self, max_points: usize) -> Vec<RoutePoint> {
        if self.points.len() <= max_points || max_points < 2 {
            return self.points.clone();
        }

        let step = (self.points.len() - 1) as f64 / (max_points - 1) as f64;
        let mut result = Vec::with_capacity(max_points);
        for i in 0..max_points {
            let idx = (i as f64 * step).round() as usize;
            result.push(self.points[idx.min(self.points.len() - 1)].clone());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::gpx_parser::parse_gpx;

    fn sample_gpx() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test"
  xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <name>Test Route</name>
    <trkseg>
      <trkpt lat="35.7796" lon="-78.6382"><ele>100.0</ele></trkpt>
      <trkpt lat="35.7800" lon="-78.6380"><ele>102.0</ele></trkpt>
      <trkpt lat="35.7805" lon="-78.6375"><ele>105.0</ele></trkpt>
      <trkpt lat="35.7810" lon="-78.6370"><ele>103.0</ele></trkpt>
      <trkpt lat="35.7815" lon="-78.6365"><ele>110.0</ele></trkpt>
    </trkseg>
  </trk>
</gpx>"#
    }

    #[test]
    fn test_position_at_start() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let (lat, lng) = route.position_at_distance(0.0).unwrap();
        assert!((lat - 35.7796).abs() < 0.0001);
        assert!((lng - (-78.6382)).abs() < 0.0001);
    }

    #[test]
    fn test_position_at_end() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let (lat, lng) = route.position_at_distance(route.total_distance_m).unwrap();
        assert!((lat - 35.7815).abs() < 0.0001);
        assert!((lng - (-78.6365)).abs() < 0.0001);
    }

    #[test]
    fn test_position_at_midpoint() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let pos = route.position_at_distance(route.total_distance_m / 2.0);
        assert!(pos.is_some());
    }

    #[test]
    fn test_downsample() {
        let route = parse_gpx(sample_gpx()).unwrap();
        let downsampled = route.downsample(3);
        assert_eq!(downsampled.len(), 3);
        // First and last points preserved
        assert_eq!(downsampled[0].lat, route.points[0].lat);
        assert_eq!(
            downsampled.last().unwrap().lat,
            route.points.last().unwrap().lat
        );
    }
}
