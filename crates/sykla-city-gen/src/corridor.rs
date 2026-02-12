use sykla_core::geo_math::haversine_distance;
use sykla_core::types::RoutePoint;

use crate::parser::{BuildingFeature, PolygonFeature, RoadFeature};

/// Compute the centroid of a polygon ring as (lat, lng).
fn centroid(ring: &[(f64, f64)]) -> (f64, f64) {
    let n = ring.len() as f64;
    let sum_lat: f64 = ring.iter().map(|(lat, _)| lat).sum();
    let sum_lng: f64 = ring.iter().map(|(_, lng)| lng).sum();
    (sum_lat / n, sum_lng / n)
}

/// Minimum distance from a point to a polyline (sequence of route points), in meters.
fn distance_to_route(lat: f64, lng: f64, route_points: &[RoutePoint]) -> f64 {
    // Check distance to each route point (fast approximation — good enough for filtering)
    route_points
        .iter()
        .map(|p| haversine_distance(lat, lng, p.lat, p.lng))
        .fold(f64::MAX, f64::min)
}

/// Compute a bounding box that encloses the route expanded by corridor_m meters.
/// Returns (south, west, north, east).
pub fn route_bbox(route_points: &[RoutePoint], corridor_m: f64) -> (f64, f64, f64, f64) {
    let mut min_lat = f64::MAX;
    let mut max_lat = f64::MIN;
    let mut min_lng = f64::MAX;
    let mut max_lng = f64::MIN;

    for p in route_points {
        if p.lat < min_lat { min_lat = p.lat; }
        if p.lat > max_lat { max_lat = p.lat; }
        if p.lng < min_lng { min_lng = p.lng; }
        if p.lng > max_lng { max_lng = p.lng; }
    }

    // Expand by corridor distance (convert meters to approximate degrees)
    let lat_expand = corridor_m / 111_320.0;
    let avg_lat = (min_lat + max_lat) / 2.0;
    let lng_expand = corridor_m / (111_320.0 * avg_lat.to_radians().cos());

    (
        min_lat - lat_expand,
        min_lng - lng_expand,
        max_lat + lat_expand,
        max_lng + lng_expand,
    )
}

/// Compute the center point of a route (midpoint by count).
pub fn route_center(route_points: &[RoutePoint]) -> (f64, f64) {
    let n = route_points.len() as f64;
    let sum_lat: f64 = route_points.iter().map(|p| p.lat).sum();
    let sum_lng: f64 = route_points.iter().map(|p| p.lng).sum();
    (sum_lat / n, sum_lng / n)
}

/// Filter buildings to those whose centroid is within corridor_m of the route.
pub fn filter_buildings(
    features: Vec<BuildingFeature>,
    route_points: &[RoutePoint],
    corridor_m: f64,
) -> Vec<BuildingFeature> {
    features
        .into_iter()
        .filter(|f| {
            let (lat, lng) = centroid(&f.ring);
            distance_to_route(lat, lng, route_points) <= corridor_m
        })
        .collect()
}

/// Filter road features to those with any vertex within corridor_m of the route.
pub fn filter_roads(
    features: Vec<RoadFeature>,
    route_points: &[RoutePoint],
    corridor_m: f64,
) -> Vec<RoadFeature> {
    features
        .into_iter()
        .filter(|f| {
            // A road is included if any point on its centerline is near the route
            f.centerline.iter().any(|&(lat, lng)| {
                distance_to_route(lat, lng, route_points) <= corridor_m
            })
        })
        .collect()
}

/// Filter polygon features to those whose centroid is within corridor_m of the route.
pub fn filter_polygons(
    features: Vec<PolygonFeature>,
    route_points: &[RoutePoint],
    corridor_m: f64,
) -> Vec<PolygonFeature> {
    features
        .into_iter()
        .filter(|f| {
            let (lat, lng) = centroid(&f.ring);
            distance_to_route(lat, lng, route_points) <= corridor_m
        })
        .collect()
}
