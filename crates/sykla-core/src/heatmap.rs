use std::collections::HashSet;
use std::f64::consts::PI;

/// Convert lat/lng to grid coordinates at a given zoom level.
/// Uses Web Mercator tile math (same as OSM/Leaflet slippy map tiles).
pub fn lat_lng_to_grid(lat: f64, lng: f64, zoom: u32) -> (i32, i32) {
    let n = (1u64 << zoom) as f64;
    let x = ((lng + 180.0) / 360.0 * n).floor() as i32;
    let lat_rad = lat.to_radians();
    let y = ((1.0 - lat_rad.tan().asinh() / PI) / 2.0 * n).floor() as i32;
    (x, y)
}

/// Convert grid coordinates back to lat/lng (returns the NW corner of the tile).
pub fn grid_to_lat_lng(grid_x: i32, grid_y: i32, zoom: u32) -> (f64, f64) {
    let n = (1u64 << zoom) as f64;
    let lng = grid_x as f64 / n * 360.0 - 180.0;
    let lat_rad = (PI * (1.0 - 2.0 * grid_y as f64 / n)).sinh().atan();
    let lat = lat_rad.to_degrees();
    (lat, lng)
}

/// Rasterize a GPS track into a set of grid cells using Bresenham's line algorithm
/// between consecutive points on the grid.
pub fn rasterize_track(points: &[(f64, f64)], zoom: u32) -> HashSet<(i32, i32)> {
    let mut cells = HashSet::new();

    if points.is_empty() {
        return cells;
    }

    let mut prev_grid = lat_lng_to_grid(points[0].0, points[0].1, zoom);
    cells.insert(prev_grid);

    for &(lat, lng) in &points[1..] {
        let curr_grid = lat_lng_to_grid(lat, lng, zoom);

        // Bresenham line between prev_grid and curr_grid
        bresenham_line(prev_grid.0, prev_grid.1, curr_grid.0, curr_grid.1, &mut cells);

        prev_grid = curr_grid;
    }

    cells
}

/// Bresenham's line algorithm for grid rasterization.
fn bresenham_line(x0: i32, y0: i32, x1: i32, y1: i32, cells: &mut HashSet<(i32, i32)>) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        cells.insert((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 {
                break;
            }
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lat_lng_to_grid_roundtrip() {
        let zoom = 15;
        let lat = 35.78;
        let lng = -78.64;
        let (gx, gy) = lat_lng_to_grid(lat, lng, zoom);
        let (lat2, lng2) = grid_to_lat_lng(gx, gy, zoom);

        // Should be within one tile's width
        let tile_size_deg = 360.0 / (1u64 << zoom) as f64;
        assert!((lat - lat2).abs() < tile_size_deg * 2.0);
        assert!((lng - lng2).abs() < tile_size_deg * 2.0);
    }

    #[test]
    fn test_lat_lng_to_grid_known() {
        // Zoom 0 should give (0, 0) for most points
        let (x, y) = lat_lng_to_grid(0.0, 0.0, 0);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }

    #[test]
    fn test_rasterize_single_point() {
        let points = vec![(35.78, -78.64)];
        let cells = rasterize_track(&points, 15);
        assert_eq!(cells.len(), 1);
    }

    #[test]
    fn test_rasterize_straight_line() {
        // Two nearby points should produce a connected line of cells
        let points = vec![(35.7800, -78.6400), (35.7810, -78.6390)];
        let cells = rasterize_track(&points, 15);
        assert!(cells.len() >= 2);
    }

    #[test]
    fn test_rasterize_same_point() {
        let points = vec![(35.78, -78.64), (35.78, -78.64)];
        let cells = rasterize_track(&points, 15);
        assert_eq!(cells.len(), 1);
    }

    #[test]
    fn test_bresenham_horizontal() {
        let mut cells = HashSet::new();
        bresenham_line(0, 0, 5, 0, &mut cells);
        assert_eq!(cells.len(), 6); // 0..=5
        for x in 0..=5 {
            assert!(cells.contains(&(x, 0)));
        }
    }

    #[test]
    fn test_bresenham_vertical() {
        let mut cells = HashSet::new();
        bresenham_line(0, 0, 0, 5, &mut cells);
        assert_eq!(cells.len(), 6);
        for y in 0..=5 {
            assert!(cells.contains(&(0, y)));
        }
    }

    #[test]
    fn test_bresenham_diagonal() {
        let mut cells = HashSet::new();
        bresenham_line(0, 0, 3, 3, &mut cells);
        assert!(cells.len() >= 4);
        assert!(cells.contains(&(0, 0)));
        assert!(cells.contains(&(3, 3)));
    }
}
