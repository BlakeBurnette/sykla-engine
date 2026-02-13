use sykla_core::ride_engine::RideEngine;
use sykla_core::types::Route;

use crate::ble::{TrainerCommand, TrainerState};

/// Active ride state — plain struct, no Bevy.
pub struct ActiveRide {
    pub engine: RideEngine,
    pub command: TrainerCommand,
}

impl ActiveRide {
    pub fn new(route: Route) -> Self {
        Self {
            engine: RideEngine::new(route),
            command: TrainerCommand::default(),
        }
    }

    /// Update ride simulation. Returns current grade.
    pub fn update(&mut self, dt: f64, trainer: &TrainerState) -> f64 {
        let grade = self.engine.update(
            dt,
            trainer.speed_kmh,
            trainer.power_watts,
            trainer.cadence_rpm,
        );

        self.command.params.grade_percent = grade;
        self.command.dirty = true;

        grade
    }

    /// Flush pending trainer commands.
    pub fn flush_commands(&mut self) {
        self.command.flush();
    }
}
