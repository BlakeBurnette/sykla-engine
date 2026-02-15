//! Procedural cycling audio synthesis.
//!
//! Pure Rust, no platform dependencies. Platform-specific output (cpal / WebAudio)
//! lives in the app layer (demo.rs / app.rs).
//!
//! 6 layers: wind, chain, tire, breathing, freewheel, environment.
//! All driven by `AudioParams` copied from the game thread each frame.

use crate::terrain::SurfaceType;

// ── Public types ──────────────────────────────────────────────────────

/// Surface types extended for audio (future: Cobblestone, Dirt, Wet).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioSurface {
    Asphalt,
    Gravel,
    Cobblestone,
    Dirt,
    Wet,
}

impl AudioSurface {
    pub fn from_surface_type(s: SurfaceType) -> Self {
        match s {
            SurfaceType::Gravel => AudioSurface::Gravel,
            SurfaceType::Paved => AudioSurface::Asphalt,
        }
    }
}

/// Biome for environment ambience layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Biome {
    Forest,
    Alpine,
    Barren,
    Coastal,
}

impl Biome {
    /// Map from terrain_params[3] biome float used in grass/vegetation.
    pub fn from_terrain_param(val: f32) -> Self {
        if val < 0.5 {
            Biome::Barren
        } else if val < 1.5 {
            Biome::Forest
        } else if val < 2.5 {
            Biome::Forest
        } else if val < 3.5 {
            Biome::Alpine
        } else {
            Biome::Coastal
        }
    }
}

/// Game→audio bridge. Updated each frame from the game thread.
#[derive(Debug, Clone, Copy)]
pub struct AudioParams {
    pub speed_mps: f32,
    pub cadence_rpm: f32,
    pub power_watts: f32,
    pub gradient: f32,
    pub elevation_m: f32,
    pub surface: AudioSurface,
    pub biome: Biome,
    pub is_coasting: bool,
    pub wheel_rps: f32,
    pub forward_x: f32,
    pub forward_z: f32,
}

impl Default for AudioParams {
    fn default() -> Self {
        Self {
            speed_mps: 0.0,
            cadence_rpm: 0.0,
            power_watts: 0.0,
            gradient: 0.0,
            elevation_m: 100.0,
            surface: AudioSurface::Asphalt,
            biome: Biome::Forest,
            is_coasting: false,
            wheel_rps: 0.0,
            forward_x: 0.0,
            forward_z: 1.0,
        }
    }
}

pub const SAMPLE_RATE: f32 = 48000.0;
const INV_SR: f32 = 1.0 / SAMPLE_RATE;

// ── DSP primitives ────────────────────────────────────────────────────

/// Exponential moving average smoother. Per-sample smoothing.
struct Ema {
    value: f32,
    alpha: f32,
}

impl Ema {
    fn new(smoothing_ms: f32) -> Self {
        let samples = (smoothing_ms * 0.001 * SAMPLE_RATE).max(1.0);
        Self {
            value: 0.0,
            alpha: 1.0 / samples,
        }
    }

    fn tick(&mut self, target: f32) -> f32 {
        self.value += self.alpha * (target - self.value);
        self.value
    }

    #[allow(dead_code)]
    fn set(&mut self, v: f32) {
        self.value = v;
    }
}

/// Xorshift64 PRNG → white noise in [-1, 1].
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_f32(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        // Map to [-1, 1]
        (x as i64 as f64 / i64::MAX as f64) as f32
    }
}

/// Brown noise: integrated white with DC leak.
struct BrownNoise {
    rng: Xorshift64,
    state: f32,
    leak: f32,
}

impl BrownNoise {
    fn new(seed: u64) -> Self {
        Self {
            rng: Xorshift64::new(seed),
            state: 0.0,
            leak: 0.02,
        }
    }

    fn tick(&mut self) -> f32 {
        let white = self.rng.next_f32();
        self.state += white * 0.1;
        self.state -= self.state * self.leak;
        self.state
    }
}

/// Pink noise: 3-stage Voss-McCartney approximation.
struct PinkNoise {
    rng: Xorshift64,
    rows: [f32; 3],
    counter: u32,
    running_sum: f32,
}

impl PinkNoise {
    fn new(seed: u64) -> Self {
        Self {
            rng: Xorshift64::new(seed),
            rows: [0.0; 3],
            counter: 0,
            running_sum: 0.0,
        }
    }

    fn tick(&mut self) -> f32 {
        self.counter = self.counter.wrapping_add(1);
        // Determine which row to update based on trailing zeros
        let tz = self.counter.trailing_zeros().min(2);
        let old = self.rows[tz as usize];
        let new = self.rng.next_f32();
        self.rows[tz as usize] = new;
        self.running_sum += new - old;
        // Add white noise component and normalize
        let white = self.rng.next_f32();
        (self.running_sum + white) * 0.25
    }
}

/// Biquad IIR filter (Direct Form II Transposed).
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn set_lowpass(&mut self, freq: f32, q: f32) {
        let w0 = core::f32::consts::TAU * freq * INV_SR;
        let (sin_w, cos_w) = (w0.sin(), w0.cos());
        let alpha = sin_w / (2.0 * q);
        let a0 = 1.0 + alpha;
        let inv_a0 = 1.0 / a0;
        self.b0 = ((1.0 - cos_w) * 0.5) * inv_a0;
        self.b1 = (1.0 - cos_w) * inv_a0;
        self.b2 = self.b0;
        self.a1 = (-2.0 * cos_w) * inv_a0;
        self.a2 = (1.0 - alpha) * inv_a0;
    }

    fn set_highpass(&mut self, freq: f32, q: f32) {
        let w0 = core::f32::consts::TAU * freq * INV_SR;
        let (sin_w, cos_w) = (w0.sin(), w0.cos());
        let alpha = sin_w / (2.0 * q);
        let a0 = 1.0 + alpha;
        let inv_a0 = 1.0 / a0;
        self.b0 = ((1.0 + cos_w) * 0.5) * inv_a0;
        self.b1 = -(1.0 + cos_w) * inv_a0;
        self.b2 = self.b0;
        self.a1 = (-2.0 * cos_w) * inv_a0;
        self.a2 = (1.0 - alpha) * inv_a0;
    }

    fn set_bandpass(&mut self, freq: f32, q: f32) {
        let w0 = core::f32::consts::TAU * freq * INV_SR;
        let (sin_w, cos_w) = (w0.sin(), w0.cos());
        let alpha = sin_w / (2.0 * q);
        let a0 = 1.0 + alpha;
        let inv_a0 = 1.0 / a0;
        self.b0 = alpha * inv_a0;
        self.b1 = 0.0;
        self.b2 = -alpha * inv_a0;
        self.a1 = (-2.0 * cos_w) * inv_a0;
        self.a2 = (1.0 - alpha) * inv_a0;
    }

    fn tick(&mut self, input: f32) -> f32 {
        let out = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * out + self.z2;
        self.z2 = self.b2 * input - self.a2 * out;
        out
    }
}

fn soft_clip(x: f32) -> f32 {
    (x * 0.8).tanh()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ── Wind layer ────────────────────────────────────────────────────────

struct WindLayer {
    brown_l: BrownNoise,
    brown_r: BrownNoise,
    bp_l: Biquad,
    bp_r: Biquad,
    // Helmet turbulence
    turb_brown: BrownNoise,
    turb_hp: Biquad,
    turb_phase: f32,
    // Filter tracking
    last_center: f32,
}

impl WindLayer {
    fn new() -> Self {
        let mut s = Self {
            brown_l: BrownNoise::new(12345),
            brown_r: BrownNoise::new(67890),
            bp_l: Biquad::new(),
            bp_r: Biquad::new(),
            turb_brown: BrownNoise::new(11111),
            turb_hp: Biquad::new(),
            turb_phase: 0.0,
            last_center: 0.0,
        };
        s.turb_hp.set_highpass(800.0, 0.7);
        s
    }

    fn tick(&mut self, speed: f32, elevation: f32, forward_x: f32, _forward_z: f32) -> (f32, f32) {
        if speed < 0.5 {
            return (0.0, 0.0);
        }

        // Bandpass center tracks speed: 150Hz at 2m/s → 1200Hz at 20m/s
        let speed_t = ((speed - 2.0) / 18.0).clamp(0.0, 1.0);
        let center = lerp(150.0, 1200.0, speed_t);

        // Only recompute filter coefficients when center changes significantly
        if (center - self.last_center).abs() > 5.0 {
            self.bp_l.set_bandpass(center, 1.2);
            self.bp_r.set_bandpass(center, 1.2);
            self.last_center = center;
        }

        // Volume ∝ speed^1.5, normalized
        let vol = (speed / 15.0).powf(1.5).min(1.0) * 0.35;

        // Altitude thinning: gentle highpass
        let _alt_cutoff = 20.0 + elevation * 0.03;

        // Main wind
        let raw_l = self.brown_l.tick();
        let raw_r = self.brown_r.tick();
        let wind_l = self.bp_l.tick(raw_l);
        let wind_r = self.bp_r.tick(raw_r);

        // Helmet turbulence: AM modulated at 2-5Hz
        let turb_rate = lerp(2.0, 5.0, speed_t);
        self.turb_phase += turb_rate * INV_SR;
        if self.turb_phase > 1.0 {
            self.turb_phase -= 1.0;
        }
        let turb_mod = (self.turb_phase * core::f32::consts::TAU).sin() * 0.5 + 0.5;
        let turb_raw = self.turb_brown.tick();
        let turb = self.turb_hp.tick(turb_raw) * turb_mod * 0.3 * speed_t;

        // Stereo panning from heading (wind appears to come from the direction of travel)
        // Simple equal-power-ish pan: right gets more if forward_x > 0
        let pan = forward_x.clamp(-1.0, 1.0) * 0.3;
        let gain_l = vol * (1.0 - pan);
        let gain_r = vol * (1.0 + pan);

        (
            (wind_l + turb) * gain_l,
            (wind_r + turb) * gain_r,
        )
    }
}

// ── Chain layer ───────────────────────────────────────────────────────

struct ChainLayer {
    phase: f32,
    bp: Biquad,
    rng: Xorshift64,
    last_freq: f32,
}

impl ChainLayer {
    fn new() -> Self {
        let mut s = Self {
            phase: 0.0,
            bp: Biquad::new(),
            rng: Xorshift64::new(22222),
            last_freq: 0.0,
        };
        s.bp.set_bandpass(3000.0, 2.0);
        s
    }

    fn tick(&mut self, cadence: f32, power: f32, is_coasting: bool, surface: AudioSurface) -> (f32, f32) {
        if is_coasting || cadence < 5.0 {
            return (0.0, 0.0);
        }

        // Tooth frequency: (cadence/60) * 50 teeth
        let tooth_freq = (cadence / 60.0) * 50.0;
        self.phase += tooth_freq * INV_SR;
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }

        // Pulse: sharp impulse at each tooth
        let pulse = if self.phase < 0.05 {
            (-self.phase * 20.0 * tooth_freq / 50.0).exp()
        } else {
            0.0
        };

        // Tension shifts bandpass up with power
        let tension = (power / 300.0).clamp(0.0, 1.0);
        let bp_freq = lerp(2500.0, 4000.0, tension);
        if (bp_freq - self.last_freq).abs() > 30.0 {
            self.bp.set_bandpass(bp_freq, 2.0);
            self.last_freq = bp_freq;
        }

        let filtered = self.bp.tick(pulse);

        // Random extra impulses on gravel
        let gravel_extra = if surface == AudioSurface::Gravel {
            let r = self.rng.next_f32();
            if r > 0.97 { r * 0.3 } else { 0.0 }
        } else {
            0.0
        };

        let vol = lerp(0.06, 0.15, tension);
        let out = (filtered + gravel_extra) * vol;
        (out, out)
    }
}

// ── Tire layer ────────────────────────────────────────────────────────

struct TireLayer {
    pink: PinkNoise,
    white: Xorshift64,
    brown: BrownNoise,
    impulse_rng: Xorshift64,
    lp: Biquad,
    bp: Biquad,
    hp: Biquad,
    cobble_phase: f32,
    current_surface: AudioSurface,
}

impl TireLayer {
    fn new() -> Self {
        let mut s = Self {
            pink: PinkNoise::new(33333),
            white: Xorshift64::new(44444),
            brown: BrownNoise::new(55555),
            impulse_rng: Xorshift64::new(66666),
            lp: Biquad::new(),
            bp: Biquad::new(),
            hp: Biquad::new(),
            cobble_phase: 0.0,
            current_surface: AudioSurface::Asphalt,
        };
        s.lp.set_lowpass(2000.0, 0.7);
        s.bp.set_bandpass(3000.0, 1.5);
        s.hp.set_highpass(4000.0, 0.7);
        s
    }

    fn tick(&mut self, speed: f32, surface: AudioSurface) -> (f32, f32) {
        if speed < 0.3 {
            return (0.0, 0.0);
        }

        let vol = (speed / 12.0).min(1.0) * 0.2;

        // Reconfigure filters on surface change
        if surface != self.current_surface {
            self.current_surface = surface;
            match surface {
                AudioSurface::Asphalt => self.lp.set_lowpass(2000.0, 0.7),
                AudioSurface::Gravel => self.bp.set_bandpass(3000.0, 1.5),
                AudioSurface::Cobblestone => self.lp.set_lowpass(1000.0, 0.5),
                AudioSurface::Dirt => self.lp.set_lowpass(800.0, 0.7),
                AudioSurface::Wet => {
                    self.lp.set_lowpass(2000.0, 0.7);
                    self.hp.set_highpass(4000.0, 0.7);
                }
            }
        }

        let out = match surface {
            AudioSurface::Asphalt => {
                let raw = self.pink.tick();
                self.lp.tick(raw) * vol
            }
            AudioSurface::Gravel => {
                // Dense random impulse train
                let impulse = if self.impulse_rng.next_f32().abs() > 0.85 {
                    self.impulse_rng.next_f32() * 0.5
                } else {
                    0.0
                };
                let raw = self.pink.tick() * 0.5 + impulse;
                self.bp.tick(raw) * vol * 1.5
            }
            AudioSurface::Cobblestone => {
                // Periodic thumps
                let thump_freq = speed / 0.1;
                self.cobble_phase += thump_freq * INV_SR;
                if self.cobble_phase > 1.0 {
                    self.cobble_phase -= 1.0;
                }
                let thump = if self.cobble_phase < 0.1 {
                    (1.0 - self.cobble_phase * 10.0) * 0.8
                } else {
                    0.0
                };
                let rumble = self.brown.tick();
                self.lp.tick(thump + rumble * 0.3) * vol
            }
            AudioSurface::Dirt => {
                let raw = self.brown.tick();
                self.lp.tick(raw) * vol * 0.8
            }
            AudioSurface::Wet => {
                // Asphalt base + white noise sizzle
                let base = self.lp.tick(self.pink.tick());
                let sizzle = self.hp.tick(self.white.next_f32());
                (base + sizzle * 0.3) * vol
            }
        };

        (out, out)
    }
}

// ── Breathing layer ───────────────────────────────────────────────────

struct BreathingLayer {
    pink: PinkNoise,
    bp_inhale: Biquad,
    bp_exhale: Biquad,
    phase: f32,
    vocal_phase: f32,
}

impl BreathingLayer {
    fn new() -> Self {
        let mut s = Self {
            pink: PinkNoise::new(77777),
            bp_inhale: Biquad::new(),
            bp_exhale: Biquad::new(),
            phase: 0.0,
            vocal_phase: 0.0,
        };
        s.bp_inhale.set_bandpass(500.0, 1.5);
        s.bp_exhale.set_bandpass(350.0, 1.5);
        s
    }

    fn tick(&mut self, power: f32, cadence: f32) -> (f32, f32) {
        let effort = (power / 250.0).clamp(0.0, 1.0);

        // Breathing rate: 12 BPM at rest → 60 BPM at max effort
        let bpm = 12.0 + effort * 48.0;

        // Cadence entrainment at moderate effort
        let rate = if effort > 0.3 && effort < 0.7 && cadence > 60.0 {
            // Entrain to half cadence
            let entrained = cadence * 0.5;
            lerp(bpm, entrained, 0.3)
        } else {
            bpm
        };

        let cycle_freq = rate / 60.0;
        self.phase += cycle_freq * INV_SR;
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }

        let pink = self.pink.tick();

        // Inhale: first 40% of cycle, exhale: remaining 60%
        let out = if self.phase < 0.4 {
            // Inhale: sine envelope
            let env = (self.phase / 0.4 * core::f32::consts::PI).sin();
            self.bp_inhale.tick(pink) * env
        } else {
            // Exhale: decay envelope
            let exhale_t = (self.phase - 0.4) / 0.6;
            let env = (1.0 - exhale_t).max(0.0).powf(0.5);
            self.bp_exhale.tick(pink) * env
        };

        // Vocal harmonic above 80% effort
        let vocal = if effort > 0.8 {
            self.vocal_phase += 300.0 * INV_SR;
            if self.vocal_phase > 1.0 {
                self.vocal_phase -= 1.0;
            }
            let vocal_gain = (effort - 0.8) / 0.2;
            (self.vocal_phase * core::f32::consts::TAU).sin() * vocal_gain * 0.05
        } else {
            0.0
        };

        // Volume scales with effort, minimum audible breathing at rest
        let vol = lerp(0.03, 0.20, effort);
        let sample = (out + vocal) * vol;

        // Breathing is centered (mono-ish, slightly wider at high effort)
        let spread = effort * 0.15;
        (sample * (1.0 + spread), sample * (1.0 - spread))
    }
}

// ── Freewheel layer ───────────────────────────────────────────────────

struct FreewheelLayer {
    phase: f32,
    bp: Biquad,
    fade: Ema,
}

impl FreewheelLayer {
    fn new() -> Self {
        let mut s = Self {
            phase: 0.0,
            bp: Biquad::new(),
            fade: Ema::new(50.0),
        };
        s.bp.set_bandpass(5000.0, 3.0);
        s
    }

    fn tick(&mut self, wheel_rps: f32, is_coasting: bool) -> (f32, f32) {
        let target = if is_coasting && wheel_rps > 0.5 { 1.0 } else { 0.0 };
        let gain = self.fade.tick(target);

        if gain < 0.001 {
            return (0.0, 0.0);
        }

        // Pawl frequency: wheel_rps * 36 pawls
        let pawl_freq = wheel_rps * 36.0;
        self.phase += pawl_freq * INV_SR;
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }

        // Sharp pulse at each pawl engagement
        let pulse = if self.phase < 0.15 {
            (-self.phase * 30.0).exp()
        } else {
            0.0
        };

        let filtered = self.bp.tick(pulse);
        let vol = gain * 0.12;
        let out = filtered * vol;
        (out, out)
    }
}

// ── Environment layer ─────────────────────────────────────────────────

struct EnvironmentLayer {
    pink: PinkNoise,
    brown: BrownNoise,
    bp_high: Biquad,
    bp_low: Biquad,
    lp: Biquad,
    hp_whistle: Biquad,
    mod_phase: f32,
    current_biome: Biome,
}

impl EnvironmentLayer {
    fn new() -> Self {
        let mut s = Self {
            pink: PinkNoise::new(88888),
            brown: BrownNoise::new(99999),
            bp_high: Biquad::new(),
            bp_low: Biquad::new(),
            lp: Biquad::new(),
            hp_whistle: Biquad::new(),
            mod_phase: 0.0,
            current_biome: Biome::Forest,
        };
        s.bp_high.set_bandpass(4000.0, 1.0);
        s.bp_low.set_bandpass(400.0, 1.0);
        s.lp.set_lowpass(600.0, 0.7);
        s.hp_whistle.set_highpass(2000.0, 2.0);
        s
    }

    fn tick(&mut self, biome: Biome, speed: f32) -> (f32, f32) {
        if biome != self.current_biome {
            self.current_biome = biome;
            match biome {
                Biome::Forest => {
                    self.bp_high.set_bandpass(4000.0, 1.0);
                    self.bp_low.set_bandpass(400.0, 1.0);
                }
                Biome::Alpine => {
                    self.hp_whistle.set_highpass(2000.0, 2.0);
                }
                Biome::Coastal => {
                    self.lp.set_lowpass(600.0, 0.7);
                }
                Biome::Barren => {}
            }
        }

        match biome {
            Biome::Forest => {
                let canopy = self.bp_high.tick(self.pink.tick());
                let rustle = self.bp_low.tick(self.brown.tick());
                // Wind-modulated: louder when moving
                let wind_mod = lerp(0.5, 1.0, (speed / 10.0).min(1.0));
                let out = (canopy * 0.08 + rustle * 0.05) * wind_mod;
                (out, out * 0.9) // slight stereo offset
            }
            Biome::Alpine => {
                // Sparse highpass whistle
                let whistle = self.hp_whistle.tick(self.brown.tick());
                let out = whistle * 0.04;
                (out, out)
            }
            Biome::Barren => {
                // Near silence — just very faint wind
                let out = self.brown.tick() * 0.01;
                (out, out)
            }
            Biome::Coastal => {
                // Brown noise lowpass with slow modulation (waves)
                self.mod_phase += 0.1 * INV_SR;
                if self.mod_phase > 1.0 {
                    self.mod_phase -= 1.0;
                }
                let wave_mod = (self.mod_phase * core::f32::consts::TAU).sin() * 0.3 + 0.7;
                let raw = self.brown.tick();
                let filtered = self.lp.tick(raw);
                let out = filtered * 0.08 * wave_mod;
                (out, out)
            }
        }
    }
}

// ── Master synthesizer ────────────────────────────────────────────────

pub struct AudioSynth {
    wind: WindLayer,
    chain: ChainLayer,
    tire: TireLayer,
    breathing: BreathingLayer,
    freewheel: FreewheelLayer,
    environment: EnvironmentLayer,
    // Per-param smoothers
    speed_s: Ema,
    cadence_s: Ema,
    power_s: Ema,
    gradient_s: Ema,
    elevation_s: Ema,
    wheel_rps_s: Ema,
    forward_x_s: Ema,
    forward_z_s: Ema,
    // Current smoothed params
    params: AudioParams,
}

impl AudioSynth {
    pub fn new() -> Self {
        Self {
            wind: WindLayer::new(),
            chain: ChainLayer::new(),
            tire: TireLayer::new(),
            breathing: BreathingLayer::new(),
            freewheel: FreewheelLayer::new(),
            environment: EnvironmentLayer::new(),
            speed_s: Ema::new(80.0),
            cadence_s: Ema::new(100.0),
            power_s: Ema::new(120.0),
            gradient_s: Ema::new(200.0),
            elevation_s: Ema::new(500.0),
            wheel_rps_s: Ema::new(80.0),
            forward_x_s: Ema::new(60.0),
            forward_z_s: Ema::new(60.0),
            params: AudioParams::default(),
        }
    }

    /// Update params from the game thread. Call once per frame (or whenever params change).
    pub fn set_params(&mut self, params: AudioParams) {
        self.params = params;
    }

    /// Fill an interleaved stereo f32 buffer. Called from the audio callback.
    pub fn fill_buffer(&mut self, output: &mut [f32]) {
        let p = self.params;

        for frame in output.chunks_exact_mut(2) {
            // Smooth all params per sample
            let speed = self.speed_s.tick(p.speed_mps);
            let cadence = self.cadence_s.tick(p.cadence_rpm);
            let power = self.power_s.tick(p.power_watts);
            let _gradient = self.gradient_s.tick(p.gradient);
            let elevation = self.elevation_s.tick(p.elevation_m);
            let wheel_rps = self.wheel_rps_s.tick(p.wheel_rps);
            let forward_x = self.forward_x_s.tick(p.forward_x);
            let forward_z = self.forward_z_s.tick(p.forward_z);

            // Tick all layers
            let (wl, wr) = self.wind.tick(speed, elevation, forward_x, forward_z);
            let (cl, cr) = self.chain.tick(cadence, power, p.is_coasting, p.surface);
            let (tl, tr) = self.tire.tick(speed, p.surface);
            let (bl, br) = self.breathing.tick(power, cadence);
            let (fl, fr) = self.freewheel.tick(wheel_rps, p.is_coasting);
            let (el, er) = self.environment.tick(p.biome, speed);

            // Sum and soft-clip
            let left = soft_clip(wl + cl + tl + bl + fl + el);
            let right = soft_clip(wr + cr + tr + br + fr + er);

            frame[0] = left;
            frame[1] = right;
        }
    }
}
