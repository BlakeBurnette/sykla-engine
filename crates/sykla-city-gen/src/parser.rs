use serde_json::Value;
use sykla_world::city::FeatureSubtype;

/// A parsed OSM building feature.
#[derive(Debug)]
pub struct BuildingFeature {
    /// Outer ring of the building footprint as (lat, lng) pairs.
    pub ring: Vec<(f64, f64)>,
    /// Building height in meters (derived from tags).
    pub height: f32,
    /// Building subtype.
    pub subtype: FeatureSubtype,
}

/// A parsed OSM road feature (linestring, not polygon).
#[derive(Debug)]
pub struct RoadFeature {
    /// Centerline as (lat, lng) pairs.
    pub centerline: Vec<(f64, f64)>,
    /// Road subtype.
    pub subtype: FeatureSubtype,
    /// Street name from OSM `name` tag.
    pub name: Option<String>,
    /// Whether this road is part of a roundabout (junction=roundabout).
    pub is_roundabout: bool,
}

/// A parsed OSM polygon feature (land use or water).
#[derive(Debug)]
pub struct PolygonFeature {
    /// Outer ring as (lat, lng) pairs.
    pub ring: Vec<(f64, f64)>,
    /// Inner rings (holes) as (lat, lng) pairs.
    pub holes: Vec<Vec<(f64, f64)>>,
    /// Feature subtype.
    pub subtype: FeatureSubtype,
}

/// Extract geometry coordinates from an Overpass "out geom" way element.
fn extract_way_geometry(element: &Value) -> Option<Vec<(f64, f64)>> {
    let geometry = element.get("geometry")?.as_array()?;
    let coords: Vec<(f64, f64)> = geometry
        .iter()
        .filter_map(|node| {
            let lat = node.get("lat")?.as_f64()?;
            let lon = node.get("lon")?.as_f64()?;
            Some((lat, lon))
        })
        .collect();

    if coords.len() < 3 {
        return None;
    }
    Some(coords)
}

/// Extract the outer ring from a relation (multipolygon).
fn extract_relation_outer_ring(element: &Value) -> Option<Vec<(f64, f64)>> {
    let members = element.get("members")?.as_array()?;
    for member in members {
        let role = member.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role == "outer" {
            if let Some(geometry) = member.get("geometry").and_then(|g| g.as_array()) {
                let coords: Vec<(f64, f64)> = geometry
                    .iter()
                    .filter_map(|node| {
                        let lat = node.get("lat")?.as_f64()?;
                        let lon = node.get("lon")?.as_f64()?;
                        Some((lat, lon))
                    })
                    .collect();
                if coords.len() >= 3 {
                    return Some(coords);
                }
            }
        }
    }
    None
}

/// Extract inner rings (holes) from a relation.
fn extract_relation_inner_rings(element: &Value) -> Vec<Vec<(f64, f64)>> {
    let Some(members) = element.get("members").and_then(|m| m.as_array()) else {
        return vec![];
    };
    members
        .iter()
        .filter_map(|member| {
            let role = member.get("role").and_then(|r| r.as_str()).unwrap_or("");
            if role != "inner" {
                return None;
            }
            let geometry = member.get("geometry")?.as_array()?;
            let coords: Vec<(f64, f64)> = geometry
                .iter()
                .filter_map(|node| {
                    let lat = node.get("lat")?.as_f64()?;
                    let lon = node.get("lon")?.as_f64()?;
                    Some((lat, lon))
                })
                .collect();
            if coords.len() >= 3 { Some(coords) } else { None }
        })
        .collect()
}

/// Extract geometry coordinates from an Overpass way as a linestring (2+ points).
fn extract_way_linestring(element: &Value) -> Option<Vec<(f64, f64)>> {
    let geometry = element.get("geometry")?.as_array()?;
    let coords: Vec<(f64, f64)> = geometry
        .iter()
        .filter_map(|node| {
            let lat = node.get("lat")?.as_f64()?;
            let lon = node.get("lon")?.as_f64()?;
            Some((lat, lon))
        })
        .collect();

    if coords.len() < 2 {
        return None;
    }
    Some(coords)
}

/// Classify road type from OSM highway tag.
fn classify_road(element: &Value) -> FeatureSubtype {
    let highway_val = get_tag(element, "highway").unwrap_or_default();
    match highway_val.as_str() {
        "motorway" | "trunk" | "motorway_link" | "trunk_link" => FeatureSubtype::Highway,
        "primary" | "secondary" | "primary_link" | "secondary_link" => FeatureSubtype::MainStreet,
        "tertiary" | "residential" | "unclassified" | "living_street" | "tertiary_link" => {
            FeatureSubtype::Street
        }
        "footway" | "cycleway" | "path" | "pedestrian" => FeatureSubtype::Path,
        _ => FeatureSubtype::RoadOther,
    }
}

fn get_tag(element: &Value, key: &str) -> Option<String> {
    element.get("tags")?.get(key)?.as_str().map(|s| s.to_string())
}

/// Determine building height from OSM tags.
fn determine_building_height(element: &Value, subtype: FeatureSubtype) -> f32 {
    // Explicit height tag
    if let Some(h) = get_tag(element, "height") {
        if let Ok(meters) = h.trim_end_matches(" m").trim_end_matches('m').trim().parse::<f32>() {
            return meters;
        }
    }

    // building:levels tag
    if let Some(levels_str) = get_tag(element, "building:levels") {
        if let Ok(levels) = levels_str.parse::<f32>() {
            return levels * 3.5;
        }
    }

    // Default by subtype
    subtype.default_height()
}

/// Classify building type from OSM tags.
fn classify_building(element: &Value) -> FeatureSubtype {
    let building_val = get_tag(element, "building").unwrap_or_default();
    match building_val.as_str() {
        "residential" | "house" | "apartments" | "detached" | "semidetached_house"
        | "terrace" | "dormitory" => FeatureSubtype::Residential,
        "commercial" | "office" => FeatureSubtype::Commercial,
        "industrial" | "warehouse" | "manufacture" => FeatureSubtype::Industrial,
        "retail" | "supermarket" | "kiosk" => FeatureSubtype::Retail,
        "school" | "university" | "college" | "public" | "civic" | "government"
        | "hospital" | "church" | "chapel" | "mosque" | "synagogue" | "temple"
        | "cathedral" | "library" | "fire_station" | "police" => FeatureSubtype::Public,
        _ => FeatureSubtype::BuildingOther,
    }
}

/// Classify land use type from OSM tags.
fn classify_landuse(element: &Value) -> FeatureSubtype {
    let lu_val = get_tag(element, "landuse").unwrap_or_default();
    match lu_val.as_str() {
        "grass" | "recreation_ground" | "village_green" => FeatureSubtype::Park,
        "forest" | "wood" => FeatureSubtype::Forest,
        "farmland" | "farm" | "orchard" | "vineyard" => FeatureSubtype::Farmland,
        "meadow" => FeatureSubtype::Meadow,
        _ => FeatureSubtype::LanduseOther,
    }
}

/// Classify water type from OSM tags.
fn classify_water(element: &Value) -> FeatureSubtype {
    if get_tag(element, "waterway").is_some() {
        return FeatureSubtype::Stream;
    }
    let water_val = get_tag(element, "water").unwrap_or_default();
    match water_val.as_str() {
        "river" | "stream" | "canal" | "ditch" | "drain" => FeatureSubtype::Stream,
        _ => FeatureSubtype::Lake,
    }
}

/// Parse Overpass JSON response for buildings.
pub fn parse_buildings(json: &Value) -> Vec<BuildingFeature> {
    let Some(elements) = json.get("elements").and_then(|e| e.as_array()) else {
        return vec![];
    };

    elements
        .iter()
        .filter_map(|element| {
            let element_type = element.get("type")?.as_str()?;
            let ring = match element_type {
                "way" => extract_way_geometry(element)?,
                "relation" => extract_relation_outer_ring(element)?,
                _ => return None,
            };

            let subtype = classify_building(element);
            let height = determine_building_height(element, subtype);

            Some(BuildingFeature {
                ring,
                height,
                subtype,
            })
        })
        .collect()
}

/// Parse Overpass JSON response for land use polygons.
pub fn parse_landuse(json: &Value) -> Vec<PolygonFeature> {
    parse_polygon_features(json, |element| classify_landuse(element))
}

/// Parse Overpass JSON response for water features.
pub fn parse_water(json: &Value) -> Vec<PolygonFeature> {
    parse_polygon_features(json, |element| classify_water(element))
}

/// Parse Overpass JSON response for parks.
pub fn parse_parks(json: &Value) -> Vec<PolygonFeature> {
    parse_polygon_features(json, |_| FeatureSubtype::Park)
}

/// Parse Overpass JSON response for parking lots.
pub fn parse_parking(json: &Value) -> Vec<PolygonFeature> {
    parse_polygon_features(json, |_| FeatureSubtype::ParkingLot)
}

/// Parse Overpass JSON response for roads (linestrings).
pub fn parse_roads(json: &Value) -> Vec<RoadFeature> {
    let Some(elements) = json.get("elements").and_then(|e| e.as_array()) else {
        return vec![];
    };

    elements
        .iter()
        .filter_map(|element| {
            let element_type = element.get("type")?.as_str()?;
            if element_type != "way" {
                return None;
            }
            let centerline = extract_way_linestring(element)?;
            let subtype = classify_road(element);
            let name = get_tag(element, "name");
            let is_roundabout = get_tag(element, "junction").as_deref() == Some("roundabout");
            Some(RoadFeature {
                centerline,
                subtype,
                name,
                is_roundabout,
            })
        })
        .collect()
}

fn parse_polygon_features(
    json: &Value,
    classify: impl Fn(&Value) -> FeatureSubtype,
) -> Vec<PolygonFeature> {
    let Some(elements) = json.get("elements").and_then(|e| e.as_array()) else {
        return vec![];
    };

    elements
        .iter()
        .filter_map(|element| {
            let element_type = element.get("type")?.as_str()?;
            let (ring, holes) = match element_type {
                "way" => (extract_way_geometry(element)?, vec![]),
                "relation" => {
                    let outer = extract_relation_outer_ring(element)?;
                    let inner = extract_relation_inner_rings(element);
                    (outer, inner)
                }
                _ => return None,
            };

            let subtype = classify(element);

            Some(PolygonFeature {
                ring,
                holes,
                subtype,
            })
        })
        .collect()
}
