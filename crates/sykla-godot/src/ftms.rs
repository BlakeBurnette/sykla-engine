use godot::prelude::*;
use sykla_core::ftms::{IndoorBikeData, SimulationParams};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct SyklaFtms;

#[godot_api]
impl IRefCounted for SyklaFtms {
    fn init(_base: Base<RefCounted>) -> Self {
        Self
    }
}

#[godot_api]
impl SyklaFtms {
    /// Encode FTMS simulation parameters to a byte array for BLE transmission.
    #[func]
    fn encode_simulation_params(
        wind_speed_mps: f64,
        grade_percent: f64,
        crr: f64,
        cw: f64,
    ) -> PackedByteArray {
        let params = SimulationParams {
            wind_speed_mps,
            grade_percent,
            crr,
            cw,
        };
        let bytes = params.encode();
        PackedByteArray::from(bytes.as_slice())
    }

    /// Decode an Indoor Bike Data BLE notification payload into a Dictionary.
    /// Keys: speed_kmh, cadence_rpm, power_watts, heart_rate_bpm (or -1 if absent).
    #[func]
    fn decode_indoor_bike_data(data: PackedByteArray) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let bytes: Vec<u8> = data.to_vec();
        match IndoorBikeData::decode(&bytes) {
            Some(bike) => {
                dict.set("speed_kmh", bike.speed_kmh);
                dict.set("cadence_rpm", bike.cadence_rpm);
                dict.set("power_watts", bike.power_watts);
                dict.set(
                    "heart_rate_bpm",
                    bike.heart_rate_bpm.map(|v| v as i32).unwrap_or(-1),
                );
            }
            None => {
                godot_error!("SyklaFtms: failed to decode Indoor Bike Data");
            }
        }
        dict
    }
}
