use crate::dem::GeoOrigin;
use crate::terrain::{DemTerrainSource, RoutePoint, SurfaceType, TerrainConfig};
use crate::vegetation::VegetationConfig;

#[derive(Clone, Copy, PartialEq)]
pub enum RoadMarkings {
    None,
    European,
}

/// Visual style parameters that control how the world looks for a given route.
pub struct RouteStyle {
    /// Terrain mesh color [r, g, b, a] — low elevation / base zone
    pub terrain_color: [f32; 4],
    /// Mid elevation terrain color (rocky grey for alpine)
    pub terrain_mid_color: [f32; 4],
    /// High elevation terrain color (snow white for alpine)
    pub terrain_high_color: [f32; 4],
    /// Elevation zone transition bands [low_start, low_end, high_start, high_end]
    pub elevation_zones: [f32; 4],
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
    /// Fog / atmospheric haze color [r, g, b, base_elevation]
    pub fog_color: [f32; 4],
    /// Sky clear color
    pub sky_color: [f32; 3],
    /// Road marking style
    pub road_markings: RoadMarkings,
    /// Cabin spacing in meters (0.0 = no cabins)
    pub cabin_spacing_m: f32,
    /// Whether to generate background mountains
    pub mountains: bool,
    /// Light direction FROM sun TO scene (normalized)
    pub light_direction: [f32; 3],
    /// Key light RGBA
    pub light_color: [f32; 4],
    /// Fill/ambient RGBA
    pub light_ambient: [f32; 4],
    /// Terrain falloff — how steeply terrain drops from road center (0.08 flat, 0.35 mountain)
    pub terrain_falloff: f32,
    /// Terrain noise amplitude — 1.0 for flat trails, 30-50 for mountain terrain
    pub terrain_noise_amplitude: f32,
    /// Optional DEM data for real elevation terrain (embedded via include_bytes)
    pub dem_data: Option<&'static [u8]>,
    /// Override terrain half-width (None = default: 1500 DEM, 300 procedural)
    pub dem_half_width: Option<f32>,
    /// Override terrain cross-section count (None = default: 100 DEM, 48 procedural)
    pub dem_cross_sections: Option<usize>,
    /// Terrain splatting params [path_blend_width, slope_threshold, noise_scale, enable]
    pub terrain_params: [f32; 4],
}

impl RouteStyle {
    pub fn terrain_config(&self, dem: Option<DemTerrainSource>) -> TerrainConfig {
        let is_dem = dem.is_some();
        TerrainConfig {
            half_width: self.dem_half_width.unwrap_or(if is_dem { 1500.0 } else { 300.0 }),
            cross_sections: self.dem_cross_sections.unwrap_or(if is_dem { 100 } else { 48 }),
            falloff: self.terrain_falloff,
            noise_amplitude: self.terrain_noise_amplitude,
            elevation_scale: 1.0,
            dem,
        }
    }

    /// Lush temperate forest (default, Blue Ridge, ATT)
    fn forest() -> Self {
        Self {
            terrain_color: [0.32, 0.52, 0.15, 1.0],
            terrain_mid_color: [0.32, 0.52, 0.15, 1.0],
            terrain_high_color: [0.32, 0.52, 0.15, 1.0],
            elevation_zones: [99999.0; 4],
            ground_color: [0.25, 0.40, 0.12, 1.0],
            road_color: [0.35, 0.30, 0.22, 1.0],
            gravel_color: [0.35, 0.30, 0.22, 1.0],
            trunk_color: [0.28, 0.16, 0.06, 1.0],
            canopy_color: [0.10, 0.28, 0.05, 1.0],
            pine_color: [0.06, 0.22, 0.04, 1.0],
            bush_color: [0.14, 0.35, 0.08, 1.0],
            vegetation: VegetationConfig {
                min_distance: 6.0,
                max_distance: 130.0,
                use_eastern_species: true,
                fallen_tree_probability: 0.005,
                ..Default::default()
            },
            fog_color: [0.72, 0.68, 0.52, 80.0],
            sky_color: [0.52, 0.70, 0.82],
            road_markings: RoadMarkings::None,
            cabin_spacing_m: 0.0,
            mountains: false,
            light_direction: [-0.4, -0.45, -0.35],
            light_color: [1.0, 0.88, 0.65, 1.0],
            light_ambient: [0.35, 0.32, 0.22, 1.0],
            terrain_falloff: 0.08,
            terrain_noise_amplitude: 1.0,
            dem_data: None,
            dem_half_width: None,
            dem_cross_sections: None,
            // biome 5 = generic lush forest
            terrain_params: [0.10, 0.25, 1.0, 5.0],
        }
    }

    /// High alpine — green meadow base → rocky grey mid → snow white summit
    fn alpine() -> Self {
        Self {
            terrain_color: [0.35, 0.55, 0.20, 1.0],       // Lush green meadow
            terrain_mid_color: [0.36, 0.48, 0.54, 1.0],    // Slate blue-grey rock
            terrain_high_color: [0.88, 0.90, 0.95, 1.0],   // Blue-white snow
            elevation_zones: [1700.0, 1900.0, 2000.0, 2200.0],
            ground_color: [0.35, 0.32, 0.28, 1.0],
            road_color: [0.18, 0.18, 0.20, 1.0],
            gravel_color: [0.18, 0.18, 0.20, 1.0],
            trunk_color: [0.25, 0.18, 0.10, 1.0],
            canopy_color: [0.15, 0.25, 0.10, 1.0],
            pine_color: [0.10, 0.20, 0.08, 1.0],
            bush_color: [0.20, 0.28, 0.12, 1.0],
            vegetation: VegetationConfig {
                spacing_m: 6.0,
                min_distance: 6.0,
                max_distance: 90.0,
                trees_per_slot: 4,
                min_scale: 0.3,
                max_scale: 0.7,
                treeline: 1900.0,
                ..Default::default()
            },
            fog_color: [0.78, 0.82, 0.90, 700.0],
            sky_color: [0.55, 0.72, 0.88],
            road_markings: RoadMarkings::European,
            cabin_spacing_m: 500.0,
            mountains: true,
            light_direction: [-0.4, -0.45, -0.35],
            light_color: [1.0, 0.88, 0.65, 1.0],
            light_ambient: [0.35, 0.32, 0.22, 1.0],
            terrain_falloff: 0.35,
            terrain_noise_amplitude: 40.0,
            dem_data: None,
            dem_half_width: None,
            dem_cross_sections: None,
            // biome 2 = alpine (meadow→rock→snow)
            terrain_params: [0.05, 0.20, 1.5, 2.0],
        }
    }

    /// High alpine winter — snow from start, Col de la Loze
    fn high_alpine_winter() -> Self {
        Self {
            terrain_color: [0.55, 0.58, 0.52, 1.0],       // Winter grey-green (barely visible, transitions fast)
            terrain_mid_color: [0.36, 0.48, 0.54, 1.0],    // Slate blue-grey rock (#5B7B8A)
            terrain_high_color: [0.94, 0.94, 0.91, 1.0],   // Deep snow (#F0F0E8)
            elevation_zones: [1100.0, 1300.0, 1500.0, 1700.0],
            ground_color: [0.36, 0.48, 0.54, 1.0],         // Rock color
            road_color: [0.23, 0.23, 0.26, 1.0],           // Dark grey asphalt (#3A3A42)
            gravel_color: [0.22, 0.22, 0.24, 1.0],
            trunk_color: [0.25, 0.18, 0.10, 1.0],
            canopy_color: [0.15, 0.25, 0.10, 1.0],
            pine_color: [0.18, 0.35, 0.15, 1.0],           // Brighter pine (#2D5A27)
            bush_color: [0.20, 0.28, 0.12, 1.0],
            vegetation: VegetationConfig {
                spacing_m: 6.0,
                min_distance: 6.0,
                max_distance: 90.0,
                trees_per_slot: 4,
                min_scale: 0.3,
                max_scale: 0.7,
                treeline: 1800.0,
                ..Default::default()
            },
            fog_color: [0.85, 0.88, 0.95, 1600.0],        // White-blue horizon
            sky_color: [0.55, 0.72, 0.88],                 // Warm cerulean
            road_markings: RoadMarkings::European,
            cabin_spacing_m: 500.0,
            mountains: true,
            light_direction: [-0.71, -0.57, -0.41],        // 35deg elev, 60deg azimuth
            light_color: [1.0, 0.96, 0.88, 1.0],           // Warm golden key #FFF4E0
            light_ambient: [0.38, 0.48, 0.53, 1.0],        // Cool blue fill
            terrain_falloff: 0.35,
            terrain_noise_amplitude: 40.0,
            dem_data: None,
            dem_half_width: None,
            dem_cross_sections: None,
            // biome 2 = alpine (meadow→rock→snow)
            terrain_params: [0.05, 0.20, 1.5, 2.0],
        }
    }

    /// Barren moonscape — Mont Ventoux upper section: white limestone, almost no vegetation
    fn barren() -> Self {
        Self {
            terrain_color: [0.65, 0.62, 0.58, 1.0],
            terrain_mid_color: [0.65, 0.62, 0.58, 1.0],
            terrain_high_color: [0.65, 0.62, 0.58, 1.0],
            elevation_zones: [99999.0; 4],
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
                max_distance: 70.0,
                trees_per_slot: 1,
                min_scale: 0.2,
                max_scale: 0.5,
                ..Default::default()
            },
            fog_color: [0.72, 0.68, 0.52, 300.0],
            sky_color: [0.52, 0.70, 0.82],
            road_markings: RoadMarkings::None,
            cabin_spacing_m: 0.0,
            mountains: false,
            light_direction: [-0.4, -0.45, -0.35],
            light_color: [1.0, 0.88, 0.65, 1.0],
            light_ambient: [0.35, 0.32, 0.22, 1.0],
            terrain_falloff: 0.15,
            terrain_noise_amplitude: 10.0,
            dem_data: None,
            dem_half_width: None,
            dem_cross_sections: None,
            // biome 3 = desert/barren
            terrain_params: [0.03, 0.15, 2.0, 3.0],
        }
    }

    /// Coastal — sandy/scrubby terrain, ocean-influenced palette
    fn coastal() -> Self {
        Self {
            terrain_color: [0.45, 0.50, 0.28, 1.0],
            terrain_mid_color: [0.45, 0.50, 0.28, 1.0],
            terrain_high_color: [0.45, 0.50, 0.28, 1.0],
            elevation_zones: [99999.0; 4],
            ground_color: [0.50, 0.48, 0.35, 1.0],
            road_color: [0.32, 0.32, 0.30, 1.0],
            gravel_color: [0.32, 0.32, 0.30, 1.0],
            trunk_color: [0.30, 0.20, 0.10, 1.0],
            canopy_color: [0.12, 0.30, 0.08, 1.0],
            pine_color: [0.08, 0.24, 0.06, 1.0],
            bush_color: [0.18, 0.32, 0.12, 1.0],
            vegetation: VegetationConfig {
                spacing_m: 8.0,
                min_distance: 6.0,
                max_distance: 100.0,
                trees_per_slot: 3,
                min_scale: 0.5,
                max_scale: 0.9,
                use_eastern_species: true,
                fallen_tree_probability: 0.005,
                ..Default::default()
            },
            fog_color: [0.72, 0.68, 0.52, 10.0],
            sky_color: [0.52, 0.70, 0.82],
            road_markings: RoadMarkings::None,
            cabin_spacing_m: 0.0,
            mountains: false,
            light_direction: [-0.4, -0.45, -0.35],
            light_color: [1.0, 0.88, 0.65, 1.0],
            light_ambient: [0.35, 0.32, 0.22, 1.0],
            terrain_falloff: 0.08,
            terrain_noise_amplitude: 1.0,
            dem_data: None,
            dem_half_width: None,
            dem_cross_sections: None,
            // biome 4 = coastal
            terrain_params: [0.12, 0.35, 0.8, 4.0],
        }
    }

    /// Paved greenway trail — tree-lined corridor, dense canopy, asphalt path
    fn greenway() -> Self {
        Self {
            terrain_color: [0.30, 0.48, 0.18, 1.0],
            terrain_mid_color: [0.30, 0.48, 0.18, 1.0],
            terrain_high_color: [0.30, 0.48, 0.18, 1.0],
            elevation_zones: [99999.0; 4],
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
                max_distance: 60.0,
                trees_per_slot: 4,
                min_scale: 0.8,
                max_scale: 1.2,
                use_eastern_species: true,
                fallen_tree_probability: 0.005,
                ..Default::default()
            },
            fog_color: [0.72, 0.68, 0.52, 80.0],
            sky_color: [0.52, 0.70, 0.82],
            road_markings: RoadMarkings::None,
            cabin_spacing_m: 0.0,
            mountains: false,
            light_direction: [-0.4, -0.45, -0.35],
            light_color: [1.0, 0.88, 0.65, 1.0],
            light_ambient: [0.35, 0.32, 0.22, 1.0],
            terrain_falloff: 0.08,
            terrain_noise_amplitude: 1.0,
            dem_data: None,
            dem_half_width: Some(800.0),
            dem_cross_sections: Some(80),
            // biome 1 = NC Piedmont forest floor
            terrain_params: [0.15, 0.3, 1.0, 1.0],
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
        RouteInfo {
            name: "Col de la Loze",
            key: "col-de-la-loze",
            description: "15 km, avg 6%, summit 2299m (TdF 2025)",
        },
    ]
}

/// Returns the visual style for a given route key.
pub fn route_style(key: &str) -> RouteStyle {
    match key {
        "att" => {
            #[cfg(feature = "dem")]
            { let mut s = RouteStyle::greenway(); s.dem_data = Some(include_bytes!("../../../assets/routes/att.dem")); s }
            #[cfg(not(feature = "dem"))]
            RouteStyle::greenway()
        }
        "black-creek" => {
            #[cfg(feature = "dem")]
            { let mut s = RouteStyle::greenway(); s.dem_data = Some(include_bytes!("../../../assets/routes/black-creek.dem")); s }
            #[cfg(not(feature = "dem"))]
            RouteStyle::greenway()
        }
        "oak-creek" => {
            #[cfg(feature = "dem")]
            { let mut s = RouteStyle::greenway(); s.dem_data = Some(include_bytes!("../../../assets/routes/oak-creek.dem")); s }
            #[cfg(not(feature = "dem"))]
            RouteStyle::greenway()
        }
        "demo" => RouteStyle::forest(),
        "alpe-dhuez" => RouteStyle::alpine(),
        "mont-ventoux" => RouteStyle::barren(),
        "stelvio" => RouteStyle::alpine(),
        "blue-ridge" => RouteStyle::forest(),
        "pacific-coast" => RouteStyle::coastal(),
        "col-de-la-loze" => {
            #[cfg(feature = "dem")]
            {
                let mut style = RouteStyle::high_alpine_winter();
                style.dem_data = Some(include_bytes!("../../../assets/routes/col-de-la-loze.dem"));
                style
            }
            #[cfg(not(feature = "dem"))]
            RouteStyle::high_alpine_winter()
        }
        _ => RouteStyle::forest(),
    }
}

/// Generates route points at 10m intervals with curved path positions.
/// Greenway trails use real GPS data from GPX files for authentic path geometry.
/// Returns the route points and optionally a GeoOrigin for GPX-based routes.
pub fn generate_route(key: &str) -> (Vec<RoutePoint>, Option<GeoOrigin>) {
    let (mut points, geo_origin) = match key {
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
        "demo" => (generate_demo(), None),
        "alpe-dhuez" => (generate_path(&ALPE_DHUEZ, &ALPE_DHUEZ_HEADINGS), None),
        "mont-ventoux" => (generate_path(&MONT_VENTOUX, &MONT_VENTOUX_HEADINGS), None),
        "stelvio" => (generate_path(&STELVIO, &STELVIO_HEADINGS), None),
        "pacific-coast" => (generate_path(&PACIFIC_COAST, &PACIFIC_COAST_HEADINGS), None),
        "col-de-la-loze" => {
            let gpx = include_str!("../../../assets/routes/col-de-la-loze.gpx");
            crate::gpx::generate_from_gpx(gpx, &COL_DE_LA_LOZE)
        }
        _ => (generate_demo(), None),
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

    crate::terrain::compute_curvatures(&mut points);

    (points, geo_origin)
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
            banking: 0.0,
            geo_x: pos_x as f32,
            geo_z: pos_z as f32,
            curvature: 0.0,
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

// GPX covers ~10.5km from Durham DBAP area south to Apex/I-40 tunnel area.
// Rail trail: very gentle grades, max ~2%. Drops ~22m over 10.5km.
static ATT_ELEVATION: [(f64, f64); 14] = [
    (0.0, 120.0),       // Durham Bulls Athletic Park area
    (800.0, 119.0),
    (1600.0, 117.5),
    (2400.0, 116.0),    // South Durham
    (3200.0, 114.5),
    (4000.0, 113.0),
    (4800.0, 111.5),
    (5600.0, 110.0),    // Approaching I-40
    (6400.0, 108.5),
    (7200.0, 107.0),    // I-40 area
    (8000.0, 105.5),
    (8800.0, 104.0),    // RTP corridor
    (9600.0, 102.5),
    (10500.0, 101.0),   // South end of GPX data
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

// ============================================================================
// Col de la Loze — upper half, TdF 2025 Stage 18 summit finish
// 15.1 km, 899m gain (1400m to 2299m), avg 6.0%
// One of the highest and steepest summit finishes in Tour de France history.
// Real GPS path from TdF 2025 race route above Méribel/Courchevel.
// Brutal final ramps of 9.3% above 2100m, brief descent at ridge crossing.
// ============================================================================

static COL_DE_LA_LOZE: [(f64, f64); 32] = [
    (0.0, 1400.0),       // Start: above Méribel
    (500.0, 1420.0),
    (1000.0, 1459.2),
    (1500.0, 1494.9),
    (2000.0, 1518.5),
    (2500.0, 1556.0),
    (3000.0, 1575.0),
    (3500.0, 1592.9),
    (4000.0, 1631.8),
    (4500.0, 1670.4),
    (5000.0, 1691.3),    // 5 km
    (5500.0, 1724.7),
    (6000.0, 1744.0),
    (6500.0, 1764.8),
    (7000.0, 1797.0),
    (7500.0, 1827.8),
    (8000.0, 1859.9),
    (8500.0, 1880.4),
    (9000.0, 1915.1),
    (9500.0, 1940.0),
    (10000.0, 1982.8),   // 10 km — approaching 2000m
    (10500.0, 2034.4),
    (11000.0, 2070.5),
    (11500.0, 2048.0),   // Ridge dip — brief descent
    (12000.0, 2063.7),
    (12500.0, 2104.0),
    (13000.0, 2135.6),
    (13500.0, 2188.0),   // Steepening above 2100m
    (14000.0, 2229.0),
    (14500.0, 2242.0),
    (15000.0, 2294.4),   // 15 km
    (15058.8, 2299.0),   // Summit: Col de la Loze, 2299m
];
