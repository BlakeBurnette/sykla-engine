use reqwest::blocking::Client;
use std::thread;
use std::time::Duration;

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";

/// Rate-limit delay between Overpass queries (be a good citizen).
const QUERY_DELAY: Duration = Duration::from_secs(5);

/// Max retry attempts on 429/504 responses.
const MAX_RETRIES: u32 = 3;

fn query_overpass(query: &str) -> Result<serde_json::Value, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let body = format!("data={}", urlencoded(query));

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let backoff = Duration::from_secs(10 * attempt as u64);
            eprintln!("  Retry {attempt}/{MAX_RETRIES} after {backoff:?}...");
            thread::sleep(backoff);
        }

        let resp = client
            .post(OVERPASS_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.clone())
            .send()
            .map_err(|e| format!("Overpass request failed: {e}"))?;

        let status = resp.status();
        if status.as_u16() == 429 || status.as_u16() == 504 {
            if attempt < MAX_RETRIES {
                eprintln!("  Overpass returned {status} (server busy)");
                continue;
            }
        }

        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("Overpass HTTP {status}: {body}"));
        }

        let json: serde_json::Value = resp.json().map_err(|e| format!("JSON parse error: {e}"))?;
        thread::sleep(QUERY_DELAY);
        return Ok(json);
    }

    Err("Overpass API: max retries exceeded".into())
}

/// URL-encode a string (minimal: just spaces, brackets, quotes, newlines).
fn urlencoded(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('[', "%5B")
        .replace(']', "%5D")
        .replace('"', "%22")
        .replace('\n', "%0A")
        .replace('(', "%28")
        .replace(')', "%29")
        .replace(';', "%3B")
        .replace('|', "%7C")
        .replace('~', "%7E")
}

fn bbox_str(south: f64, west: f64, north: f64, east: f64) -> String {
    format!("{south},{west},{north},{east}")
}

/// Fetch all buildings within the bounding box.
pub fn fetch_buildings(south: f64, west: f64, north: f64, east: f64) -> Result<serde_json::Value, String> {
    let bbox = bbox_str(south, west, north, east);
    let query = format!(
        r#"[out:json][timeout:90];
(
  way["building"]({bbox});
  relation["building"]({bbox});
);
out geom;"#
    );
    query_overpass(&query)
}

/// Fetch land use areas within the bounding box.
pub fn fetch_landuse(south: f64, west: f64, north: f64, east: f64) -> Result<serde_json::Value, String> {
    let bbox = bbox_str(south, west, north, east);
    let query = format!(
        r#"[out:json][timeout:90];
(
  way["landuse"]({bbox});
  relation["landuse"]({bbox});
);
out geom;"#
    );
    query_overpass(&query)
}

/// Fetch water features within the bounding box.
pub fn fetch_water(south: f64, west: f64, north: f64, east: f64) -> Result<serde_json::Value, String> {
    let bbox = bbox_str(south, west, north, east);
    let query = format!(
        r#"[out:json][timeout:90];
(
  way["natural"="water"]({bbox});
  way["waterway"]({bbox});
  relation["natural"="water"]({bbox});
);
out geom;"#
    );
    query_overpass(&query)
}

/// Fetch roads and highways within the bounding box.
pub fn fetch_roads(south: f64, west: f64, north: f64, east: f64) -> Result<serde_json::Value, String> {
    let bbox = bbox_str(south, west, north, east);
    let query = format!(
        r#"[out:json][timeout:90];
(
  way["highway"~"motorway|trunk|primary|secondary|tertiary|residential|service|unclassified|living_street|pedestrian|footway|cycleway|path"]({bbox});
);
out geom;"#
    );
    query_overpass(&query)
}

/// Fetch parking lots within the bounding box.
pub fn fetch_parking(south: f64, west: f64, north: f64, east: f64) -> Result<serde_json::Value, String> {
    let bbox = bbox_str(south, west, north, east);
    let query = format!(
        r#"[out:json][timeout:90];
(
  way["amenity"="parking"]({bbox});
  relation["amenity"="parking"]({bbox});
);
out geom;"#
    );
    query_overpass(&query)
}

/// Fetch parks and gardens within the bounding box.
pub fn fetch_parks(south: f64, west: f64, north: f64, east: f64) -> Result<serde_json::Value, String> {
    let bbox = bbox_str(south, west, north, east);
    let query = format!(
        r#"[out:json][timeout:90];
(
  way["leisure"~"park|garden|playground"]({bbox});
  relation["leisure"~"park|garden|playground"]({bbox});
);
out geom;"#
    );
    query_overpass(&query)
}
