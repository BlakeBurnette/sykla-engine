// ============================================================
// Sykla Engine — PBR-ish shader with shadows and instancing
// ============================================================

struct CameraUniform {
    view_proj: mat4x4<f32>,
    eye_pos: vec4<f32>,
    light_vp: mat4x4<f32>,
};

struct LightUniform {
    direction: vec4<f32>,
    color: vec4<f32>,
    ambient: vec4<f32>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(0) var shadow_tex: texture_depth_2d;
@group(2) @binding(1) var shadow_samp: sampler_comparison;

// --- Vertex types ---

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct InstanceInput {
    @location(3) inst_pos: vec3<f32>,
    @location(4) inst_scale: f32,
};

struct AnimInstanceInput {
    @location(3) anim_pos: vec3<f32>,
    @location(4) anim_scale: f32,
    @location(5) anim_velocity: vec3<f32>,
    @location(6) anim_phase: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) shadow_pos: vec3<f32>,
};

// --- Helpers ---

fn world_to_shadow(world_pos: vec3<f32>) -> vec3<f32> {
    let lp = camera.light_vp * vec4<f32>(world_pos, 1.0);
    let ndc = lp.xyz / lp.w;
    return vec3<f32>(
        ndc.x * 0.5 + 0.5,
        1.0 - (ndc.y * 0.5 + 0.5),
        ndc.z
    );
}

// --- Main pass vertex shaders ---

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.world_normal = in.normal;
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(in.position);
    return out;
}

@vertex
fn vs_instanced(in: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let wp = in.position * inst.inst_scale + inst.inst_pos;
    out.clip_position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp;
    out.world_normal = in.normal;
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(wp);
    return out;
}

// --- Shadow pass vertex shaders ---

@vertex
fn vs_shadow(in: VertexInput) -> @builtin(position) vec4<f32> {
    return camera.light_vp * vec4<f32>(in.position, 1.0);
}

@vertex
fn vs_shadow_inst(in: VertexInput, inst: InstanceInput) -> @builtin(position) vec4<f32> {
    let wp = in.position * inst.inst_scale + inst.inst_pos;
    return camera.light_vp * vec4<f32>(wp, 1.0);
}

// --- Procedural noise ---

fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var pos = p;
    for (var i = 0; i < 4; i++) {
        v += a * noise2d(pos);
        pos *= 2.0;
        a *= 0.5;
    }
    return v;
}

// --- Shadow sampling ---

fn shadow_factor_biased(sp: vec3<f32>, normal: vec3<f32>) -> f32 {
    if sp.x < 0.001 || sp.x > 0.999 || sp.y < 0.001 || sp.y > 0.999 || sp.z > 1.0 {
        return 1.0;
    }

    // Slope-scaled bias — tighter range to minimize shadow seams between
    // adjacent terrain faces with slightly different normals
    let L = normalize(-light.direction.xyz);
    let NdotL = max(dot(normal, L), 0.0);
    let bias = mix(0.0015, 0.0005, NdotL);

    // 5x5 PCF for softer shadows
    var shadow = 0.0;
    let texel = 1.0 / 4096.0;
    let depth = sp.z - bias;

    for (var x = -2i; x <= 2i; x++) {
        for (var y = -2i; y <= 2i; y++) {
            let off = vec2<f32>(f32(x), f32(y)) * texel;
            shadow += textureSampleCompare(shadow_tex, shadow_samp, sp.xy + off, depth);
        }
    }
    return shadow / 25.0;
}

// --- Fragment shader ---

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    // Noise-based color variation — multi-octave for natural look
    let n1 = fbm(in.world_pos.xz * 0.06);
    let n2 = noise2d(in.world_pos.xz * 0.3);
    let n3 = noise2d(in.world_pos.xz * 1.2); // fine detail
    let color_var = material.base_color.xyz
        * (0.75 + 0.5 * n1)
        * (0.85 + 0.3 * n2)
        * (0.92 + 0.16 * n3);

    // Slope: dirt on steep areas, warm earth tones
    let slope = 1.0 - max(N.y, 0.0);
    let dirt = vec3<f32>(0.38, 0.28, 0.14);
    let base_color = mix(color_var, dirt, clamp(slope * 2.5, 0.0, 0.5));

    // Lighting
    let NdotL = dot(N, L);
    let diff = max(NdotL, 0.0);
    let wrap_diff = max(NdotL * 0.5 + 0.5, 0.0);
    let spec = pow(max(dot(N, H), 0.0), 64.0);

    // Subsurface-like translucency for foliage — warm glow when backlit
    let back_light = max(dot(-N, L), 0.0);
    let sss_color = vec3<f32>(0.45, 0.55, 0.1); // warm yellow-green glow
    let sss = back_light * back_light * 0.35 * sss_color * base_color;

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, normalize(in.world_normal));

    // Warm fill light from ground bounce
    let ground_bounce = max(N.y, 0.0) * 0.08;
    let bounce_color = vec3<f32>(0.3, 0.35, 0.1) * base_color * ground_bounce;

    // Combine — warm ambient with wrap lighting
    let ambient = light.ambient.xyz * base_color * (0.55 + 0.45 * wrap_diff);
    let sun_warm = light.color.xyz * vec3<f32>(1.0, 0.95, 0.85); // extra warmth
    let direct = shadow * (diff * sun_warm * base_color + spec * sun_warm * 0.06);
    var color = ambient + direct + sss * shadow + bounce_color;

    // Warm atmospheric haze — golden hour feel
    let dist = length(camera.eye_pos.xyz - in.world_pos);
    let height_factor = clamp((in.world_pos.y - 80.0) / 80.0, 0.0, 1.0);
    let fog_density = (1.0 - height_factor * 0.4) * clamp(dist / 1400.0, 0.0, 1.0);
    let fog_color = vec3<f32>(0.72, 0.68, 0.52); // warm golden haze
    color = mix(color, fog_color, fog_density * fog_density * 0.7);

    // Subtle tone mapping to prevent blowout
    color = color / (color + vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}

// ── Road fragment shader — clean surface, minimal noise ─────────

@fragment
fn fs_road(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    // Subtle micro-texture — very fine asphalt/gravel grain
    let n1 = noise2d(in.world_pos.xz * 2.0);
    let n2 = noise2d(in.world_pos.xz * 8.0);
    let color_var = material.base_color.xyz
        * (0.92 + 0.08 * n1)
        * (0.96 + 0.04 * n2);

    // Road edge darkening — darken near uv.x = 0 or 1
    let edge = abs(in.uv.x - 0.5) * 2.0; // 0 at center, 1 at edge
    let edge_darken = 1.0 - edge * edge * 0.25;
    let base_color = color_var * edge_darken;

    // Lighting (simpler than terrain — no subsurface)
    let NdotL = dot(N, L);
    let diff = max(NdotL, 0.0);
    let spec = pow(max(dot(N, H), 0.0), 32.0);

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, normalize(in.world_normal));

    let ambient = light.ambient.xyz * base_color * 0.6;
    let sun = light.color.xyz * vec3<f32>(1.0, 0.95, 0.85);
    let direct = shadow * (diff * sun * base_color + spec * sun * 0.04);
    var color = ambient + direct;

    // Same atmospheric haze
    let dist = length(camera.eye_pos.xyz - in.world_pos);
    let height_factor = clamp((in.world_pos.y - 80.0) / 80.0, 0.0, 1.0);
    let fog_density = (1.0 - height_factor * 0.4) * clamp(dist / 1400.0, 0.0, 1.0);
    let fog_color = vec3<f32>(0.72, 0.68, 0.52);
    color = mix(color, fog_color, fog_density * fog_density * 0.7);

    // Tone mapping
    color = color / (color + vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}

// ── Animated instance vertex shaders (wing flapping) ────────────

@vertex
fn vs_animated(in: VertexInput, inst: AnimInstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let time = camera.eye_pos.w;

    var local = in.position;
    // Wing flap: vertices further from X=0 displace more
    local.y += sin(time * 6.0 + inst.anim_phase) * 0.15 * abs(in.position.x) * inst.anim_scale;

    let wp = local * inst.anim_scale + inst.anim_pos;
    out.clip_position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp;
    out.world_normal = in.normal;
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(wp);
    return out;
}

@vertex
fn vs_shadow_animated(in: VertexInput, inst: AnimInstanceInput) -> @builtin(position) vec4<f32> {
    let time = camera.eye_pos.w;

    var local = in.position;
    local.y += sin(time * 6.0 + inst.anim_phase) * 0.15 * abs(in.position.x) * inst.anim_scale;

    let wp = local * inst.anim_scale + inst.anim_pos;
    return camera.light_vp * vec4<f32>(wp, 1.0);
}

// ── Water vertex + fragment shaders ─────────────────────────────

@vertex
fn vs_water(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let time = camera.eye_pos.w;

    var pos = in.position;
    // Wave displacement
    pos.y += sin(pos.z * 0.3 + time * 1.5) * 0.08 + sin(pos.x * 0.5 + time) * 0.05;

    // Perturbed normal from wave derivatives
    let dx = cos(pos.x * 0.5 + time) * 0.05 * 0.5;
    let dz = cos(pos.z * 0.3 + time * 1.5) * 0.08 * 0.3;
    let N = normalize(vec3<f32>(-dx, 1.0, -dz));

    out.clip_position = camera.view_proj * vec4<f32>(pos, 1.0);
    out.world_pos = pos;
    out.world_normal = N;
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(pos);
    return out;
}

@fragment
fn fs_water(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = camera.eye_pos.w;
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    // Scrolling FBM flow pattern
    let flow_uv = in.world_pos.xz * 0.15 + vec2<f32>(time * 0.08, time * 0.12);
    let flow = fbm(flow_uv);

    // Dark blue-green water base
    let water_base = vec3<f32>(0.05, 0.12, 0.18);
    let water_light = vec3<f32>(0.08, 0.20, 0.28);
    let water_color = mix(water_base, water_light, flow * 0.5);

    // Strong specular (pow 128)
    let spec = pow(max(dot(N, H), 0.0), 128.0);

    // Fresnel sky reflection
    let fresnel = pow(1.0 - max(dot(N, V), 0.0), 4.0);
    let sky_color = vec3<f32>(0.52, 0.70, 0.82);
    let reflected = mix(water_color, sky_color, fresnel * 0.6);

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, normalize(in.world_normal));

    // Lighting
    let NdotL = max(dot(N, L), 0.0);
    let ambient = light.ambient.xyz * water_color * 0.5;
    let direct = shadow * (NdotL * light.color.xyz * reflected + spec * light.color.xyz * 0.8);
    var color = ambient + direct;

    // Same fog as fs_main
    let dist = length(camera.eye_pos.xyz - in.world_pos);
    let height_factor = clamp((in.world_pos.y - 80.0) / 80.0, 0.0, 1.0);
    let fog_density = (1.0 - height_factor * 0.4) * clamp(dist / 1400.0, 0.0, 1.0);
    let fog_color = vec3<f32>(0.72, 0.68, 0.52);
    color = mix(color, fog_color, fog_density * fog_density * 0.7);

    // Tone mapping
    color = color / (color + vec3<f32>(1.0));

    return vec4<f32>(color, 0.85);
}

// ── HUD vertex + fragment shaders ───────────────────────────────

@vertex
fn vs_hud(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.world_normal = in.normal;
    out.uv = in.uv;
    out.shadow_pos = vec3<f32>(0.0);
    return out;
}

@fragment
fn fs_hud(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.world_normal, in.uv.x);
}
