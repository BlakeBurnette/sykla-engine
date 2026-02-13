use godot::prelude::*;
use sykla_core::ride_engine::RideEngine;

use crate::route::SyklaRoute;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct SyklaRideEngine {
    engine: Option<RideEngine>,
}

#[godot_api]
impl IRefCounted for SyklaRideEngine {
    fn init(_base: Base<RefCounted>) -> Self {
        Self { engine: None }
    }
}

#[godot_api]
impl SyklaRideEngine {
    /// Start a ride on the given route. Returns true on success.
    #[func]
    fn start(&mut self, route: Gd<SyklaRoute>) -> bool {
        let route_ref = route.bind();
        match route_ref.inner() {
            Some(r) => {
                self.engine = Some(RideEngine::new(r.clone()));
                true
            }
            None => {
                godot_error!("SyklaRideEngine::start: route has no data loaded");
                false
            }
        }
    }

    /// Advance the ride simulation. Returns the current grade (%) the trainer should target.
    #[func]
    fn update(&mut self, dt: f64, speed_kmh: f64, power_watts: f64, cadence_rpm: f64) -> f64 {
        match &mut self.engine {
            Some(engine) => engine.update(dt, speed_kmh, power_watts, cadence_rpm),
            None => 0.0,
        }
    }

    /// Get the full ride state as a Dictionary.
    #[func]
    fn get_state(&self) -> VarDictionary {
        let mut dict = VarDictionary::new();
        if let Some(engine) = &self.engine {
            let s = engine.state();
            dict.set("distance_m", s.distance_m);
            dict.set("elapsed_secs", s.elapsed_secs);
            dict.set("grade_percent", s.current_grade_percent);
            dict.set("elevation_m", s.current_elevation_m);
            dict.set("speed_kmh", s.current_speed_kmh);
            dict.set("power_watts", s.current_power_watts);
            dict.set("cadence_rpm", s.current_cadence_rpm);
            dict.set("is_complete", s.is_complete);
        }
        dict
    }

    #[func]
    fn get_distance_m(&self) -> f64 {
        self.engine.as_ref().map(|e| e.state().distance_m).unwrap_or(0.0)
    }

    #[func]
    fn get_elevation_m(&self) -> f64 {
        self.engine
            .as_ref()
            .map(|e| e.state().current_elevation_m)
            .unwrap_or(0.0)
    }

    #[func]
    fn get_speed_kmh(&self) -> f64 {
        self.engine
            .as_ref()
            .map(|e| e.state().current_speed_kmh)
            .unwrap_or(0.0)
    }

    #[func]
    fn get_grade_percent(&self) -> f64 {
        self.engine
            .as_ref()
            .map(|e| e.state().current_grade_percent)
            .unwrap_or(0.0)
    }

    #[func]
    fn get_progress(&self) -> f64 {
        self.engine.as_ref().map(|e| e.progress()).unwrap_or(0.0)
    }

    #[func]
    fn is_complete(&self) -> bool {
        self.engine
            .as_ref()
            .map(|e| e.state().is_complete)
            .unwrap_or(false)
    }

    /// Get current lat/lng as a Vector2 (x=lat, y=lng).
    #[func]
    fn get_position(&self) -> Vector2 {
        self.engine
            .as_ref()
            .and_then(|e| e.current_position())
            .map(|(lat, lng)| Vector2::new(lat as f32, lng as f32))
            .unwrap_or(Vector2::ZERO)
    }
}
