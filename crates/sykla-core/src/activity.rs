use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source of an activity recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource {
    Recorded,
    GpxImport,
}

impl std::fmt::Display for ActivitySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivitySource::Recorded => write!(f, "recorded"),
            ActivitySource::GpxImport => write!(f, "gpx_import"),
        }
    }
}

impl ActivitySource {
    pub fn from_str(s: &str) -> Self {
        match s {
            "gpx_import" => ActivitySource::GpxImport,
            _ => ActivitySource::Recorded,
        }
    }
}

/// A single telemetry point in an activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityPoint {
    pub lat: f64,
    pub lng: f64,
    pub elevation_m: Option<f64>,
    pub timestamp_ms: i64,
    pub speed_kmh: Option<f64>,
    pub power_watts: Option<f64>,
    pub heart_rate_bpm: Option<u8>,
    pub cadence_rpm: Option<f64>,
    pub distance_from_start_m: f64,
    pub grade_percent: Option<f64>,
}

/// Axis-aligned bounding box in lat/lng coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl BoundingBox {
    /// Compute bounding box from a sequence of lat/lng points.
    pub fn from_points(points: &[ActivityPoint]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let mut south = f64::MAX;
        let mut west = f64::MAX;
        let mut north = f64::MIN;
        let mut east = f64::MIN;
        for p in points {
            south = south.min(p.lat);
            west = west.min(p.lng);
            north = north.max(p.lat);
            east = east.max(p.lng);
        }
        Some(BoundingBox {
            south,
            west,
            north,
            east,
        })
    }
}

/// A complete activity with all telemetry points and aggregate stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub name: String,
    pub source: ActivitySource,
    pub points: Vec<ActivityPoint>,
    pub total_distance_m: f64,
    pub elevation_gain_m: f64,
    pub duration_secs: f64,
    pub started_at: Option<DateTime<Utc>>,
    pub bbox: Option<BoundingBox>,
    pub avg_power_watts: Option<f64>,
    pub max_power_watts: Option<f64>,
    pub avg_speed_kmh: Option<f64>,
    pub max_speed_kmh: Option<f64>,
    pub avg_heart_rate_bpm: Option<u8>,
    pub max_heart_rate_bpm: Option<u8>,
    pub avg_cadence_rpm: Option<f64>,
    pub max_cadence_rpm: Option<f64>,
    pub calories: Option<i32>,
}

impl Activity {
    /// Compute aggregate stats from the activity's points.
    pub fn compute_aggregates(points: &[ActivityPoint]) -> ActivityAggregates {
        if points.is_empty() {
            return ActivityAggregates::default();
        }

        let mut total_power = 0.0_f64;
        let mut power_count = 0u32;
        let mut max_power = 0.0_f64;

        let mut total_speed = 0.0_f64;
        let mut speed_count = 0u32;
        let mut max_speed = 0.0_f64;

        let mut total_hr = 0u32;
        let mut hr_count = 0u32;
        let mut max_hr = 0u8;

        let mut total_cadence = 0.0_f64;
        let mut cadence_count = 0u32;
        let mut max_cadence = 0.0_f64;

        let mut elevation_gain = 0.0_f64;

        for (i, p) in points.iter().enumerate() {
            if let Some(pw) = p.power_watts {
                total_power += pw;
                power_count += 1;
                if pw > max_power {
                    max_power = pw;
                }
            }
            if let Some(sp) = p.speed_kmh {
                total_speed += sp;
                speed_count += 1;
                if sp > max_speed {
                    max_speed = sp;
                }
            }
            if let Some(hr) = p.heart_rate_bpm {
                total_hr += hr as u32;
                hr_count += 1;
                if hr > max_hr {
                    max_hr = hr;
                }
            }
            if let Some(ca) = p.cadence_rpm {
                total_cadence += ca;
                cadence_count += 1;
                if ca > max_cadence {
                    max_cadence = ca;
                }
            }
            if i > 0 {
                if let (Some(prev_elev), Some(curr_elev)) =
                    (points[i - 1].elevation_m, p.elevation_m)
                {
                    let diff = curr_elev - prev_elev;
                    if diff > 0.0 {
                        elevation_gain += diff;
                    }
                }
            }
        }

        let total_distance = points.last().map(|p| p.distance_from_start_m).unwrap_or(0.0);

        let duration_secs = if points.len() >= 2 {
            let first_ts = points.first().unwrap().timestamp_ms;
            let last_ts = points.last().unwrap().timestamp_ms;
            (last_ts - first_ts) as f64 / 1000.0
        } else {
            0.0
        };

        ActivityAggregates {
            total_distance_m: total_distance,
            elevation_gain_m: elevation_gain,
            duration_secs,
            avg_power_watts: if power_count > 0 {
                Some(total_power / power_count as f64)
            } else {
                None
            },
            max_power_watts: if power_count > 0 {
                Some(max_power)
            } else {
                None
            },
            avg_speed_kmh: if speed_count > 0 {
                Some(total_speed / speed_count as f64)
            } else {
                None
            },
            max_speed_kmh: if speed_count > 0 {
                Some(max_speed)
            } else {
                None
            },
            avg_heart_rate_bpm: if hr_count > 0 {
                Some((total_hr / hr_count) as u8)
            } else {
                None
            },
            max_heart_rate_bpm: if hr_count > 0 { Some(max_hr) } else { None },
            avg_cadence_rpm: if cadence_count > 0 {
                Some(total_cadence / cadence_count as f64)
            } else {
                None
            },
            max_cadence_rpm: if cadence_count > 0 {
                Some(max_cadence)
            } else {
                None
            },
            bbox: BoundingBox::from_points(points),
        }
    }
}

/// Computed aggregate statistics from activity points.
#[derive(Debug, Clone, Default)]
pub struct ActivityAggregates {
    pub total_distance_m: f64,
    pub elevation_gain_m: f64,
    pub duration_secs: f64,
    pub avg_power_watts: Option<f64>,
    pub max_power_watts: Option<f64>,
    pub avg_speed_kmh: Option<f64>,
    pub max_speed_kmh: Option<f64>,
    pub avg_heart_rate_bpm: Option<u8>,
    pub max_heart_rate_bpm: Option<u8>,
    pub avg_cadence_rpm: Option<f64>,
    pub max_cadence_rpm: Option<f64>,
    pub bbox: Option<BoundingBox>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_box_from_points() {
        let points = vec![
            ActivityPoint {
                lat: 35.78,
                lng: -78.64,
                elevation_m: Some(100.0),
                timestamp_ms: 0,
                speed_kmh: None,
                power_watts: None,
                heart_rate_bpm: None,
                cadence_rpm: None,
                distance_from_start_m: 0.0,
                grade_percent: None,
            },
            ActivityPoint {
                lat: 35.80,
                lng: -78.62,
                elevation_m: Some(110.0),
                timestamp_ms: 1000,
                speed_kmh: None,
                power_watts: None,
                heart_rate_bpm: None,
                cadence_rpm: None,
                distance_from_start_m: 100.0,
                grade_percent: None,
            },
        ];
        let bbox = BoundingBox::from_points(&points).unwrap();
        assert!((bbox.south - 35.78).abs() < 0.001);
        assert!((bbox.north - 35.80).abs() < 0.001);
        assert!((bbox.west - (-78.64)).abs() < 0.001);
        assert!((bbox.east - (-78.62)).abs() < 0.001);
    }

    #[test]
    fn test_compute_aggregates() {
        let points = vec![
            ActivityPoint {
                lat: 0.0,
                lng: 0.0,
                elevation_m: Some(100.0),
                timestamp_ms: 0,
                speed_kmh: Some(20.0),
                power_watts: Some(150.0),
                heart_rate_bpm: Some(120),
                cadence_rpm: Some(80.0),
                distance_from_start_m: 0.0,
                grade_percent: None,
            },
            ActivityPoint {
                lat: 0.0,
                lng: 0.0,
                elevation_m: Some(110.0),
                timestamp_ms: 10000,
                speed_kmh: Some(30.0),
                power_watts: Some(250.0),
                heart_rate_bpm: Some(140),
                cadence_rpm: Some(90.0),
                distance_from_start_m: 100.0,
                grade_percent: None,
            },
        ];
        let agg = Activity::compute_aggregates(&points);
        assert!((agg.total_distance_m - 100.0).abs() < 0.1);
        assert!((agg.elevation_gain_m - 10.0).abs() < 0.1);
        assert!((agg.duration_secs - 10.0).abs() < 0.1);
        assert!((agg.avg_power_watts.unwrap() - 200.0).abs() < 0.1);
        assert!((agg.max_power_watts.unwrap() - 250.0).abs() < 0.1);
        assert_eq!(agg.avg_heart_rate_bpm.unwrap(), 130);
        assert_eq!(agg.max_heart_rate_bpm.unwrap(), 140);
    }

    #[test]
    fn test_activity_source_display() {
        assert_eq!(ActivitySource::Recorded.to_string(), "recorded");
        assert_eq!(ActivitySource::GpxImport.to_string(), "gpx_import");
    }
}
