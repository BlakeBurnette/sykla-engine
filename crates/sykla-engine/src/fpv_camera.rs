use glam::{Quat, Vec3};

use crate::physics::PhysicsState;
use crate::terrain::{interpolate_point, surface_at, RoutePoint, SurfaceType};
use crate::turnaround::RouteDirection;

/// User-configurable FPV camera settings.
pub struct FpvSettings {
    /// Overall motion intensity (0.0 = static, 1.0 = full). Default 0.6.
    pub intensity: f32,
    pub head_bob_enabled: bool,
    pub road_vibration_enabled: bool,
    pub corner_lookahead_enabled: bool,
    pub descent_fov_boost_enabled: bool,
    /// Base field-of-view in degrees. Default 85.
    pub base_fov_degrees: f32,
}

impl Default for FpvSettings {
    fn default() -> Self {
        Self {
            intensity: 0.6,
            head_bob_enabled: true,
            road_vibration_enabled: true,
            corner_lookahead_enabled: true,
            descent_fov_boost_enabled: true,
            base_fov_degrees: 85.0,
        }
    }
}

/// First-person cycling camera with 5 motion layers.
pub struct CyclingCamera {
    smoothed_position: Vec3,
    smoothed_rotation: Quat,
    sway_time: f32,
    fatigue_accumulator: f32,
    smoothed_gradient_pitch: f32,
    smoothed_lookahead_yaw: f32,
    smoothed_corner_lean: f32,
    standing_blend: f32,
    cadence_fade: f32,
    pub settings: FpvSettings,
}

impl CyclingCamera {
    pub fn new() -> Self {
        Self {
            smoothed_position: Vec3::ZERO,
            smoothed_rotation: Quat::IDENTITY,
            sway_time: 0.0,
            fatigue_accumulator: 0.0,
            smoothed_gradient_pitch: 0.0,
            smoothed_lookahead_yaw: 0.0,
            smoothed_corner_lean: 0.0,
            standing_blend: 0.0,
            cadence_fade: 0.0,
            settings: FpvSettings::default(),
        }
    }

    /// Snap camera state when entering FPV mode (no smoothing on first frame).
    pub fn reset(&mut self, initial_position: Vec3) {
        self.smoothed_position = initial_position;
        self.smoothed_rotation = Quat::IDENTITY;
        self.sway_time = 0.0;
        self.fatigue_accumulator = 0.0;
        self.smoothed_gradient_pitch = 0.0;
        self.smoothed_lookahead_yaw = 0.0;
        self.smoothed_corner_lean = 0.0;
        self.standing_blend = 0.0;
        self.cadence_fade = 0.0;
    }

    /// Update the FPV camera and return (eye, target, fov_radians).
    pub fn update(
        &mut self,
        physics: &PhysicsState,
        route_points: &[RoutePoint],
        route_length: f32,
        dt: f32,
        direction: RouteDirection,
    ) -> (Vec3, Vec3, f32) {
        if dt <= 0.0 || route_points.is_empty() {
            return (self.smoothed_position, self.smoothed_position + Vec3::Z, self.settings.base_fov_degrees.to_radians());
        }

        let intensity = self.settings.intensity;

        // --- Road position interpolation ---
        let dist = physics.distance as f64;
        let (px, pz, elev, raw_fx, raw_fz) = interpolate_point(route_points, dist);

        // Direction-aware forward
        let (fx, fz) = match direction {
            RouteDirection::Forward => (raw_fx, raw_fz),
            RouteDirection::Reverse => (-raw_fx, -raw_fz),
        };

        let forward = Vec3::new(fx, 0.0, fz).normalize_or_zero();
        let right = Vec3::new(-fz, 0.0, fx).normalize_or_zero();

        // --- Standing detection & blend ---
        let is_standing = detect_standing(physics);
        let stand_target = if is_standing { 1.0 } else { 0.0 };
        let stand_rate = 1.0 / 0.5; // 0.5s transition
        self.standing_blend = exp_toward(self.standing_blend, stand_target, stand_rate, dt);

        // --- Base head position ---
        let seated_height = 1.20_f32;
        let standing_height = 1.35_f32;
        let head_height = lerp(seated_height, standing_height, self.standing_blend);
        let forward_offset = lerp(0.08, 0.18, self.standing_blend);

        let gradient = physics.gradient;

        // Gradient position shift: lean forward on climbs, lower on descents
        let gradient_forward_shift = if gradient > 0.05 {
            (gradient - 0.05) * 0.5 * intensity
        } else {
            0.0
        };
        let gradient_height_shift = if gradient > 0.05 {
            -(gradient - 0.05) * 0.3 * intensity
        } else if gradient < -0.02 {
            (gradient + 0.02) * 0.15 * intensity // slightly lower on descent
        } else {
            0.0
        };

        let base_pos = Vec3::new(px, elev + head_height + gradient_height_shift, pz)
            + forward * (forward_offset + gradient_forward_shift);

        // --- Layer 1: Cadence Bob ---
        let cadence_offset = if self.settings.head_bob_enabled {
            self.compute_cadence_bob(physics, intensity, dt)
        } else {
            Vec3::ZERO
        };

        // --- Layer 2: Effort Sway ---
        let (sway_offset, fatigue_pitch) = self.compute_effort_sway(physics, intensity, dt);

        // --- Layer 3: Road Vibration ---
        let vibration_offset = if self.settings.road_vibration_enabled {
            self.compute_road_vibration(physics, route_points, intensity)
        } else {
            Vec3::ZERO
        };

        // --- Sum position offsets (in local space, then transform to world) ---
        let local_offset = cadence_offset + sway_offset + vibration_offset;
        let world_offset = right * local_offset.x + Vec3::Y * local_offset.y + forward * local_offset.z;
        let raw_position = base_pos + world_offset;

        // --- Rotation layers ---
        // Layer 1 roll: standing bob roll
        let bob_roll = if self.settings.head_bob_enabled && self.standing_blend > 0.01 {
            let power_factor = (physics.power_watts / 400.0).clamp(0.3, 1.0);
            let roll_amp = 2.5_f32.to_radians() * self.standing_blend * power_factor * intensity;
            roll_amp * physics.crank_angle.sin() * self.cadence_fade
        } else {
            0.0
        };

        // Layer 4: Gradient pitch
        let gradient_pitch_target = gradient * 0.5;
        self.smoothed_gradient_pitch = exp_toward(self.smoothed_gradient_pitch, gradient_pitch_target, 2.0, dt);
        let gradient_pitch = self.smoothed_gradient_pitch * intensity;

        // Descent gaze adjustment
        let descent_gaze = if gradient < -0.02 {
            (gradient + 0.02).abs() * 0.3 * intensity // look slightly up on descent
        } else {
            0.0
        };

        // Base downward gaze
        let gaze_pitch = -4.0_f32.to_radians() * intensity;

        // Layer 5: Corner look-ahead
        let (corner_yaw, corner_lean) = if self.settings.corner_lookahead_enabled {
            self.compute_corner_lookahead(physics, route_points, route_length, intensity, dt, direction)
        } else {
            (0.0, 0.0)
        };

        // --- Build rotation from road-forward basis + offsets ---
        let total_pitch = gradient_pitch + fatigue_pitch + gaze_pitch + descent_gaze;
        let total_yaw = corner_yaw;
        let total_roll = bob_roll + corner_lean;

        // Road-forward basis quaternion (look along forward direction)
        let look_target = Vec3::new(px + fx * 10.0, elev + head_height, pz + fz * 10.0);
        let base_rotation = look_rotation(raw_position, look_target);

        // Apply pitch/yaw/roll offsets
        let offset_rotation = Quat::from_euler(glam::EulerRot::YXZ, total_yaw, total_pitch, total_roll);
        let raw_rotation = base_rotation * offset_rotation;

        // --- EMA smoothing ---
        let pos_alpha = ema_alpha(20.0, dt);
        self.smoothed_position = self.smoothed_position.lerp(raw_position, pos_alpha);

        let rot_alpha = ema_alpha(15.0, dt);
        self.smoothed_rotation = self.smoothed_rotation.slerp(raw_rotation, rot_alpha);
        self.smoothed_rotation = self.smoothed_rotation.normalize();

        // --- Derive target from smoothed eye + rotation ---
        let eye = self.smoothed_position;
        let look_dir = self.smoothed_rotation * Vec3::NEG_Z;
        let target = eye + look_dir * 20.0;

        // --- FOV with descent speed boost ---
        let base_fov = self.settings.base_fov_degrees;
        let fov_degrees = if self.settings.descent_fov_boost_enabled && gradient < -0.01 {
            let speed_kmh = physics.speed * 3.6;
            let boost = ((speed_kmh - 30.0) / 20.0).clamp(0.0, 1.0) * 5.0 * intensity;
            base_fov + boost
        } else {
            base_fov
        };

        (eye, target, fov_degrees.to_radians())
    }

    /// Layer 1: Cadence bob — phase-locked to crank angle.
    fn compute_cadence_bob(&mut self, physics: &PhysicsState, intensity: f32, dt: f32) -> Vec3 {
        // Fade cadence bob when cadence drops to 0
        let cadence_target = if physics.cadence > 0 { 1.0 } else { 0.0 };
        self.cadence_fade = exp_toward(self.cadence_fade, cadence_target, 4.0, dt);

        if self.cadence_fade < 0.001 {
            return Vec3::ZERO;
        }

        let power_factor = (physics.power_watts / 400.0).clamp(0.3, 1.0);
        let phase = physics.crank_angle;

        // Two pedal strokes per crank revolution → use 2× frequency for Y bob
        let bob_y_phase = (phase * 2.0).sin();
        let bob_x_phase = phase.sin(); // lateral sway at crank frequency

        // Seated vs standing amplitudes
        let seated_y = 0.008; // 8mm
        let seated_x = 0.003; // 3mm
        let standing_y = 0.025; // 25mm
        let standing_x = 0.030; // 30mm

        let amp_y = lerp(seated_y, standing_y, self.standing_blend) * power_factor * intensity;
        let amp_x = lerp(seated_x, standing_x, self.standing_blend) * power_factor * intensity;

        Vec3::new(
            bob_x_phase * amp_x * self.cadence_fade,
            bob_y_phase * amp_y * self.cadence_fade,
            0.0,
        )
    }

    /// Layer 2: Effort sway — low-frequency oscillation above threshold + fatigue drift.
    fn compute_effort_sway(&mut self, physics: &PhysicsState, intensity: f32, dt: f32) -> (Vec3, f32) {
        self.sway_time += dt;

        let effort_ratio = physics.power_watts / physics.ftp_watts.max(1.0);

        // Only activate above 70% FTP
        let sway_factor = ((effort_ratio - 0.7) / 0.5).clamp(0.0, 1.0);

        let sway_offset = if sway_factor > 0.0 {
            let t = self.sway_time;
            // Two overlapping sine waves for organic feel
            let sway_x = (t * 0.37 * std::f32::consts::TAU).sin() * 0.6
                + (t * 0.53 * std::f32::consts::TAU).sin() * 0.4;
            let sway_y = (t * 0.29 * std::f32::consts::TAU).sin() * 0.5
                + (t * 0.67 * std::f32::consts::TAU).sin() * 0.5;

            let standing_mult = lerp(1.0, 1.5, self.standing_blend);
            let amp = 0.015 * sway_factor * standing_mult * intensity;

            Vec3::new(sway_x * amp, sway_y * amp * 0.5, 0.0)
        } else {
            Vec3::ZERO
        };

        // Fatigue drift: slow accumulation above 80% FTP → forward pitch
        if effort_ratio > 0.8 {
            let accum_rate = (effort_ratio - 0.8) * 0.02; // slow buildup
            self.fatigue_accumulator = (self.fatigue_accumulator + accum_rate * dt).min(1.0);
        } else if effort_ratio < 0.6 {
            let recovery_rate = 0.05;
            self.fatigue_accumulator = (self.fatigue_accumulator - recovery_rate * dt).max(0.0);
        }
        let fatigue_pitch = -3.0_f32.to_radians() * self.fatigue_accumulator * intensity;

        (sway_offset, fatigue_pitch)
    }

    /// Layer 3: Road vibration — surface-dependent high-frequency noise.
    fn compute_road_vibration(&self, physics: &PhysicsState, route_points: &[RoutePoint], intensity: f32) -> Vec3 {
        let surface = surface_at(route_points, physics.distance as f64);
        let base_amp = match surface {
            SurfaceType::Paved => 0.0005,  // 0.5mm
            SurfaceType::Gravel => 0.003,  // 3mm
        };

        let speed_scale = (physics.speed / 10.0).clamp(0.0, 3.0);
        let amp = base_amp * speed_scale * intensity;

        // Hash-based pseudo-random per frame (approximates 22Hz jitter)
        let hash_input = physics.distance * 73.856 + physics.elapsed_secs * 22.0 * 6.283;
        let h = fast_hash(hash_input);

        Vec3::new(
            h.0 * amp * 0.5,  // X: half amplitude
            h.1 * amp,        // Y: full amplitude
            h.2 * amp * 0.3,  // Z: 0.3x amplitude
        )
    }

    /// Layer 5: Corner look-ahead — anticipatory yaw and lean into turns.
    fn compute_corner_lookahead(
        &mut self,
        physics: &PhysicsState,
        route_points: &[RoutePoint],
        route_length: f32,
        intensity: f32,
        dt: f32,
        direction: RouteDirection,
    ) -> (f32, f32) {
        let speed = physics.speed;
        let dist = physics.distance;

        // Direction-aware look-ahead
        let lookahead_dist = 30.0 + 2.0 * speed;
        let ahead_dist = match direction {
            RouteDirection::Forward => (dist + lookahead_dist).min(route_length),
            RouteDirection::Reverse => (dist - lookahead_dist).max(0.0),
        };

        let (_, _, _, raw_fx_now, raw_fz_now) = interpolate_point(route_points, dist as f64);
        let (_, _, _, raw_fx_ahead, raw_fz_ahead) = interpolate_point(route_points, ahead_dist as f64);

        // Flip forward vectors when in reverse
        let (fx_now, fz_now, fx_ahead, fz_ahead) = match direction {
            RouteDirection::Forward => (raw_fx_now, raw_fz_now, raw_fx_ahead, raw_fz_ahead),
            RouteDirection::Reverse => (-raw_fx_now, -raw_fz_now, -raw_fx_ahead, -raw_fz_ahead),
        };

        // Curvature from cross product of forward vectors
        let curvature = fx_now * fz_ahead - fz_now * fx_ahead;

        // Yaw into turn: up to ±15°
        let max_yaw = 15.0_f32.to_radians();
        let yaw_target = (curvature * 5.0).clamp(-1.0, 1.0) * max_yaw * intensity;
        self.smoothed_lookahead_yaw = exp_toward(self.smoothed_lookahead_yaw, yaw_target, 1.5, dt);

        // Lean into turn: proportional to v²×curvature, up to ±25°
        let max_lean = 25.0_f32.to_radians();
        let lean_factor = speed * speed * curvature.abs() * 0.01;
        let lean_target = lean_factor.clamp(0.0, 1.0) * max_lean * curvature.signum() * intensity;
        self.smoothed_corner_lean = exp_toward(self.smoothed_corner_lean, lean_target, 3.0, dt);

        (self.smoothed_lookahead_yaw, self.smoothed_corner_lean)
    }
}

/// Standing detection: high power on steep gradient, or very high power anywhere.
fn detect_standing(physics: &PhysicsState) -> bool {
    let gradient_pct = physics.gradient * 100.0;
    (physics.power_watts > 350.0 && gradient_pct > 6.0) || physics.power_watts > 500.0
}

/// Exponential moving average toward target at given rate.
fn exp_toward(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let alpha = 1.0 - (-rate * dt).exp();
    current + (target - current) * alpha
}

/// EMA alpha from rate and dt: 1 - e^(-rate * dt).
fn ema_alpha(rate: f32, dt: f32) -> f32 {
    (1.0 - (-rate * dt).exp()).clamp(0.0, 1.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Build a look-at quaternion (forward = -Z convention).
fn look_rotation(eye: Vec3, target: Vec3) -> Quat {
    let forward = (target - eye).normalize_or_zero();
    if forward.length_squared() < 0.0001 {
        return Quat::IDENTITY;
    }
    // glam's from_rotation_arc rotates NEG_Z to forward
    let default_forward = Vec3::NEG_Z;
    let dot = default_forward.dot(forward);
    if dot > 0.9999 {
        Quat::IDENTITY
    } else if dot < -0.9999 {
        Quat::from_axis_angle(Vec3::Y, std::f32::consts::PI)
    } else {
        Quat::from_rotation_arc(default_forward, forward)
    }
}

/// Fast pseudo-random hash returning 3 values in [-1, 1].
fn fast_hash(input: f32) -> (f32, f32, f32) {
    let bits = input.to_bits();
    let a = bits.wrapping_mul(2654435761);
    let b = a.wrapping_mul(2246822519);
    let c = b.wrapping_mul(3266489917);
    let to_norm = |v: u32| (v as f32 / u32::MAX as f32) * 2.0 - 1.0;
    (to_norm(a), to_norm(b), to_norm(c))
}
