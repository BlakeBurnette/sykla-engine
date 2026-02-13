use crate::terrain::{RoutePoint, SurfaceType};
use crate::vegetation::VegetationConfig;

/// Visual style parameters that control how the world looks for a given route.
pub struct RouteStyle {
    /// Terrain mesh color [r, g, b, a]
    pub terrain_color: [f32; 4],
    /// Ground plane color [r, g, b, a]
    pub ground_color: [f32; 4],
    /// Road surface color [r, g, b, a]
    pub road_color: [f32; 4],
    /// Gravel surface color [r, g, b, a]
    pub gravel_color: [f32; 4],
    /// Trunk material color [r, g, b, a]
    pub trunk_color: [f32; 4],
    /// Deciduous canopy color [r, g, b, a]
    pub canopy_color: [f32; 4],
    /// Pine canopy color [r, g, b, a]
    pub pine_color: [f32; 4],
    /// Bush color [r, g, b, a]
    pub bush_color: [f32; 4],
    /// Vegetation density config
    pub vegetation: VegetationConfig,
}

impl RouteStyle {
    /// Lush temperate forest (default, Blue Ridge, ATT)
    fn forest() -> Self {
        Self {
            terrain_color: [0.32, 0.52, 0.15, 1.0],
            ground_color: [0.25, 0.40, 0.12, 1.0],
            road_color: [0.35, 0.30, 0.22, 1.0],
            gravel_color: [0.35, 0.30, 0.22, 1.0],
            trunk_color: [0.28, 0.16, 0.06, 1.0],
            canopy_color: [0.10, 0.28, 0.05, 1.0],
            pine_color: [0.06, 0.22, 0.04, 1.0],
            bush_color: [0.14, 0.35, 0.08, 1.0],
            vegetation: VegetationConfig::default(),
        }
    }

    /// High alpine — rocky grey terrain, sparse stunted trees, dark asphalt road
    fn alpine() -> Self {
        Self {
            terrain_color: [0.45, 0.42, 0.38, 1.0],
            ground_color: [0.35, 0.32, 0.28, 1.0],
            road_color: [0.18, 0.18, 0.20, 1.0],
            gravel_color: [0.18, 0.18, 0.20, 1.0],
            trunk_color: [0.25, 0.18, 0.10, 1.0],
            canopy_color: [0.15, 0.25, 0.10, 1.0],
            pine_color: [0.10, 0.20, 0.08, 1.0],
            bush_color: [0.20, 0.28, 0.12, 1.0],
            vegetation: VegetationConfig {
                spacing_m: 12.0,
                min_distance: 6.0,
                max_distance: 60.0,
                trees_per_slot: 2,
                min_scale: 0.3,
                max_scale: 0.7,
            },
        }
    }

    /// Barren moonscape — Mont Ventoux upper section: white limestone, almost no vegetation
    fn barren() -> Self {
        Self {
            terrain_color: [0.65, 0.62, 0.58, 1.0],
            ground_color: [0.55, 0.52, 0.48, 1.0],
            road_color: [0.20, 0.20, 0.22, 1.0],
            gravel_color: [0.20, 0.20, 0.22, 1.0],
            trunk_color: [0.30, 0.22, 0.12, 1.0],
            canopy_color: [0.22, 0.30, 0.15, 1.0],
            pine_color: [0.18, 0.25, 0.12, 1.0],
            bush_color: [0.28, 0.32, 0.18, 1.0],
            vegetation: VegetationConfig {
                spacing_m: 25.0,
                min_distance: 10.0,
                max_distance: 50.0,
                trees_per_slot: 1,
                min_scale: 0.2,
                max_scale: 0.5,
            },
        }
    }

    /// Coastal — sandy/scrubby terrain, ocean-influenced palette
    fn coastal() -> Self {
        Self {
            terrain_color: [0.45, 0.50, 0.28, 1.0],
            ground_color: [0.50, 0.48, 0.35, 1.0],
            road_color: [0.32, 0.32, 0.30, 1.0],
            gravel_color: [0.32, 0.32, 0.30, 1.0],
            trunk_color: [0.30, 0.20, 0.10, 1.0],
            canopy_color: [0.12, 0.30, 0.08, 1.0],
            pine_color: [0.08, 0.24, 0.06, 1.0],
            bush_color: [0.18, 0.32, 0.12, 1.0],
            vegetation: VegetationConfig {
                spacing_m: 8.0,
                min_distance: 5.0,
                max_distance: 70.0,
                trees_per_slot: 3,
                min_scale: 0.5,
                max_scale: 0.9,
            },
        }
    }

    /// Paved greenway trail — tree-lined corridor, dense canopy, asphalt path
    fn greenway() -> Self {
        Self {
            terrain_color: [0.30, 0.48, 0.18, 1.0],
            ground_color: [0.28, 0.42, 0.15, 1.0],
            road_color: [0.88, 0.85, 0.80, 1.0],
            gravel_color: [0.72, 0.60, 0.42, 1.0],
            trunk_color: [0.28, 0.16, 0.06, 1.0],
            canopy_color: [0.10, 0.28, 0.05, 1.0],
            pine_color: [0.06, 0.22, 0.04, 1.0],
            bush_color: [0.14, 0.35, 0.08, 1.0],
            vegetation: VegetationConfig {
                spacing_m: 4.0,
                min_distance: 7.0,
                max_distance: 40.0,
                trees_per_slot: 4,
                min_scale: 0.8,
                max_scale: 1.2,
            },
        }
    }
}

pub struct RouteInfo {
    pub name: &'static str,
    pub key: &'static str,
    pub description: &'static str,
}

pub fn catalog() -> Vec<RouteInfo> {
    vec![
        RouteInfo {
            name: "American Tobacco Trail",
            key: "att",
            description: "35 km rail-trail, Durham to New Hill NC",
        },
        RouteInfo {
            name: "Black Creek Greenway",
            key: "black-creek",
            description: "12 km creek-side trail, Cary NC",
        },
        RouteInfo {
            name: "Oak Creek Greenway",
            key: "oak-creek",
            description: "5.5 km wooded greenway, Cary NC",
        },
        RouteInfo {
            name: "Rolling Hills (Demo)",
            key: "demo",
            description: "Gentle 3 km loop with rolling terrain",
        },
        RouteInfo {
            name: "Alpe d'Huez",
            key: "alpe-dhuez",
            description: "13.8 km, avg 8.1%, 21 hairpin bends",
        },
        RouteInfo {
            name: "Mont Ventoux",
            key: "mont-ventoux",
            description: "21.5 km from Bedoin, avg 7.5%",
        },
        RouteInfo {
            name: "Stelvio Pass",
            key: "stelvio",
            description: "24.3 km, 48 hairpins, avg 7.4%",
        },
        RouteInfo {
            name: "Blue Ridge Parkway",
            key: "blue-ridge",
            description: "30 km rolling scenic drive, max 6%",
        },
        RouteInfo {
            name: "Pacific Coast Hwy",
            key: "pacific-coast",
            description: "25 km coastal road, max 5%",
        },
    ]
}

/// Returns the visual style for a given route key.
pub fn route_style(key: &str) -> RouteStyle {
    match key {
        "att" => RouteStyle::greenway(),
        "black-creek" => RouteStyle::greenway(),
        "oak-creek" => RouteStyle::greenway(),
        "demo" => RouteStyle::forest(),
        "alpe-dhuez" => RouteStyle::alpine(),
        "mont-ventoux" => RouteStyle::barren(),
        "stelvio" => RouteStyle::alpine(),
        "blue-ridge" => RouteStyle::forest(),
        "pacific-coast" => RouteStyle::coastal(),
        _ => RouteStyle::forest(),
    }
}

/// Generates route points at 10m intervals with curved path positions.
/// Greenway trails use real GPS data from GPX files for authentic path geometry.
pub fn generate_route(key: &str) -> Vec<RoutePoint> {
    let mut points = match key {
        "att" => {
            let gpx = include_str!("../../../assets/routes/american-tobacco-trail.gpx");
            crate::gpx::generate_from_gpx(gpx, &ATT_ELEVATION)
        }
        "black-creek" => {
            let gpx = include_str!("../../../assets/routes/black-creek-greenway.gpx");
            crate::gpx::generate_from_gpx(gpx, &BLACK_CREEK_ELEVATION)
        }
        "oak-creek" => {
            let gpx = include_str!("../../../assets/routes/white-oak-creek-greenway.gpx");
            crate::gpx::generate_from_gpx(gpx, &OAK_CREEK_ELEVATION)
        }
        "blue-ridge" => {
            let gpx = include_str!("../../../assets/routes/blue-ridge-parkway.gpx");
            crate::gpx::generate_from_gpx(gpx, &BLUE_RIDGE)
        }
        "demo" => generate_demo(),
        "alpe-dhuez" => generate_path(&ALPE_DHUEZ, &ALPE_DHUEZ_HEADINGS),
        "mont-ventoux" => generate_path(&MONT_VENTOUX, &MONT_VENTOUX_HEADINGS),
        "stelvio" => generate_path(&STELVIO, &STELVIO_HEADINGS),
        "pacific-coast" => generate_path(&PACIFIC_COAST, &PACIFIC_COAST_HEADINGS),
        _ => generate_demo(),
    };

    // Tag surface types for specific routes
    if key == "att" {
        // ATT switches from paved to gravel in the southern section
        let total = points.last().map(|p| p.distance_m).unwrap_or(1.0);
        let gravel_start = total * 0.63; // ~63% of the way (Green Level area)
        for p in &mut points {
            if p.distance_m >= gravel_start {
                p.surface = SurfaceType::Gravel;
            }
        }
    }

    points
}

// ============================================================================
// Path generation: elevation + heading waypoints → curved RoutePoints
// ============================================================================

/// Generate route points at 5m intervals from elevation + heading waypoint data.
/// Heading is in degrees (compass bearing: 0=N, 90=E, 180=S, 270=W).
/// Between waypoints, heading is smoothly interpolated using cosine blending.
/// Position is accumulated: pos += forward * 5m at each step.
fn generate_path(
    elevation_waypoints: &[(f64, f64)],
    heading_waypoints: &[(f64, f64)],
) -> Vec<RoutePoint> {
    if elevation_waypoints.len() < 2 || heading_waypoints.is_empty() {
        return Vec::new();
    }

    let total_dist = elevation_waypoints.last().unwrap().0;
    let num_points = (total_dist / 5.0).ceil() as usize + 1;
    let mut points = Vec::with_capacity(num_points);

    let mut elev_seg = 0usize;
    let mut pos_x: f64 = 0.0;
    let mut pos_z: f64 = 0.0;

    for i in 0..num_points {
        let d = (i as f64 * 5.0).min(total_dist);

        // Interpolate elevation
        while elev_seg + 1 < elevation_waypoints.len() - 1
            && elevation_waypoints[elev_seg + 1].0 < d
        {
            elev_seg += 1;
        }
        let (d0, e0) = elevation_waypoints[elev_seg];
        let (d1, e1) = elevation_waypoints[elev_seg + 1];
        let t = if (d1 - d0).abs() < 0.001 {
            0.0
        } else {
            ((d - d0) / (d1 - d0)).clamp(0.0, 1.0)
        };
        let elevation = e0 + t * (e1 - e0);

        // Interpolate heading with cosine blending and proper angle wrapping
        let heading_deg = interpolate_heading(heading_waypoints, d);
        let heading_rad = heading_deg.to_radians();

        // Compass heading: 0=North(+Z), 90=East(+X), 180=South(-Z), 270=West(-X)
        let forward_x = heading_rad.sin() as f32;
        let forward_z = heading_rad.cos() as f32;

        // Accumulate position (skip for first point)
        if i > 0 {
            let step = 5.0_f64.min(d - (i as f64 - 1.0) * 5.0);
            pos_x += forward_x as f64 * step;
            pos_z += forward_z as f64 * step;
        }

        points.push(RoutePoint {
            distance_m: d,
            elevation_m: elevation,
            pos_x: pos_x as f32,
            pos_z: pos_z as f32,
            forward_x,
            forward_z,
            surface: SurfaceType::Paved,
        });
    }

    points
}

/// Interpolate heading at distance d using cosine blending between heading waypoints.
/// Handles angle wrapping properly (e.g., 350° to 10° goes through 0°).
fn interpolate_heading(waypoints: &[(f64, f64)], d: f64) -> f64 {
    if waypoints.is_empty() {
        return 0.0;
    }
    if d <= waypoints[0].0 {
        return waypoints[0].1;
    }
    if d >= waypoints.last().unwrap().0 {
        return waypoints.last().unwrap().1;
    }

    // Find the segment
    let mut seg = 0usize;
    for i in 1..waypoints.len() {
        if waypoints[i].0 >= d {
            seg = i - 1;
            break;
        }
    }

    let (d0, h0) = waypoints[seg];
    let (d1, h1) = waypoints[seg + 1];
    let t = if (d1 - d0).abs() < 0.001 {
        0.0
    } else {
        ((d - d0) / (d1 - d0)).clamp(0.0, 1.0)
    };

    // Cosine interpolation for smooth transitions
    let t_smooth = (1.0 - (t * std::f64::consts::PI).cos()) * 0.5;

    // Shortest-arc angle interpolation
    let mut delta = h1 - h0;
    if delta > 180.0 {
        delta -= 360.0;
    }
    if delta < -180.0 {
        delta += 360.0;
    }

    let result = h0 + delta * t_smooth;
    // Normalize to 0..360
    ((result % 360.0) + 360.0) % 360.0
}

/// Demo route: gentle S-curves with sine waves.
fn generate_demo() -> Vec<RoutePoint> {
    let elevation: Vec<(f64, f64)> = (0..=30)
        .map(|i| {
            let d = i as f64 * 100.0;
            let elev = 100.0
                + 8.0 * (d / 700.0 * std::f64::consts::TAU).sin()
                + 1.5 * (d / 300.0 * std::f64::consts::TAU).sin();
            (d, elev)
        })
        .collect();

    let headings: Vec<(f64, f64)> = vec![
        (0.0, 0.0),
        (500.0, 15.0),
        (1000.0, 0.0),
        (1500.0, 345.0),
        (2000.0, 0.0),
        (2500.0, 20.0),
        (3000.0, 0.0),
    ];

    generate_path(&elevation, &headings)
}

// ============================================================================
// American Tobacco Trail — 35 km rail-trail, Durham to New Hill, NC
// Former Norfolk Southern railroad corridor. Paved multi-use trail.
// Very gentle grades (max ~3%) following the old railroad grade.
// Heading is generally SSW (~195°) with gentle curves following the railroad right-of-way.
// Elevation: 120m (Durham) dropping to 86m (New Hill).
// ============================================================================

static ATT_ELEVATION: [(f64, f64); 36] = [
    (0.0, 120.0),      // Durham Bulls Athletic Park area
    (1000.0, 118.5),
    (2000.0, 116.0),
    (3000.0, 113.5),
    (4000.0, 111.0),
    (5000.0, 109.0),    // South Durham
    (6000.0, 107.5),
    (7000.0, 106.0),
    (8000.0, 105.0),
    (9000.0, 107.0),    // Slight rise before I-40 tunnel
    (10000.0, 109.0),   // I-40 pedestrian tunnel
    (11000.0, 107.0),
    (12000.0, 105.0),
    (13000.0, 103.0),
    (14000.0, 101.0),   // RTP area - Fayetteville Rd crossing
    (15000.0, 100.0),
    (16000.0, 98.5),
    (17000.0, 97.0),
    (18000.0, 96.0),
    (19000.0, 97.5),    // Gentle rise approaching NC 54
    (20000.0, 99.0),    // NC 54 crossing
    (21000.0, 100.0),
    (22000.0, 98.5),
    (23000.0, 97.0),    // Entering Cary/Apex area
    (24000.0, 95.0),
    (25000.0, 93.5),
    (26000.0, 92.0),    // White Oak Creek crossing
    (27000.0, 91.0),
    (28000.0, 93.0),    // Gentle rise past creek
    (29000.0, 94.0),
    (30000.0, 92.0),    // Green Level area
    (31000.0, 90.5),
    (32000.0, 89.0),    // Approaching Friendship
    (33000.0, 88.0),
    (34000.0, 87.0),
    (35000.0, 86.0),    // New Hill terminus
];

// ============================================================================
// Black Creek Greenway — 12 km creek-side trail, Cary NC
// Follows Black Creek south through forested corridor.
// More winding than ATT — follows the creek's natural meandering.
// Elevation: 105m (Lake Crabtree) to 88m (Regency Pkwy area).
// Dense Piedmont hardwood/pine forest canopy.
// ============================================================================

static BLACK_CREEK_ELEVATION: [(f64, f64); 25] = [
    (0.0, 105.0),       // Lake Crabtree County Park
    (500.0, 104.0),
    (1000.0, 102.5),
    (1500.0, 101.0),
    (2000.0, 99.5),     // Following creek south
    (2500.0, 98.0),
    (3000.0, 97.0),
    (3500.0, 96.0),
    (4000.0, 95.0),     // Weston Pkwy underpass
    (4500.0, 94.5),
    (5000.0, 93.5),
    (5500.0, 93.0),
    (6000.0, 92.0),     // NW Maynard Rd crossing
    (6500.0, 91.5),
    (7000.0, 91.0),
    (7500.0, 92.0),     // Gentle rise
    (8000.0, 93.0),
    (8500.0, 92.0),
    (9000.0, 91.0),
    (9500.0, 90.0),
    (10000.0, 89.5),    // Lake Johnson area
    (10500.0, 89.0),
    (11000.0, 88.5),
    (11500.0, 88.0),
    (12000.0, 88.0),    // Regency Pkwy terminus
];

// ============================================================================
// Oak Creek Greenway — 5.5 km wooded greenway, Cary NC
// Connects Annie Jones Park to Fred Bond Metro Park.
// Follows Oak Creek westward through residential/wooded areas.
// Elevation: 95m to 102m, gently rolling Piedmont terrain.
// Dense mixed hardwood/pine forest.
// ============================================================================

static OAK_CREEK_ELEVATION: [(f64, f64); 12] = [
    (0.0, 95.0),        // Annie Jones Park (east end)
    (500.0, 96.0),
    (1000.0, 97.0),
    (1500.0, 98.5),     // Gentle rise
    (2000.0, 100.0),
    (2500.0, 101.0),
    (3000.0, 100.0),    // Slight dip at creek crossing
    (3500.0, 99.0),
    (4000.0, 100.5),
    (4500.0, 101.5),
    (5000.0, 102.0),    // Approaching Bond Park
    (5500.0, 101.0),    // Fred Bond Metro Park
];

// ============================================================================
// European / Scenic Routes (kept for variety)
// ============================================================================

/// Alpe d'Huez — 13.8 km, 21 hairpin bends.
static ALPE_DHUEZ: [(f64, f64); 15] = [
    (0.0, 720.0), (1000.0, 800.0), (2000.0, 890.0), (3000.0, 990.0),
    (4000.0, 1070.0), (5000.0, 1140.0), (6000.0, 1215.0), (7000.0, 1290.0),
    (8000.0, 1370.0), (9000.0, 1445.0), (10000.0, 1520.0), (11000.0, 1610.0),
    (12000.0, 1700.0), (13000.0, 1790.0), (13800.0, 1850.0),
];

/// Alpe d'Huez headings — 21 hairpin switchbacks alternating direction.
static ALPE_DHUEZ_HEADINGS: [(f64, f64); 28] = [
    (0.0, 30.0),     // Start heading NNE
    (600.0, 30.0),   // Hairpin 21
    (650.0, 210.0),  // Reverse
    (1300.0, 210.0), // Hairpin 20
    (1350.0, 30.0),
    (1900.0, 30.0),  // Hairpin 19
    (1950.0, 210.0),
    (2600.0, 210.0), // Hairpin 18
    (2650.0, 30.0),
    (3300.0, 30.0),  // Hairpin 17
    (3350.0, 210.0),
    (4100.0, 210.0), // Hairpin 16
    (4150.0, 30.0),
    (5000.0, 30.0),  // Hairpin 15
    (5050.0, 210.0),
    (6000.0, 210.0), // Hairpin 14
    (6050.0, 30.0),
    (7000.0, 30.0),  // Hairpin 13
    (7050.0, 210.0),
    (8200.0, 210.0), // Hairpin 12
    (8250.0, 30.0),
    (9400.0, 30.0),  // Hairpin 11
    (9450.0, 210.0),
    (10800.0, 210.0), // Hairpin 10
    (10850.0, 30.0),
    (12200.0, 30.0),  // Hairpin 9
    (12250.0, 210.0),
    (13800.0, 210.0), // Summit
];

/// Mont Ventoux — 21.5 km, gradual curves through forest then exposed summit.
static MONT_VENTOUX: [(f64, f64); 23] = [
    (0.0, 310.0), (1000.0, 340.0), (2000.0, 380.0), (3000.0, 430.0),
    (4000.0, 510.0), (5000.0, 580.0), (6000.0, 660.0), (7000.0, 740.0),
    (8000.0, 810.0), (9000.0, 890.0), (10000.0, 970.0), (11000.0, 1050.0),
    (12000.0, 1130.0), (13000.0, 1200.0), (14000.0, 1280.0), (15000.0, 1360.0),
    (16000.0, 1445.0), (17000.0, 1520.0), (18000.0, 1590.0), (19000.0, 1665.0),
    (20000.0, 1740.0), (21000.0, 1830.0), (21500.0, 1909.0),
];

static MONT_VENTOUX_HEADINGS: [(f64, f64); 10] = [
    (0.0, 350.0), (3000.0, 5.0), (6000.0, 355.0), (9000.0, 10.0),
    (12000.0, 0.0), (15000.0, 345.0), (17000.0, 355.0), (19000.0, 5.0),
    (20500.0, 350.0), (21500.0, 355.0),
];

/// Stelvio Pass — 24.3 km, 48 hairpins.
static STELVIO: [(f64, f64); 26] = [
    (0.0, 900.0), (1000.0, 960.0), (2000.0, 1030.0), (3000.0, 1100.0),
    (4000.0, 1175.0), (5000.0, 1250.0), (6000.0, 1320.0), (7000.0, 1395.0),
    (8000.0, 1465.0), (9000.0, 1535.0), (10000.0, 1600.0), (11000.0, 1680.0),
    (12000.0, 1750.0), (13000.0, 1830.0), (14000.0, 1900.0), (15000.0, 1975.0),
    (16000.0, 2040.0), (17000.0, 2110.0), (18000.0, 2180.0), (19000.0, 2250.0),
    (20000.0, 2325.0), (21000.0, 2420.0), (22000.0, 2520.0), (23000.0, 2630.0),
    (24000.0, 2730.0), (24300.0, 2758.0),
];

/// Stelvio headings — many tight switchbacks with 500m between hairpins.
static STELVIO_HEADINGS: [(f64, f64); 30] = [
    (0.0, 320.0),
    (500.0, 320.0), (550.0, 140.0),
    (1050.0, 140.0), (1100.0, 320.0),
    (1600.0, 320.0), (1650.0, 140.0),
    (2200.0, 140.0), (2250.0, 320.0),
    (3000.0, 320.0), (3050.0, 140.0),
    (3800.0, 140.0), (3850.0, 320.0),
    (4600.0, 320.0), (4650.0, 140.0),
    (5500.0, 140.0), (5550.0, 320.0),
    (6500.0, 320.0), (6550.0, 140.0),
    (7500.0, 140.0), (7550.0, 320.0),
    (9000.0, 320.0), (9050.0, 140.0),
    (11000.0, 140.0), (11050.0, 320.0),
    (14000.0, 320.0), (14050.0, 140.0),
    (18000.0, 140.0), (18050.0, 320.0),
    (24300.0, 320.0),
];

/// Blue Ridge Parkway — 30 km, sweeping Appalachian curves.
static BLUE_RIDGE: [(f64, f64); 31] = [
    (0.0, 600.0), (1000.0, 610.0), (2000.0, 625.0), (3000.0, 645.0),
    (4000.0, 660.0), (5000.0, 650.0), (6000.0, 640.0), (7000.0, 655.0),
    (8000.0, 680.0), (9000.0, 710.0), (10000.0, 730.0), (11000.0, 720.0),
    (12000.0, 700.0), (13000.0, 690.0), (14000.0, 710.0), (15000.0, 740.0),
    (16000.0, 755.0), (17000.0, 770.0), (18000.0, 760.0), (19000.0, 745.0),
    (20000.0, 730.0), (21000.0, 740.0), (22000.0, 760.0), (23000.0, 780.0),
    (24000.0, 795.0), (25000.0, 780.0), (26000.0, 760.0), (27000.0, 750.0),
    (28000.0, 770.0), (29000.0, 790.0), (30000.0, 800.0),
];

/// Pacific Coast Highway — 25 km coastal section.
static PACIFIC_COAST: [(f64, f64); 26] = [
    (0.0, 15.0), (1000.0, 20.0), (2000.0, 35.0), (3000.0, 55.0),
    (4000.0, 70.0), (5000.0, 65.0), (6000.0, 45.0), (7000.0, 30.0),
    (8000.0, 20.0), (9000.0, 25.0), (10000.0, 40.0), (11000.0, 60.0),
    (12000.0, 75.0), (13000.0, 80.0), (14000.0, 65.0), (15000.0, 45.0),
    (16000.0, 30.0), (17000.0, 20.0), (18000.0, 15.0), (19000.0, 25.0),
    (20000.0, 45.0), (21000.0, 60.0), (22000.0, 50.0), (23000.0, 35.0),
    (24000.0, 20.0), (25000.0, 15.0),
];

static PACIFIC_COAST_HEADINGS: [(f64, f64); 14] = [
    (0.0, 315.0), (2000.0, 330.0), (4000.0, 310.0), (6000.0, 325.0),
    (8000.0, 340.0), (10000.0, 320.0), (12000.0, 305.0), (14000.0, 325.0),
    (16000.0, 340.0), (18000.0, 315.0), (20000.0, 330.0), (22000.0, 310.0),
    (24000.0, 325.0), (25000.0, 320.0),
];
