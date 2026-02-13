use crate::geo_math::haversine_distance;
use crate::gradient::calculate_grades;
use crate::types::{Route, RoutePoint};
use std::io::BufReader;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpxError {
    #[error("Failed to parse GPX: {0}")]
    ParseError(String),
    #[error("No track found in GPX file")]
    NoTrack,
    #[error("No track segments found")]
    NoSegments,
    #[error("Track has no points")]
    NoPoints,
}

/// Parse a GPX XML string into a Route.
///
/// Merges all tracks and segments into a single continuous path.
/// Multi-track GPX files (common from OSM exports) are fully supported.
pub fn parse_gpx(gpx_xml: &str) -> Result<Route, GpxError> {
    let reader = BufReader::new(gpx_xml.as_bytes());
    let gpx = gpx::read(reader).map_err(|e| GpxError::ParseError(e.to_string()))?;

    if gpx.tracks.is_empty() {
        return Err(GpxError::NoTrack);
    }

    let name = gpx
        .tracks
        .first()
        .and_then(|t| t.name.clone())
        .unwrap_or_else(|| "Unnamed Route".to_string());

    // Collect all waypoints from all tracks and segments
    let all_waypoints: Vec<&gpx::Waypoint> = gpx
        .tracks
        .iter()
        .flat_map(|t| t.segments.iter())
        .flat_map(|s| s.points.iter())
        .collect();

    if all_waypoints.is_empty() {
        return Err(GpxError::NoPoints);
    }

    // Build route points with cumulative distance
    let mut points = Vec::with_capacity(all_waypoints.len());
    let mut cumulative_distance = 0.0;
    let mut elevation_gain = 0.0;
    let mut min_elevation = f64::MAX;
    let mut max_elevation = f64::MIN;

    for (i, wp) in all_waypoints.iter().enumerate() {
        let lat = wp.point().y();
        let lng = wp.point().x();
        let elevation = wp.elevation.unwrap_or(0.0);

        if i > 0 {
            let prev = all_waypoints[i - 1];
            let d = haversine_distance(prev.point().y(), prev.point().x(), lat, lng);
            cumulative_distance += d;

            let prev_elev = prev.elevation.unwrap_or(0.0);
            let elev_diff = elevation - prev_elev;
            if elev_diff > 0.0 {
                elevation_gain += elev_diff;
            }
        }

        if elevation < min_elevation {
            min_elevation = elevation;
        }
        if elevation > max_elevation {
            max_elevation = elevation;
        }

        points.push(RoutePoint {
            lat,
            lng,
            elevation_m: elevation,
            distance_from_start_m: cumulative_distance,
            grade_percent: None,
            surface: Default::default(),
        });
    }

    // Calculate grades with smoothing
    calculate_grades(&mut points, 5);

    Ok(Route {
        name,
        total_distance_m: cumulative_distance,
        elevation_gain_m: elevation_gain,
        min_elevation_m: if min_elevation == f64::MAX {
            0.0
        } else {
            min_elevation
        },
        max_elevation_m: if max_elevation == f64::MIN {
            0.0
        } else {
            max_elevation
        },
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
      <trkpt lat="35.7820" lon="-78.6360"><ele>115.0</ele></trkpt>
      <trkpt lat="35.7825" lon="-78.6355"><ele>112.0</ele></trkpt>
      <trkpt lat="35.7830" lon="-78.6350"><ele>108.0</ele></trkpt>
    </trkseg>
  </trk>
</gpx>"#
    }

    #[test]
    fn test_parse_gpx_basic() {
        let route = parse_gpx(sample_gpx()).unwrap();
        assert_eq!(route.name, "Test Route");
        assert_eq!(route.points.len(), 8);
        assert_eq!(route.points[0].distance_from_start_m, 0.0);
        assert!(route.total_distance_m > 0.0);
        assert!(route.elevation_gain_m > 0.0);
        assert!(route.min_elevation_m <= route.max_elevation_m);
    }

    #[test]
    fn test_parse_gpx_distances_increase() {
        let route = parse_gpx(sample_gpx()).unwrap();
        for i in 1..route.points.len() {
            assert!(
                route.points[i].distance_from_start_m > route.points[i - 1].distance_from_start_m,
                "Distance should increase monotonically"
            );
        }
    }

    #[test]
    fn test_parse_gpx_grades_populated() {
        let route = parse_gpx(sample_gpx()).unwrap();
        // Grades should be populated after smoothing
        for point in &route.points {
            assert!(point.grade_percent.is_some());
        }
    }

    #[test]
    fn test_parse_gpx_no_track() {
        let gpx = r#"<?xml version="1.0"?><gpx version="1.1" xmlns="http://www.topografix.com/GPX/1/1"></gpx>"#;
        assert!(parse_gpx(gpx).is_err());
    }

    #[test]
    fn test_parse_real_gpx_files() {
        // Test against real OSM-exported multi-track GPX files in assets/
        let files = &[
            ("assets/routes/black-creek-greenway.gpx", 700),
            ("assets/routes/american-tobacco-trail.gpx", 100),
            ("assets/routes/white-oak-creek-greenway.gpx", 300),
        ];
        for (path, min_points) in files {
            let full = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path);
            if let Ok(gpx_data) = std::fs::read_to_string(&full) {
                let route = parse_gpx(&gpx_data).unwrap();
                assert!(
                    route.points.len() >= *min_points,
                    "{}: expected >= {} points, got {}",
                    path,
                    min_points,
                    route.points.len()
                );
                assert!(route.total_distance_m > 1000.0, "{}: too short", path);
                // Distances should increase
                for i in 1..route.points.len() {
                    assert!(
                        route.points[i].distance_from_start_m
                            >= route.points[i - 1].distance_from_start_m,
                    );
                }
            }
            // Skip if file not present (CI without assets)
        }
    }

    #[test]
    fn test_parse_gpx_multi_track() {
        // Simulate a multi-track GPX file (common from OSM exports)
        let gpx = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <name>Multi-Track Route</name>
    <trkseg>
      <trkpt lat="35.7796" lon="-78.6382"><ele>100.0</ele></trkpt>
      <trkpt lat="35.7800" lon="-78.6380"><ele>102.0</ele></trkpt>
    </trkseg>
  </trk>
  <trk>
    <name>Segment 2</name>
    <trkseg>
      <trkpt lat="35.7805" lon="-78.6375"><ele>105.0</ele></trkpt>
      <trkpt lat="35.7810" lon="-78.6370"><ele>108.0</ele></trkpt>
    </trkseg>
  </trk>
  <trk>
    <name>Segment 3</name>
    <trkseg>
      <trkpt lat="35.7815" lon="-78.6365"><ele>110.0</ele></trkpt>
      <trkpt lat="35.7820" lon="-78.6360"><ele>112.0</ele></trkpt>
    </trkseg>
  </trk>
</gpx>"#;
        let route = parse_gpx(gpx).unwrap();
        assert_eq!(route.name, "Multi-Track Route");
        assert_eq!(route.points.len(), 6); // all points from all tracks
        assert!(route.total_distance_m > 0.0);
        // Distances should still increase monotonically
        for i in 1..route.points.len() {
            assert!(
                route.points[i].distance_from_start_m > route.points[i - 1].distance_from_start_m,
            );
        }
    }
}
