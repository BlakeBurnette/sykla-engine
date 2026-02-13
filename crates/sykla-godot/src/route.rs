use godot::prelude::*;
use sykla_core::gradient::{elevation_at_distance, grade_at_distance, surface_at_distance};
use sykla_core::gpx_parser::parse_gpx;
use sykla_core::types::Route;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct SyklaRoute {
    route: Option<Route>,
}

#[godot_api]
impl IRefCounted for SyklaRoute {
    fn init(_base: Base<RefCounted>) -> Self {
        Self { route: None }
    }
}

#[godot_api]
impl SyklaRoute {
    /// Parse a GPX XML string and load the route. Returns true on success.
    #[func]
    fn load_gpx(&mut self, xml: GString) -> bool {
        match parse_gpx(&xml.to_string()) {
            Ok(route) => {
                self.route = Some(route);
                true
            }
            Err(e) => {
                godot_error!("Failed to parse GPX: {e}");
                false
            }
        }
    }

    #[func]
    fn get_name(&self) -> GString {
        self.route
            .as_ref()
            .map(|r| GString::from(&r.name))
            .unwrap_or_default()
    }

    #[func]
    fn get_point_count(&self) -> i64 {
        self.route.as_ref().map(|r| r.points.len() as i64).unwrap_or(0)
    }

    #[func]
    fn get_total_distance_m(&self) -> f64 {
        self.route.as_ref().map(|r| r.total_distance_m).unwrap_or(0.0)
    }

    #[func]
    fn get_elevation_gain_m(&self) -> f64 {
        self.route.as_ref().map(|r| r.elevation_gain_m).unwrap_or(0.0)
    }

    #[func]
    fn get_min_elevation_m(&self) -> f64 {
        self.route.as_ref().map(|r| r.min_elevation_m).unwrap_or(0.0)
    }

    #[func]
    fn get_max_elevation_m(&self) -> f64 {
        self.route.as_ref().map(|r| r.max_elevation_m).unwrap_or(0.0)
    }

    /// Get a single route point as a Dictionary.
    /// Keys: lat, lng, elevation, distance, grade, surface
    #[func]
    fn get_point(&self, index: i64) -> VarDictionary {
        let mut dict = VarDictionary::new();
        if let Some(route) = &self.route {
            if let Some(point) = route.points.get(index as usize) {
                dict.set("lat", point.lat);
                dict.set("lng", point.lng);
                dict.set("elevation", point.elevation_m);
                dict.set("distance", point.distance_from_start_m);
                dict.set("grade", point.grade_percent.unwrap_or(0.0));
                dict.set("surface", point.surface.as_i32());
            }
        }
        dict
    }

    /// Get the interpolated grade at a given distance along the route.
    #[func]
    fn grade_at_distance(&self, distance_m: f64) -> f64 {
        self.route
            .as_ref()
            .map(|r| grade_at_distance(&r.points, distance_m))
            .unwrap_or(0.0)
    }

    /// Get the interpolated elevation at a given distance along the route.
    #[func]
    fn elevation_at_distance(&self, distance_m: f64) -> f64 {
        self.route
            .as_ref()
            .map(|r| elevation_at_distance(&r.points, distance_m))
            .unwrap_or(0.0)
    }

    /// Get the surface type (as integer) at a given distance along the route.
    #[func]
    fn surface_at_distance(&self, distance_m: f64) -> i32 {
        self.route
            .as_ref()
            .map(|r| surface_at_distance(&r.points, distance_m).as_i32())
            .unwrap_or(0)
    }

    /// Access the inner Route for use by other gdext classes in this crate.
    pub(crate) fn inner(&self) -> Option<&Route> {
        self.route.as_ref()
    }
}
