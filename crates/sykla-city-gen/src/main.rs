mod overpass;
mod parser;
mod projection;
mod buildings;
mod landuse;
mod water;
mod roads;
mod parking;
mod format;
mod corridor;
mod graph;
mod upload;

use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use sykla_world::city::{batch_meshes, CityMesh, FeatureSubtype};

#[derive(Parser)]
#[command(name = "sykla-city-gen", about = "Generate city data from OpenStreetMap")]
struct Cli {
    /// City/route name (e.g. "cary-nc" or "cary-greenway-loop")
    #[arg(long)]
    name: String,

    /// Bounding box: "south,west,north,east" (lat/lng).
    /// Not needed if --route is provided (computed automatically).
    #[arg(long)]
    bbox: Option<String>,

    /// Center point: "lat,lng".
    /// Not needed if --route is provided (computed automatically).
    #[arg(long)]
    center: Option<String>,

    /// Path to a GPX file. Bbox and center are derived from the route.
    /// Only OSM features within --corridor meters of the route are included.
    #[arg(long)]
    route: Option<PathBuf>,

    /// Corridor width in meters (each side of route). Only used with --route.
    #[arg(long, default_value = "500")]
    corridor: f64,

    /// Output file path
    #[arg(long, default_value = "city.sykla")]
    output: PathBuf,

    /// Upload to GCS and register in database after generating
    #[arg(long, default_value_t = false)]
    upload: bool,

    /// GCS bucket name
    #[arg(long, env = "GCS_BUCKET", default_value = "sykla-city-data")]
    bucket: String,

    /// Database URL for city registration
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,
}

fn parse_bbox(s: &str) -> Result<(f64, f64, f64, f64), String> {
    let parts: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse::<f64>().map_err(|e| format!("Invalid bbox number: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 4 {
        return Err("Bbox must have exactly 4 values: south,west,north,east".into());
    }
    Ok((parts[0], parts[1], parts[2], parts[3]))
}

fn parse_center(s: &str) -> Result<(f64, f64), String> {
    let parts: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse::<f64>().map_err(|e| format!("Invalid center number: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 2 {
        return Err("Center must have exactly 2 values: lat,lng".into());
    }
    Ok((parts[0], parts[1]))
}

/// Quantize an f32 color channel to a u8 key for batching.
fn color_key(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    ]
}

/// Pre-batch meshes by (FeatureSubtype, color), returning one merged mesh per group.
///
/// With per-building color variation, walls of the same subtype may have different
/// colors. We batch by (subtype, quantized color) so each draw call has a single
/// material color. Detail elements (windows, cornices, storefronts) share one color
/// per subtype and batch down to 1 draw call each.
fn prebatch(meshes: Vec<CityMesh>) -> Vec<CityMesh> {
    let mut groups: HashMap<(FeatureSubtype, [u8; 4]), Vec<CityMesh>> = HashMap::new();
    for mesh in meshes {
        let key = (mesh.feature_subtype, color_key(mesh.color));
        groups.entry(key).or_default().push(mesh);
    }

    groups
        .into_values()
        .filter_map(|group| batch_meshes(&group))
        .collect()
}

fn main() {
    let cli = Cli::parse();

    // Determine bbox, center, and optional route points
    let (south, west, north, east, center_lat, center_lng, route_points) =
        if let Some(ref gpx_path) = cli.route {
            // Route-corridor mode
            let gpx_xml = std::fs::read_to_string(gpx_path).unwrap_or_else(|e| {
                eprintln!("Error reading GPX file: {e}");
                std::process::exit(1);
            });
            let route = sykla_core::gpx_parser::parse_gpx(&gpx_xml).unwrap_or_else(|e| {
                eprintln!("Error parsing GPX: {e}");
                std::process::exit(1);
            });

            let (s, w, n, e) = corridor::route_bbox(&route.points, cli.corridor);
            let (clat, clng) = corridor::route_center(&route.points);

            println!("Route: {} ({} points, {:.1} km)", route.name, route.points.len(), route.total_distance_m / 1000.0);
            println!("  Corridor: {:.0}m each side", cli.corridor);

            (s, w, n, e, clat, clng, Some(route.points))
        } else {
            // City-wide mode (original)
            let bbox_str = cli.bbox.as_deref().unwrap_or_else(|| {
                eprintln!("Error: --bbox is required when --route is not provided");
                std::process::exit(1);
            });
            let center_str = cli.center.as_deref().unwrap_or_else(|| {
                eprintln!("Error: --center is required when --route is not provided");
                std::process::exit(1);
            });

            let (s, w, n, e) = parse_bbox(bbox_str).unwrap_or_else(|e| {
                eprintln!("Error parsing bbox: {e}");
                std::process::exit(1);
            });
            let (clat, clng) = parse_center(center_str).unwrap_or_else(|e| {
                eprintln!("Error parsing center: {e}");
                std::process::exit(1);
            });

            (s, w, n, e, clat, clng, None)
        };

    println!("Generating city data for: {}", cli.name);
    println!("  Bbox: {south:.4},{west:.4},{north:.4},{east:.4}");
    println!("  Center: {center_lat:.4},{center_lng:.4}");
    println!();

    let proj = projection::CityProjection::new(center_lat, center_lng);

    // Fetch OSM data
    println!("Fetching buildings from Overpass API...");
    let buildings_json = overpass::fetch_buildings(south, west, north, east)
        .expect("Failed to fetch buildings");
    println!("Fetching land use...");
    let landuse_json = overpass::fetch_landuse(south, west, north, east)
        .expect("Failed to fetch land use");
    println!("Fetching water features...");
    let water_json = overpass::fetch_water(south, west, north, east)
        .expect("Failed to fetch water");
    println!("Fetching parks...");
    let parks_json = overpass::fetch_parks(south, west, north, east)
        .expect("Failed to fetch parks");
    println!("Fetching roads...");
    let roads_json = overpass::fetch_roads(south, west, north, east)
        .expect("Failed to fetch roads");
    println!("Fetching parking lots...");
    let parking_json = overpass::fetch_parking(south, west, north, east)
        .expect("Failed to fetch parking");

    // Parse OSM JSON
    println!("Parsing OSM data...");
    let mut building_features = parser::parse_buildings(&buildings_json);
    let mut landuse_features = parser::parse_landuse(&landuse_json);
    let mut water_features = parser::parse_water(&water_json);
    let mut park_features = parser::parse_parks(&parks_json);
    let mut road_features = parser::parse_roads(&roads_json);
    let mut parking_features = parser::parse_parking(&parking_json);

    let total_before = building_features.len() + landuse_features.len()
        + water_features.len() + park_features.len() + road_features.len()
        + parking_features.len();
    println!(
        "  {} buildings, {} land use, {} water, {} parks, {} roads, {} parking lots (total: {})",
        building_features.len(),
        landuse_features.len(),
        water_features.len(),
        park_features.len(),
        road_features.len(),
        parking_features.len(),
        total_before,
    );

    // Filter by corridor if in route mode
    if let Some(ref rp) = route_points {
        println!("Filtering to {:.0}m corridor...", cli.corridor);
        building_features = corridor::filter_buildings(building_features, rp, cli.corridor);
        landuse_features = corridor::filter_polygons(landuse_features, rp, cli.corridor);
        water_features = corridor::filter_polygons(water_features, rp, cli.corridor);
        park_features = corridor::filter_polygons(park_features, rp, cli.corridor);
        road_features = corridor::filter_roads(road_features, rp, cli.corridor);
        parking_features = corridor::filter_polygons(parking_features, rp, cli.corridor);

        let total_after = building_features.len() + landuse_features.len()
            + water_features.len() + park_features.len() + road_features.len()
            + parking_features.len();
        println!(
            "  After filter: {} buildings, {} land use, {} water, {} parks, {} roads, {} parking lots (total: {}, removed {})",
            building_features.len(),
            landuse_features.len(),
            water_features.len(),
            park_features.len(),
            road_features.len(),
            parking_features.len(),
            total_after,
            total_before - total_after,
        );
    }

    // Generate meshes
    println!("Generating building meshes...");
    let building_meshes = buildings::generate_building_meshes(&building_features, &proj);

    println!("Generating land use meshes...");
    let mut lu_meshes = landuse::generate_landuse_meshes(&landuse_features, &proj);
    let park_meshes = landuse::generate_park_meshes(&park_features, &proj);
    lu_meshes.extend(park_meshes);

    println!("Generating water meshes...");
    let water_meshes = water::generate_water_meshes(&water_features, &proj);

    println!("Generating road meshes...");
    let road_meshes = roads::generate_road_meshes(&road_features, &proj);

    println!("Generating parking lot meshes...");
    let parking_meshes = parking::generate_parking_meshes(&parking_features, &proj);
    lu_meshes.extend(parking_meshes);

    println!(
        "  {} building meshes, {} landuse meshes, {} water meshes, {} road meshes",
        building_meshes.len(),
        lu_meshes.len(),
        water_meshes.len(),
        road_meshes.len(),
    );

    // Build road network graph
    println!("Building road network graph...");
    let road_network = graph::build_road_network(&road_features, &proj);
    println!(
        "  Road network: {} nodes, {} edges",
        road_network.nodes.len(),
        road_network.edges.len(),
    );

    // Generate parked cars along streets (depends on road network)
    println!("Generating parked cars...");
    let car_meshes = parking::generate_parked_cars(&road_network);
    println!("  {} parked car mesh groups", car_meshes.len());
    let mut building_meshes = building_meshes;
    building_meshes.extend(car_meshes);

    // Pre-batch by subtype for minimal draw calls
    println!("Pre-batching by feature type...");
    let batched_buildings = prebatch(building_meshes);
    let batched_landuse = prebatch(lu_meshes);
    let batched_water = prebatch(water_meshes);
    let batched_roads = prebatch(road_meshes);

    println!(
        "  Batched: {} building groups, {} landuse groups, {} water groups, {} road groups ({} total draw calls)",
        batched_buildings.len(),
        batched_landuse.len(),
        batched_water.len(),
        batched_roads.len(),
        batched_buildings.len() + batched_landuse.len() + batched_water.len() + batched_roads.len(),
    );

    // Serialize
    let city_data = sykla_world::city::CityData {
        name: cli.name.clone(),
        center_lat,
        center_lng,
        buildings: batched_buildings,
        landuse: batched_landuse,
        water: batched_water,
        roads: batched_roads,
        road_network: Some(road_network),
    };

    println!("Writing to {}...", cli.output.display());
    format::write_city_data(&city_data, &cli.output).expect("Failed to write city data");

    let file_size = std::fs::metadata(&cli.output)
        .map(|m| m.len())
        .unwrap_or(0);

    if file_size > 1_048_576 {
        println!(
            "Done! {} ({:.1} MB)",
            cli.output.display(),
            file_size as f64 / 1_048_576.0
        );
    } else {
        println!(
            "Done! {} ({:.1} KB)",
            cli.output.display(),
            file_size as f64 / 1_024.0
        );
    }

    // Upload to GCS and register in DB if requested
    if cli.upload {
        let db_url = cli.database_url.as_deref().unwrap_or_else(|| {
            eprintln!("Error: --database-url or DATABASE_URL is required when --upload is set");
            std::process::exit(1);
        });

        let file_data = std::fs::read(&cli.output).unwrap_or_else(|e| {
            eprintln!("Error reading output file for upload: {e}");
            std::process::exit(1);
        });

        // Determine version by querying existing record
        let object_path = format!("cities/{}/v1.sykla", cli.name);
        let download_url = format!(
            "https://storage.googleapis.com/{}/{}",
            cli.bucket, object_path
        );

        println!("Uploading to GCS...");
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            upload::upload_to_gcs(&cli.bucket, &object_path, file_data)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("GCS upload failed: {e}");
                    std::process::exit(1);
                });

            println!("Registering city in database...");
            upload::register_city(
                db_url,
                &cli.name,
                &cli.name,
                center_lat,
                center_lng,
                (south, west, north, east),
                file_size as i64,
                &cli.bucket,
                &object_path,
                &download_url,
            )
            .await
            .unwrap_or_else(|e| {
                eprintln!("Database registration failed: {e}");
                std::process::exit(1);
            });
        });

        println!("Upload complete: {download_url}");
    }
}
