use bevy::prelude::*;
use sykla_core::ride_engine::RideEngine;
use sykla_core::types::Route;

use crate::ble::{TrainerCommand, TrainerState};

/// Resource wrapping the core ride engine.
#[derive(Resource)]
pub struct ActiveRide {
    pub engine: RideEngine,
}

/// Event to start a new ride with a loaded route.
#[derive(Event)]
pub struct StartRideEvent {
    pub route: Route,
}

/// Event fired when a ride completes.
#[derive(Event)]
pub struct RideCompleteEvent;

/// Bevy plugin for the ride simulation loop.
pub struct RidePlugin;

impl Plugin for RidePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<StartRideEvent>()
            .add_event::<RideCompleteEvent>()
            .add_systems(Update, (handle_start_ride, ride_update_system.after(handle_start_ride)));
    }
}

/// Handle StartRideEvent — initialize the ride engine.
fn handle_start_ride(
    mut commands: Commands,
    mut events: EventReader<StartRideEvent>,
) {
    for event in events.read() {
        let engine = RideEngine::new(event.route.clone());
        commands.insert_resource(ActiveRide { engine });
    }
}

/// Main ride simulation update — runs every frame.
fn ride_update_system(
    time: Res<Time>,
    trainer: Res<TrainerState>,
    mut trainer_cmd: ResMut<TrainerCommand>,
    mut ride: Option<ResMut<ActiveRide>>,
    mut complete_events: EventWriter<RideCompleteEvent>,
) {
    let Some(ref mut ride) = ride else { return };

    let dt = time.delta_secs_f64();
    if dt <= 0.0 || dt > 1.0 {
        return; // skip invalid deltas
    }

    let grade = ride.engine.update(
        dt,
        trainer.speed_kmh,
        trainer.power_watts,
        trainer.cadence_rpm,
    );

    // Update trainer simulation parameters with new grade
    trainer_cmd.params.grade_percent = grade;
    trainer_cmd.dirty = true;

    if ride.engine.state().is_complete {
        complete_events.send(RideCompleteEvent);
    }
}
