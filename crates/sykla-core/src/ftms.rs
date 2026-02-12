/// FTMS (Fitness Machine Service) protocol constants and types.
/// Bluetooth SIG specification for smart trainers.

/// FTMS Service UUID
pub const FTMS_SERVICE_UUID: u16 = 0x1826;

/// Indoor Bike Data characteristic UUID — notifications with speed/power/cadence
pub const INDOOR_BIKE_DATA_UUID: u16 = 0x2AD2;

/// Fitness Machine Control Point characteristic UUID — write commands here
pub const CONTROL_POINT_UUID: u16 = 0x2AD9;

/// Fitness Machine Status characteristic UUID — status notifications
pub const FITNESS_MACHINE_STATUS_UUID: u16 = 0x2ADA;

/// Control Point opcodes
pub const OP_REQUEST_CONTROL: u8 = 0x00;
pub const OP_RESET: u8 = 0x01;
pub const OP_SET_TARGET_RESISTANCE: u8 = 0x04;
pub const OP_SET_TARGET_POWER: u8 = 0x05;
pub const OP_START_RESUME: u8 = 0x07;
pub const OP_STOP_PAUSE: u8 = 0x08;
pub const OP_SET_SIMULATION_PARAMS: u8 = 0x11;

use serde::{Deserialize, Serialize};

/// FTMS Simulation Parameters — sent to trainer to control resistance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SimulationParams {
    /// Wind speed in m/s (positive = headwind). Resolution: 0.001 m/s.
    pub wind_speed_mps: f64,
    /// Grade in percent (-40% to +40%). Resolution: 0.01%.
    pub grade_percent: f64,
    /// Coefficient of rolling resistance. Typical: 0.004 (road), 0.008 (gravel).
    pub crr: f64,
    /// Wind resistance coefficient (kg/m). Typical: 0.51 (hoods), 0.39 (drops).
    pub cw: f64,
}

impl Default for SimulationParams {
    fn default() -> Self {
        Self {
            wind_speed_mps: 0.0,
            grade_percent: 0.0,
            crr: 0.004,
            cw: 0.51,
        }
    }
}

impl SimulationParams {
    /// Encode to FTMS Control Point byte payload (little-endian).
    /// Format: [opcode, wind_speed_lo, wind_speed_hi, grade_lo, grade_hi, crr, cw]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(7);
        buf.push(OP_SET_SIMULATION_PARAMS);

        // Wind speed: signed 16-bit, resolution 0.001 m/s
        let wind = (self.wind_speed_mps * 1000.0) as i16;
        buf.extend_from_slice(&wind.to_le_bytes());

        // Grade: signed 16-bit, resolution 0.01%
        let grade = (self.grade_percent * 100.0) as i16;
        buf.extend_from_slice(&grade.to_le_bytes());

        // CRR: unsigned 8-bit, resolution 0.0001
        let crr = (self.crr * 10000.0) as u8;
        buf.push(crr);

        // CW: unsigned 8-bit, resolution 0.01 kg/m
        let cw = (self.cw * 100.0) as u8;
        buf.push(cw);

        buf
    }

    /// Decode from FTMS Control Point byte payload.
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 7 || data[0] != OP_SET_SIMULATION_PARAMS {
            return None;
        }

        let wind = i16::from_le_bytes([data[1], data[2]]);
        let grade = i16::from_le_bytes([data[3], data[4]]);
        let crr = data[5];
        let cw = data[6];

        Some(Self {
            wind_speed_mps: wind as f64 / 1000.0,
            grade_percent: grade as f64 / 100.0,
            crr: crr as f64 / 10000.0,
            cw: cw as f64 / 100.0,
        })
    }
}

/// Data received from Indoor Bike Data characteristic notifications.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct IndoorBikeData {
    pub speed_kmh: f64,
    pub cadence_rpm: f64,
    pub power_watts: f64,
    pub heart_rate_bpm: Option<u8>,
}

impl IndoorBikeData {
    /// Decode Indoor Bike Data from FTMS notification payload.
    /// The format has a 16-bit flags field followed by optional data fields.
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }

        let flags = u16::from_le_bytes([data[0], data[1]]);
        let mut offset = 2;
        let mut result = Self::default();

        // Bit 0: More Data (0 = present below)
        // Bit 1: Average Speed Present
        // Bit 2: Instantaneous Cadence Present
        // Bit 3: Average Cadence Present
        // Bit 4: Total Distance Present
        // Bit 5: Resistance Level Present
        // Bit 6: Instantaneous Power Present
        // Bit 7: Average Power Present
        // etc.

        // Instantaneous Speed is present if bit 0 is NOT set
        if flags & 0x01 == 0 && offset + 2 <= data.len() {
            let speed_raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
            result.speed_kmh = speed_raw as f64 * 0.01; // Resolution: 0.01 km/h
            offset += 2;
        }

        // Average Speed (bit 1)
        if flags & 0x02 != 0 && offset + 2 <= data.len() {
            offset += 2; // skip
        }

        // Instantaneous Cadence (bit 2)
        if flags & 0x04 != 0 && offset + 2 <= data.len() {
            let cadence_raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
            result.cadence_rpm = cadence_raw as f64 * 0.5; // Resolution: 0.5 rpm
            offset += 2;
        }

        // Average Cadence (bit 3)
        if flags & 0x08 != 0 && offset + 2 <= data.len() {
            offset += 2; // skip
        }

        // Total Distance (bit 4) — 3 bytes
        if flags & 0x10 != 0 && offset + 3 <= data.len() {
            offset += 3; // skip
        }

        // Resistance Level (bit 5)
        if flags & 0x20 != 0 && offset + 2 <= data.len() {
            offset += 2; // skip
        }

        // Instantaneous Power (bit 6)
        if flags & 0x40 != 0 && offset + 2 <= data.len() {
            let power_raw = i16::from_le_bytes([data[offset], data[offset + 1]]);
            result.power_watts = power_raw as f64; // Resolution: 1 watt
            offset += 2;
        }

        // Average Power (bit 7)
        if flags & 0x80 != 0 && offset + 2 <= data.len() {
            offset += 2; // skip
        }

        // ... bits 8-12 (expended energy, heart rate, metabolic equiv, elapsed time, remaining time)

        // Heart Rate (bit 9)
        // Skip bits 8 (expended energy = 3 fields = 6 bytes)
        if flags & 0x100 != 0 && offset + 6 <= data.len() {
            offset += 6;
        }
        if flags & 0x200 != 0 && offset + 1 <= data.len() {
            result.heart_rate_bpm = Some(data[offset]);
            // offset += 1;
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_params_roundtrip() {
        let params = SimulationParams {
            wind_speed_mps: 2.5,
            grade_percent: 8.5,
            crr: 0.004,
            cw: 0.51,
        };

        let encoded = params.encode();
        assert_eq!(encoded.len(), 7);
        assert_eq!(encoded[0], OP_SET_SIMULATION_PARAMS);

        let decoded = SimulationParams::decode(&encoded).unwrap();
        assert!((decoded.wind_speed_mps - 2.5).abs() < 0.01);
        assert!((decoded.grade_percent - 8.5).abs() < 0.1);
        assert!((decoded.crr - 0.004).abs() < 0.001);
        assert!((decoded.cw - 0.51).abs() < 0.02);
    }

    #[test]
    fn test_simulation_params_negative_grade() {
        let params = SimulationParams {
            grade_percent: -12.0,
            ..Default::default()
        };

        let encoded = params.encode();
        let decoded = SimulationParams::decode(&encoded).unwrap();
        assert!((decoded.grade_percent - (-12.0)).abs() < 0.1);
    }

    #[test]
    fn test_indoor_bike_data_decode() {
        // flags: speed present (bit0=0), cadence present (bit2=1), power present (bit6=1)
        // flags = 0b0100_0100 = 0x44
        let mut data = vec![0x44, 0x00];
        // Speed: 25.00 km/h = 2500
        data.extend_from_slice(&2500u16.to_le_bytes());
        // Cadence: 80 rpm → 160 (resolution 0.5)
        data.extend_from_slice(&160u16.to_le_bytes());
        // Power: 200W
        data.extend_from_slice(&200i16.to_le_bytes());

        let bike_data = IndoorBikeData::decode(&data).unwrap();
        assert!((bike_data.speed_kmh - 25.0).abs() < 0.1);
        assert!((bike_data.cadence_rpm - 80.0).abs() < 0.5);
        assert!((bike_data.power_watts - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_default_simulation_params() {
        let params = SimulationParams::default();
        assert_eq!(params.wind_speed_mps, 0.0);
        assert_eq!(params.grade_percent, 0.0);
        assert!((params.crr - 0.004).abs() < 0.0001);
        assert!((params.cw - 0.51).abs() < 0.01);
    }
}
