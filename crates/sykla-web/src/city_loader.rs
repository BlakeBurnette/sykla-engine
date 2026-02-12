use bevy::prelude::*;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response};

use sykla_world::city::CityData;

use crate::world_render::GenerateWorldEvent;

// Thread-local storage for passing data from async fetch to Bevy systems.
// Safe because WASM is single-threaded.
thread_local! {
    static LOADED_CITY: RefCell<Option<Result<Vec<u8>, String>>> = RefCell::new(None);
}

/// Resource tracking city load state.
#[derive(Resource, Default)]
pub struct CityLoadState {
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
}

/// Plugin that loads city data on startup.
pub struct CityLoaderPlugin;

impl Plugin for CityLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CityLoadState>()
            .add_systems(Startup, trigger_city_load)
            .add_systems(Update, check_city_loaded);
    }
}

/// On startup, kick off an async fetch of the city data file.
fn trigger_city_load(mut state: ResMut<CityLoadState>) {
    if state.loading || state.loaded {
        return;
    }
    state.loading = true;

    // For local dev: place the .sykla file in the web/ directory as "city.sykla".
    // For production: serve from CDN.
    wasm_bindgen_futures::spawn_local(async {
        let result = fetch_bytes("city.sykla").await;
        LOADED_CITY.with(|cell| {
            *cell.borrow_mut() = Some(result);
        });
    });
}

/// Bevy system that checks if async city load is done, and if so, fires GenerateWorldEvent.
fn check_city_loaded(
    mut state: ResMut<CityLoadState>,
    mut events: EventWriter<GenerateWorldEvent>,
) {
    if state.loaded || !state.loading {
        return;
    }

    let result = LOADED_CITY.with(|cell| cell.borrow_mut().take());

    let Some(result) = result else {
        return; // still loading
    };

    match result {
        Ok(bytes) => match bincode::deserialize::<CityData>(&bytes) {
            Ok(city_data) => {
                web_sys::console::log_1(
                    &format!(
                        "City loaded: {} ({} buildings, {} landuse, {} water)",
                        city_data.name,
                        city_data.buildings.len(),
                        city_data.landuse.len(),
                        city_data.water.len(),
                    )
                    .into(),
                );

                // Dummy route — free-roam mode doesn't need a real route
                let dummy_route = sykla_core::types::Route {
                    name: city_data.name.clone(),
                    points: vec![sykla_core::types::RoutePoint {
                        lat: city_data.center_lat,
                        lng: city_data.center_lng,
                        elevation_m: 100.0,
                        distance_from_start_m: 0.0,
                        grade_percent: Some(0.0),
                    }],
                    total_distance_m: 0.0,
                    elevation_gain_m: 0.0,
                    min_elevation_m: 100.0,
                    max_elevation_m: 100.0,
                };

                events.send(GenerateWorldEvent {
                    route: dummy_route,
                    city_data: Some(city_data),
                });

                state.loaded = true;
                state.loading = false;
            }
            Err(e) => {
                let msg = format!("Deserialize error: {e}");
                web_sys::console::error_1(&msg.clone().into());
                state.error = Some(msg);
                state.loading = false;
            }
        },
        Err(e) => {
            web_sys::console::error_1(&format!("Failed to load city: {e}").into());
            state.error = Some(e);
            state.loading = false;
        }
    }
}

/// Fetch raw bytes from a URL using the browser Fetch API.
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let mut opts = RequestInit::new();
    opts.set_method("GET");

    let request =
        Request::new_with_str_and_init(url, &opts).map_err(|e| format!("Request error: {e:?}"))?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {e:?}"))?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast failed".to_string())?;

    if !resp.ok() {
        return Err(format!("HTTP {} loading city data", resp.status()));
    }

    let array_buffer = JsFuture::from(
        resp.array_buffer()
            .map_err(|e| format!("ArrayBuffer error: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("ArrayBuffer await error: {e:?}"))?;

    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    Ok(uint8_array.to_vec())
}
