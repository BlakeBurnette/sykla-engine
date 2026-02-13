use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response};

use sykla_world::city::CityData;

// Thread-local storage for passing data from async fetch to main loop.
thread_local! {
    static LOADED_CITY: RefCell<Option<Result<CityData, String>>> = RefCell::new(None);
}

/// Take the loaded city result from the thread-local, if available.
pub fn take_loaded_city() -> Option<Result<CityData, String>> {
    LOADED_CITY.with(|cell| cell.borrow_mut().take())
}

/// City load state — plain struct, no Bevy.
pub struct CityLoadState {
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
}

impl CityLoadState {
    pub fn new() -> Self {
        Self {
            loading: false,
            loaded: false,
            error: None,
        }
    }

    /// Kick off async city data fetch.
    pub fn trigger_load(&mut self) {
        if self.loading || self.loaded {
            return;
        }
        self.loading = true;

        wasm_bindgen_futures::spawn_local(async {
            let result = fetch_city_from_api().await;
            LOADED_CITY.with(|cell| {
                *cell.borrow_mut() = Some(result);
            });
        });
    }
}

/// Read the `?city=` query parameter, defaulting to "cary-nc".
fn get_city_slug() -> String {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return "cary-nc".to_string(),
    };
    let search = match window.location().search() {
        Ok(s) => s,
        Err(_) => return "cary-nc".to_string(),
    };
    let params = match web_sys::UrlSearchParams::new_with_str(&search) {
        Ok(p) => p,
        Err(_) => return "cary-nc".to_string(),
    };
    params.get("city").unwrap_or_else(|| "cary-nc".to_string())
}

/// Get the API base URL from the current origin.
fn get_api_base_url() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "http://localhost:3030".to_string())
}

/// Fetch city metadata from server API, then download the binary data from GCS.
async fn fetch_city_from_api() -> Result<CityData, String> {
    let slug = get_city_slug();
    let base = get_api_base_url();
    let api_url = format!("{base}/api/cities/{slug}");

    web_sys::console::log_1(&format!("Fetching city metadata: {api_url}").into());

    let json = fetch_json(&api_url).await?;

    let download_url = js_sys::Reflect::get(&json, &"download_url".into())
        .map_err(|_| "Missing download_url in response".to_string())?
        .as_string()
        .ok_or("download_url is not a string")?;

    web_sys::console::log_1(&format!("Downloading city data: {download_url}").into());

    let bytes = fetch_bytes(&download_url).await?;

    bincode::deserialize::<CityData>(&bytes).map_err(|e| format!("Deserialize error: {e}"))
}

/// Fetch a URL and parse the response as JSON.
async fn fetch_json(url: &str) -> Result<JsValue, String> {
    let opts = RequestInit::new();
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
        return Err(format!("HTTP {} fetching {url}", resp.status()));
    }

    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON error: {e:?}"))?)
        .await
        .map_err(|e| format!("JSON parse error: {e:?}"))?;

    Ok(json)
}

/// Fetch raw bytes from a URL using the browser Fetch API.
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let opts = RequestInit::new();
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
