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
    fog_color: vec4<f32>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    mid_color: vec4<f32>,
    high_color: vec4<f32>,
    zone_params: vec4<f32>,
    terrain_params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var base_tex: texture_2d<f32>;
@group(1) @binding(2) var base_samp: sampler;
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

/// Parametric FBM with variable octave count (1-6)
fn fbm_n(p: vec2<f32>, octaves: i32) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i++) {
        v += a * noise2d(pos);
        pos *= 2.0;
        a *= 0.5;
    }
    return v;
}

/// Voronoi edge distance — returns 0 at cell centers, ~1 at edges between cells.
/// Used for cracked mud / dried earth patterns.
fn voronoi_edge(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    var min_d = 1.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let neighbor = cell + vec2<f32>(f32(dx), f32(dy));
            let jitter = hash(neighbor);
            let jitter2 = hash(neighbor + vec2<f32>(37.0, 17.0));
            let center = neighbor + vec2<f32>(jitter, jitter2) * 0.8;
            let d = length(p - center);
            min_d = min(min_d, d);
        }
    }
    return smoothstep(0.35, 0.50, min_d);
}

// ── Biome ground cover functions ────────────────────────────────────

/// NC Piedmont forest floor — pine needles, leaf litter, moss, red clay
fn compute_piedmont_floor(
    world_pos: vec3<f32>,
    dist_from_center: f32,
    slope: f32,
    zone_color: vec3<f32>,
) -> vec3<f32> {
    // Piedmont NC palette
    let clay = vec3<f32>(0.55, 0.32, 0.18);        // red clay
    let pine_needles = vec3<f32>(0.45, 0.30, 0.15); // warm brown-orange carpet
    let leaf_litter = vec3<f32>(0.38, 0.28, 0.14);  // mixed deciduous leaf litter
    let moss = vec3<f32>(0.22, 0.35, 0.15);         // muted green moss
    let grass = vec3<f32>(0.25, 0.42, 0.12);        // maintained grass near path

    // Multi-scale noise layers
    let n_broad = fbm_n(world_pos.xz * 0.1, 3);    // ~10m patches
    let n_medium = fbm_n(world_pos.xz * 0.5, 2);   // ~2m detail
    let n_fine = fbm_n(world_pos.xz * 3.0, 2);     // ~0.3m grain

    // Approximate canopy density from distance to path
    // Trees placed ~7-40m from path center; densest in mid-range
    let canopy = smoothstep(0.05, 0.15, dist_from_center) * (1.0 - smoothstep(0.7, 1.0, dist_from_center));

    // Layer blending based on canopy density
    var color: vec3<f32>;
    if (canopy > 0.5) {
        // Dense canopy — pine needles + leaf litter dominate
        color = mix(pine_needles, leaf_litter, smoothstep(0.3, 0.7, n_broad));
        // Occasional moss patches in damp areas
        color = mix(color, moss, smoothstep(0.7, 0.9, n_medium) * 0.35);
    } else if (canopy > 0.2) {
        // Mixed zone — grass where light reaches, needles in shade
        let shade_mix = mix(pine_needles, grass, 0.5);
        color = mix(shade_mix, leaf_litter, smoothstep(0.4, 0.6, n_broad));
    } else {
        // Open — maintained grass with occasional bare patches
        color = mix(grass, clay, smoothstep(0.6, 0.8, n_broad) * 0.25);
    }

    // Slope-based blending: steep → exposed clay and roots
    let slope_dirt = smoothstep(0.15, 0.35, slope);
    color = mix(color, clay, slope_dirt * 0.5);

    // Fine grain variation prevents "painted" look
    color *= 0.75 + n_fine * 0.5;

    // Near path: more worn, bare soil visible
    let path_wear = smoothstep(0.02, 0.08, dist_from_center);
    color = mix(mix(color, clay, 0.3), color, path_wear);

    return color;
}

/// Alpine ground — meadow grass → rocky scree → snow at high elevation
fn compute_alpine_ground(
    world_pos: vec3<f32>,
    dist_from_center: f32,
    slope: f32,
    zone_color: vec3<f32>,
    elevation: f32,
    zone_params: vec4<f32>,
) -> vec3<f32> {
    let meadow = vec3<f32>(0.35, 0.55, 0.20);       // alpine meadow
    let scree = vec3<f32>(0.50, 0.48, 0.44);         // rocky scree
    let rock = vec3<f32>(0.36, 0.48, 0.54);          // slate blue-grey
    let snow = vec3<f32>(0.92, 0.93, 0.96);          // blue-white snow
    let snow_shadow = vec3<f32>(0.65, 0.70, 0.82);   // blue shadows in snow

    let n_broad = fbm_n(world_pos.xz * 0.08, 3);
    let n_medium = fbm_n(world_pos.xz * 0.4, 2);
    let n_fine = fbm_n(world_pos.xz * 3.0, 2);

    // Elevation-based zone transitions
    let t_rock = smoothstep(zone_params.x, zone_params.y, elevation);
    let t_snow = smoothstep(zone_params.z, zone_params.w, elevation);

    // Base ground at this elevation
    var base = mix(meadow, scree, t_rock);
    base = mix(base, rock, smoothstep(0.3, 0.6, slope) * t_rock);

    // Snow coverage — more on flat areas, less on steep slopes and wind-exposed
    let snow_base = smoothstep(0.2, 0.6, 1.0 - slope); // less on steep
    let snow_noise = n_broad * 0.3 + n_medium * 0.15;
    let snow_coverage = t_snow * snow_base * (0.7 + snow_noise);

    // Snow surface with subtle variation
    let snow_color = mix(snow_shadow, snow, 0.5 + n_medium * 0.3);
    // Sparkle — high freq noise thresholded
    let sparkle_noise = noise2d(world_pos.xz * 20.0);
    let sparkle = smoothstep(0.92, 0.95, sparkle_noise) * 0.5;
    let snow_final = snow_color + vec3<f32>(sparkle);

    var color = mix(base, snow_final, clamp(snow_coverage, 0.0, 1.0));

    // Slope-based rock exposure even in snow zone
    let steep_rock = smoothstep(0.5, 0.7, slope);
    color = mix(color, rock, steep_rock * 0.6);

    // Fine grain
    color *= 0.80 + n_fine * 0.4;

    return color;
}

/// Desert/barren ground — sand ripples, cracked mud, exposed rock
fn compute_desert_ground(
    world_pos: vec3<f32>,
    dist_from_center: f32,
    slope: f32,
    zone_color: vec3<f32>,
) -> vec3<f32> {
    let sand_light = vec3<f32>(0.85, 0.72, 0.50);   // sunlit sand
    let sand_dark = vec3<f32>(0.65, 0.52, 0.35);     // shadow side of ripples
    let hardpan = vec3<f32>(0.72, 0.62, 0.48);       // packed earth
    let rock = vec3<f32>(0.55, 0.38, 0.28);          // exposed red rock
    let rock_varnish = vec3<f32>(0.25, 0.18, 0.15);  // dark desert varnish
    let limestone = vec3<f32>(0.78, 0.75, 0.68);     // white limestone (Ventoux)

    // Wind ripple pattern — waves perpendicular to a fixed wind direction
    let wind_perp = vec2<f32>(0.7, 0.7);
    let ripple_coord = dot(world_pos.xz, wind_perp);
    let dune = sin(ripple_coord * 0.05 + fbm_n(world_pos.xz * 0.01, 2) * 3.0);
    let ripple = sin(ripple_coord * 2.0 + fbm_n(world_pos.xz * 0.3, 2) * 1.5);
    let ripple_shade = 0.5 + ripple * 0.12 + dune * 0.08;

    var sand_color = mix(sand_dark, sand_light, ripple_shade);

    // Surface type by slope
    let rock_blend = smoothstep(0.4, 0.7, slope);
    let hardpan_blend = smoothstep(0.15, 0.4, slope) * (1.0 - rock_blend);
    var color = sand_color * (1.0 - rock_blend - hardpan_blend)
              + hardpan * hardpan_blend
              + limestone * rock_blend;

    // Desert varnish on steep rock faces
    let n_varnish = fbm_n(world_pos.xz * 0.5, 2);
    let varnish = smoothstep(0.6, 0.8, slope) * smoothstep(0.5, 0.8, n_varnish);
    color = mix(color, rock_varnish, varnish * 0.3);

    // Cracked mud pattern on flat areas
    if (slope < 0.2) {
        let crack = voronoi_edge(world_pos.xz * 0.7);
        color = mix(color, color * 0.7, crack * (1.0 - rock_blend) * 0.5);
    }

    // Fine grain
    let grain = fbm_n(world_pos.xz * 8.0, 1);
    color *= 0.80 + grain * 0.4;

    // Near path — packed gravel/sand
    let path_wear = smoothstep(0.02, 0.06, dist_from_center);
    let packed = mix(hardpan, sand_color * 0.9, 0.5);
    color = mix(packed, color, path_wear);

    return color;
}

/// Coastal ground — sandy soil, scrubby vegetation, sea influence
fn compute_coastal_ground(
    world_pos: vec3<f32>,
    dist_from_center: f32,
    slope: f32,
    zone_color: vec3<f32>,
) -> vec3<f32> {
    let sand = vec3<f32>(0.82, 0.75, 0.58);          // sandy soil
    let coastal_grass = vec3<f32>(0.35, 0.45, 0.22);  // salt-tolerant grass
    let scrub = vec3<f32>(0.30, 0.38, 0.18);          // scrubby vegetation
    let dark_soil = vec3<f32>(0.32, 0.28, 0.22);      // rich dark soil

    let n_broad = fbm_n(world_pos.xz * 0.08, 3);
    let n_medium = fbm_n(world_pos.xz * 0.4, 2);
    let n_fine = fbm_n(world_pos.xz * 3.0, 2);

    // Patchy mix of sand and coastal grass
    var color = mix(coastal_grass, sand, smoothstep(0.4, 0.6, n_broad));
    // Scrub patches
    color = mix(color, scrub, smoothstep(0.6, 0.8, n_medium) * 0.3);

    // Slope — expose sandy soil
    let slope_sand = smoothstep(0.2, 0.4, slope);
    color = mix(color, sand, slope_sand * 0.4);

    // Occasional dark soil patches
    color = mix(color, dark_soil, smoothstep(0.8, 0.95, n_broad) * 0.2);

    // Fine grain
    color *= 0.76 + n_fine * 0.48;

    // Near path — compact sand/gravel
    let path_wear = smoothstep(0.02, 0.08, dist_from_center);
    color = mix(mix(color, sand * 0.9, 0.3), color, path_wear);

    return color;
}

/// Generic lush forest floor
fn compute_forest_floor(
    world_pos: vec3<f32>,
    dist_from_center: f32,
    slope: f32,
    zone_color: vec3<f32>,
) -> vec3<f32> {
    let dark_soil = vec3<f32>(0.22, 0.18, 0.12);     // rich forest humus
    let leaf_litter = vec3<f32>(0.35, 0.26, 0.14);    // decomposing leaves
    let moss = vec3<f32>(0.18, 0.32, 0.12);           // forest moss
    let grass = vec3<f32>(0.22, 0.38, 0.10);          // shade-tolerant grass
    let root_dirt = vec3<f32>(0.28, 0.20, 0.10);      // exposed root/dirt

    let n_broad = fbm_n(world_pos.xz * 0.1, 3);
    let n_medium = fbm_n(world_pos.xz * 0.5, 2);
    let n_fine = fbm_n(world_pos.xz * 3.0, 2);

    // Dense forest floor — leaf litter with moss patches
    var color = mix(leaf_litter, dark_soil, smoothstep(0.3, 0.6, n_broad));
    color = mix(color, moss, smoothstep(0.6, 0.85, n_medium) * 0.4);

    // Canopy approximation — denser away from path
    let openness = 1.0 - smoothstep(0.05, 0.3, dist_from_center);
    color = mix(color, grass, openness * 0.4);

    // Slope — exposed roots and dirt
    let slope_dirt = smoothstep(0.2, 0.4, slope);
    color = mix(color, root_dirt, slope_dirt * 0.4);

    // Fine grain
    color *= 0.75 + n_fine * 0.5;

    // Near path — packed dirt
    let path_wear = smoothstep(0.02, 0.08, dist_from_center);
    let trail_dirt = vec3<f32>(0.42, 0.36, 0.26);
    color = mix(trail_dirt, color, path_wear);

    return color;
}

// --- Terrain bump normal ---

/// Multi-scale terrain normal perturbation from noise derivatives.
/// Three scales: broad geological (~7m), medium clumps (~1.2m), fine grain (~25cm).
/// Returns a perturbed normal vector — apply before lighting.
fn terrain_bump_normal(N: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let wp = world_pos.xz;
    let eps = 0.05;

    // Scale 1: broad geological undulation (~7m wavelength)
    let s1 = 0.14; // 1/7m
    let n1_x0 = noise2d((wp + vec2<f32>(eps, 0.0)) * s1) - noise2d((wp - vec2<f32>(eps, 0.0)) * s1);
    let n1_z0 = noise2d((wp + vec2<f32>(0.0, eps)) * s1) - noise2d((wp - vec2<f32>(0.0, eps)) * s1);
    let grad1 = vec3<f32>(n1_x0, 0.0, n1_z0) * 0.35;

    // Scale 2: medium clumps (~1.2m wavelength)
    let s2 = 0.83; // 1/1.2m
    let n2_x0 = noise2d((wp + vec2<f32>(eps, 0.0)) * s2) - noise2d((wp - vec2<f32>(eps, 0.0)) * s2);
    let n2_z0 = noise2d((wp + vec2<f32>(0.0, eps)) * s2) - noise2d((wp - vec2<f32>(0.0, eps)) * s2);
    let grad2 = vec3<f32>(n2_x0, 0.0, n2_z0) * 0.25;

    // Scale 3: fine grain (~25cm wavelength)
    let s3 = 4.0; // 1/0.25m
    let n3_x0 = noise2d((wp + vec2<f32>(eps, 0.0)) * s3) - noise2d((wp - vec2<f32>(eps, 0.0)) * s3);
    let n3_z0 = noise2d((wp + vec2<f32>(0.0, eps)) * s3) - noise2d((wp - vec2<f32>(0.0, eps)) * s3);
    let grad3 = vec3<f32>(n3_x0, 0.0, n3_z0) * 0.15;

    return normalize(N + grad1 + grad2 + grad3);
}

// --- Shadow sampling ---

fn shadow_factor_biased(sp: vec3<f32>, normal: vec3<f32>) -> f32 {
    // Bounds check — computed uniformly, applied after sampling to avoid
    // non-uniform control flow (required by WebGPU for textureSampleCompare)
    let in_bounds = sp.x >= 0.001 && sp.x <= 0.999
                 && sp.y >= 0.001 && sp.y <= 0.999
                 && sp.z <= 1.0;

    // Clamp UVs so out-of-bounds fragments still sample safely
    let uv = clamp(sp.xy, vec2<f32>(0.001), vec2<f32>(0.999));

    // Slope-scaled bias — wider range to avoid shadow acne on biome-textured terrain
    let L = normalize(-light.direction.xyz);
    let NdotL = max(dot(normal, L), 0.0);
    let bias = mix(0.003, 0.001, NdotL);

    // 5x5 PCF for softer shadows
    var shadow = 0.0;
    let texel = 1.0 / 4096.0;
    let depth = sp.z - bias;

    for (var x = -2i; x <= 2i; x++) {
        for (var y = -2i; y <= 2i; y++) {
            let off = vec2<f32>(f32(x), f32(y)) * texel;
            shadow += textureSampleCompare(shadow_tex, shadow_samp, uv + off, depth);
        }
    }

    // Out-of-bounds = fully lit (no shadow)
    return select(shadow / 25.0, 1.0, !in_bounds);
}

// --- Atmospheric perspective ---

fn apply_atmosphere(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dist = length(camera.eye_pos.xyz - world_pos);
    let base_elev = light.fog_color.w;

    // Height factor — higher objects see less fog
    let height_factor = clamp((world_pos.y - base_elev) / 200.0, 0.0, 1.0);

    // Valley fog — denser below base elevation
    let valley_factor = 1.0 + clamp((base_elev - world_pos.y) / 50.0, 0.0, 2.0);

    // Base exponential fog
    let fog_range = 1400.0 + base_elev * 1.5;
    let density = 0.7 / fog_range * (1.0 - height_factor * 0.4) * valley_factor;
    let fog_amount = 1.0 - exp(-dist * density);

    // Aerial perspective — blue shift increases with distance squared
    let aerial_color = vec3<f32>(0.60, 0.72, 0.85);
    let aerial_dist = dist / fog_range;
    let aerial_amount = clamp(aerial_dist * aerial_dist * 0.3, 0.0, 0.4);

    // Desaturation — distant objects lose color contrast
    let lum = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    let desat = mix(color, vec3<f32>(lum), aerial_amount * 0.6);

    // Combine: fog first, then aerial blue shift
    var result = mix(desat, light.fog_color.xyz, fog_amount);
    result = mix(result, aerial_color, aerial_amount * (1.0 - fog_amount));

    // Golden sun rays — directional in-scattering (Mie-like forward scatter)
    let view_dir = normalize(world_pos - camera.eye_pos.xyz);
    let sun_dir = normalize(-light.direction.xyz);
    let sun_dot = max(dot(view_dir, sun_dir), 0.0);
    // Concentrated forward lobe + broad halo
    let scatter = pow(sun_dot, 8.0) * 0.35 + pow(sun_dot, 64.0) * 0.5;
    // Grows with distance (more atmosphere = more scattering)
    let scatter_dist = clamp(dist / 200.0, 0.0, 1.0);
    // Shadow attenuation — scattered light is blocked in shadowed areas
    let shadow_atten = clamp(dot(normalize(vec3<f32>(0.0, 1.0, 0.0)), sun_dir) + 0.3, 0.0, 1.0);
    let sun_color = vec3<f32>(1.0, 0.85, 0.45); // warm golden
    result = result + sun_color * scatter * scatter_dist * shadow_atten * light.color.w;

    return result;
}

// --- Color grading: tone mapping + warmth + saturation ---

fn color_grade(color: vec3<f32>) -> vec3<f32> {
    // ACES filmic tone mapping (approximation by Krzysztof Narkowicz)
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    var mapped = clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3(0.0), vec3(1.0));

    // Warm shift — push slightly toward orange
    mapped = mapped * vec3<f32>(1.025, 1.0, 0.96);

    // Saturation boost
    let lum = dot(mapped, vec3<f32>(0.299, 0.587, 0.114));
    mapped = mix(vec3(lum), mapped, 1.1);

    return clamp(mapped, vec3(0.0), vec3(1.0));
}

// --- Volumetric light shafts (god rays) ---

fn volumetric_light(world_pos: vec3<f32>) -> vec3<f32> {
    let ray = camera.eye_pos.xyz - world_pos;
    let ray_len = min(length(ray), 80.0);
    let ray_dir = normalize(ray);
    let step_vec = ray_dir * (ray_len / 12.0);
    var accum = 0.0;
    var pos = world_pos;
    for (var i = 0u; i < 12u; i++) {
        pos += step_vec;
        let sp = world_to_shadow(pos);
        let in_bounds = sp.x >= 0.001 && sp.x <= 0.999
                     && sp.y >= 0.001 && sp.y <= 0.999
                     && sp.z <= 1.0;
        let uv = clamp(sp.xy, vec2<f32>(0.001), vec2<f32>(0.999));
        let lit = textureSampleCompare(shadow_tex, shadow_samp, uv, sp.z - 0.002);
        accum += select(lit, 1.0, !in_bounds);
    }
    let view_dir = normalize(world_pos - camera.eye_pos.xyz);
    let sun_dir = normalize(-light.direction.xyz);
    let phase = pow(max(dot(view_dir, sun_dir), 0.0), 8.0);
    let warm = vec3<f32>(1.0, 0.9, 0.7);
    return accum / 12.0 * phase * 0.20 * warm * light.color.w;
}

// --- Fragment shader ---

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    // Elevation zone blending — smoothstep between base/mid/high colors
    let t_low = smoothstep(material.zone_params.x, material.zone_params.y, in.world_pos.y);
    let t_high = smoothstep(material.zone_params.z, material.zone_params.w, in.world_pos.y);
    let zone_color = mix(mix(material.base_color.xyz, material.mid_color.xyz, t_low), material.high_color.xyz, t_high);

    // Noise-based color variation — multi-octave for natural look
    let noise_scale = material.terrain_params.z;
    let noise_boost = 1.0 + t_high * 0.3;
    let n1 = fbm(in.world_pos.xz * 0.06 * noise_scale);
    let n2 = noise2d(in.world_pos.xz * 0.3 * noise_scale);
    let n3 = noise2d(in.world_pos.xz * 1.2); // fine detail always at native scale
    let color_var = zone_color
        * (0.75 + 0.5 * n1 * noise_boost)
        * (0.85 + 0.3 * n2 * noise_boost)
        * (0.92 + 0.16 * n3);

    // Slope factor: 0=flat, 1=cliff
    let slope = 1.0 - max(N.y, 0.0);

    // Terrain splatting — biome-aware ground cover when terrain_params.w > 0
    // terrain_params.w encodes biome type: 1=piedmont, 2=alpine, 3=desert, 4=coastal, 5=forest
    var base_color: vec3<f32>;
    let biome_type = material.terrain_params.w;
    if (biome_type > 0.5) {
        // Distance from path center: uv.x encodes lateral position (0.5 = center)
        let dist_from_center = abs(in.uv.x - 0.5) * 2.0; // 0=center, 1=edge

        // Biome dispatch
        var ground: vec3<f32>;
        if (biome_type < 1.5) {
            // Biome 1: NC Piedmont forest floor
            ground = compute_piedmont_floor(in.world_pos, dist_from_center, slope, zone_color);
        } else if (biome_type < 2.5) {
            // Biome 2: Alpine (meadow → rock → snow)
            ground = compute_alpine_ground(
                in.world_pos, dist_from_center, slope, zone_color,
                in.world_pos.y, material.zone_params,
            );
        } else if (biome_type < 3.5) {
            // Biome 3: Desert / barren
            ground = compute_desert_ground(in.world_pos, dist_from_center, slope, zone_color);
        } else if (biome_type < 4.5) {
            // Biome 4: Coastal
            ground = compute_coastal_ground(in.world_pos, dist_from_center, slope, zone_color);
        } else {
            // Biome 5: Generic lush forest
            ground = compute_forest_floor(in.world_pos, dist_from_center, slope, zone_color);
        }

        base_color = ground;
    } else {
        // Non-splatted path (original logic)
        let dirt = mix(vec3<f32>(0.38, 0.28, 0.14), material.mid_color.xyz, t_high);
        let dirt_strength = clamp(slope * 2.5, 0.0, 0.5) * (1.0 - t_high * 0.7);
        base_color = mix(color_var, dirt, dirt_strength);
    }

    // Snow-on-normals: foliage marked with mid_color.w > 1.5 gets snow on upward-facing surfaces
    if (material.mid_color.w > 1.5) {
        let snow_color = vec3<f32>(0.94, 0.94, 0.91); // #F0F0E8
        let snow_t = smoothstep(0.25, 0.65, N.y);
        base_color = mix(base_color, snow_color, snow_t);
    }

    // Terrain bump normal — multi-scale perturbation for visible surface texture
    let N_bumped = terrain_bump_normal(N, in.world_pos);

    // Lighting (use bumped normal for all terrain lighting)
    let NdotL = dot(N_bumped, L);
    let diff = max(NdotL, 0.0);
    let wrap_diff = max(NdotL * 0.5 + 0.5, 0.0);
    let spec = pow(max(dot(N_bumped, H), 0.0), 64.0);

    // Subsurface-like translucency for foliage — warm glow when backlit
    // Reduce above treeline (high zones are rock/snow, not foliage)
    let back_light = max(dot(-N_bumped, L), 0.0);
    let sss_color = vec3<f32>(0.45, 0.55, 0.1); // warm yellow-green glow
    let sss_strength = 0.35 * (1.0 - t_high);
    let sss = back_light * back_light * sss_strength * sss_color * base_color;

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, normalize(in.world_normal));

    // Warm fill light from ground bounce
    let ground_bounce = max(N_bumped.y, 0.0) * 0.08;
    let bounce_color = vec3<f32>(0.3, 0.35, 0.1) * base_color * ground_bounce;

    // Combine — warm ambient with wrap lighting
    // Blue snow shadows: tint ambient toward cool blue in high-elevation snow zones
    let snow_ambient_tint = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.75, 0.82, 1.0), t_high);
    let ambient = light.ambient.xyz * snow_ambient_tint * base_color * (0.55 + 0.45 * wrap_diff);
    let sun_warm = light.color.xyz * vec3<f32>(1.0, 0.95, 0.85); // extra warmth
    let direct = shadow * (diff * sun_warm * base_color + spec * sun_warm * 0.06);
    var color = ambient + direct + sss * shadow + bounce_color;

    // Volumetric light shafts (god rays through tree canopy)
    color += volumetric_light(in.world_pos);

    // Atmospheric perspective
    color = apply_atmosphere(color, in.world_pos);

    return vec4<f32>(color, 1.0);
}

// ── Road fragment shader — clean surface, minimal noise ─────────

@fragment
fn fs_road(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    let ux = in.uv.x;           // 0=left edge, 1=right edge
    let edge = abs(ux - 0.5) * 2.0; // 0 at center, 1 at edge

    var base_color: vec3<f32>;
    var surface_spec = 32.0;
    var spec_strength = 0.04;

    let is_gravel = material.terrain_params.x > 0.5;

    var N_perturbed = N;

    if (is_gravel) {
        // ── Packed dirt/crushed stone trail (American Tobacco Trail style) ──
        // Goal: feel every grain, see the pine needles, sense the depth.

        let wp = in.world_pos.xz;
        let road_dist = in.uv.y;
        let cam_dist = length(camera.eye_pos.xyz - in.world_pos);
        let close_detail = 1.0 - smoothstep(3.0, 15.0, cam_dist);
        let mid_detail = 1.0 - smoothstep(8.0, 30.0, cam_dist);

        // ── Color palette ──
        let stone_base = material.base_color.xyz;
        let stone_warm = vec3<f32>(0.68, 0.55, 0.38);  // warm sandy stone
        let stone_cool = vec3<f32>(0.62, 0.62, 0.60);  // cool gray gravel
        let dirt_color = vec3<f32>(0.48, 0.36, 0.22);   // exposed dirt
        let clay_color = vec3<f32>(0.58, 0.32, 0.18);   // reddish clay patches
        let needle_dark = vec3<f32>(0.28, 0.15, 0.06);  // dry pine needle
        let needle_light = vec3<f32>(0.45, 0.28, 0.12); // fresher needle
        let cone_color = vec3<f32>(0.22, 0.14, 0.07);   // pinecone

        // ── Individual gravel grains (voronoi-like cells) ──
        // Each grain is a tiny stone with its own color and height
        let grain_scale = 50.0; // ~2cm grains
        let gc = floor(wp * grain_scale);
        let gf = fract(wp * grain_scale);
        let grain_h = hash(gc);
        let grain_h2 = hash(gc + vec2<f32>(7.3, 13.1));
        let grain_h3 = hash(gc + vec2<f32>(31.7, 5.9));
        // Distance to grain center (jittered)
        let grain_center = vec2<f32>(grain_h * 0.6 + 0.2, grain_h2 * 0.6 + 0.2);
        let grain_dist = length(gf - grain_center);
        // Each grain has a rounded top — creates depth
        let grain_bump = 1.0 - smoothstep(0.0, 0.38, grain_dist);
        // Grain-to-grain crevice shadow
        let grain_crevice = smoothstep(0.30, 0.42, grain_dist);

        // Per-grain color variation: warm stone, cool stone, or quartz-white
        let grain_tone = mix(
            mix(stone_warm, stone_cool, step(0.5, grain_h)),
            stone_base * 1.15, // occasional bright quartz grain
            step(0.88, grain_h3)
        );
        var trail = grain_tone
            * (0.85 + 0.15 * grain_bump * close_detail)
            * (1.0 - grain_crevice * 0.12 * close_detail);

        // Normal perturbation from gravel grains — gives 3D depth
        let grain_nx = (hash(gc + vec2<f32>(0.5, 0.0)) - hash(gc - vec2<f32>(0.5, 0.0))) * grain_bump;
        let grain_nz = (hash(gc + vec2<f32>(0.0, 0.5)) - hash(gc - vec2<f32>(0.0, 0.5))) * grain_bump;
        N_perturbed = normalize(N + vec3<f32>(grain_nx, 0.0, grain_nz) * 0.15 * close_detail);

        // ── Multi-scale terrain variation ──
        let broad_var = fbm_n(wp * 0.8, 3);   // large geological patches
        let med_var = noise2d(wp * 4.0);       // puddle/wear zones
        let fine_var = noise2d(wp * 18.0);     // gravel clump variation
        trail *= 0.90 + 0.10 * broad_var;
        trail *= 0.94 + 0.06 * fine_var * mid_detail;

        // ── Dirt patches and clay showing through ──
        let dirt_mask = smoothstep(0.42, 0.62, fbm_n(wp * 1.8, 3));
        trail = mix(trail, dirt_color * (0.88 + 0.12 * fine_var), dirt_mask * 0.40);
        // Occasional reddish clay
        let clay_mask = smoothstep(0.68, 0.78, noise2d(wp * 2.5 + 100.0));
        trail = mix(trail, clay_color, clay_mask * 0.20);

        // ── Tire tracks — two worn paths with real depth ──
        let track_l = exp(-pow((ux - 0.30) / 0.07, 2.0));
        let track_r = exp(-pow((ux - 0.70) / 0.07, 2.0));
        let track = max(track_l, track_r);
        // Compacted surface: smoother grains, darker, slightly shinier from wear
        let compacted_color = stone_base * vec3<f32>(0.78, 0.77, 0.76);
        trail = mix(trail, compacted_color, track * 0.45);
        // Flatten grain bumps in tracks (smoother surface)
        N_perturbed = mix(N_perturbed, N, track * 0.6);

        // Tire tread marks — subtle wavy parallel lines
        let tread1 = sin(road_dist * 12.0 + noise2d(wp * 3.0) * 3.0) * 0.5 + 0.5;
        let tread2 = sin(road_dist * 12.0 + 1.5 + noise2d(wp * 3.0 + 5.0) * 3.0) * 0.5 + 0.5;
        trail -= vec3<f32>(tread1 * track_l * 0.05 * mid_detail);
        trail -= vec3<f32>(tread2 * track_r * 0.05 * mid_detail);

        // Track edge — slight berm of displaced gravel
        let berm_l1 = exp(-pow((ux - 0.22) / 0.02, 2.0));
        let berm_l2 = exp(-pow((ux - 0.38) / 0.02, 2.0));
        let berm_r1 = exp(-pow((ux - 0.62) / 0.02, 2.0));
        let berm_r2 = exp(-pow((ux - 0.78) / 0.02, 2.0));
        let berms = max(max(berm_l1, berm_l2), max(berm_r1, berm_r2));
        trail = mix(trail, trail * 1.10, berms * 0.25 * mid_detail);

        // ── Loose gravel center ridge ──
        let center_ridge = exp(-pow((ux - 0.50) / 0.06, 2.0));
        trail = mix(trail, trail * 1.06, center_ridge * 0.25);
        // Ridge has slightly raised normal
        N_perturbed = normalize(N_perturbed + vec3<f32>(0.0, 0.0, 0.0) +
            N * center_ridge * 0.03 * close_detail);

        // ── Pine needles — multiple scales, visible and 3D ──
        // Large needles (individual, clearly visible)
        let ns1 = 0.10; // ~10cm cells for big needles
        let nc1 = floor(wp / ns1);
        let nf1 = fract(wp / ns1);
        let nh1 = hash(nc1);
        let nh1b = hash(nc1 + vec2<f32>(3.7, 11.3));
        let needle1_on = step(0.60, nh1); // 40% of cells
        let na1 = nh1 * 6.28;
        let nc1_c = nf1 - vec2<f32>(nh1b * 0.4 + 0.3, hash(nc1 + 2.0) * 0.4 + 0.3);
        let nr1x = nc1_c.x * cos(na1) - nc1_c.y * sin(na1);
        let nr1y = nc1_c.x * sin(na1) + nc1_c.y * cos(na1);
        // Tapered needle shape — wider in middle, pointed at ends
        let needle1_width = 0.06 * (1.0 - abs(nr1y) / 0.40);
        let needle1_body = step(abs(nr1x), max(needle1_width, 0.0)) * step(abs(nr1y), 0.40);
        let needle1_vis = needle1_on * needle1_body;

        // Needle color varies — some dry/orange, some dark brown
        let n1_col = mix(needle_dark, needle_light, nh1b);
        // Needle shadow — shifted slightly in light direction
        let shadow_offset = vec2<f32>(0.15, 0.10);
        let nc1_s = nf1 - vec2<f32>(nh1b * 0.4 + 0.3, hash(nc1 + 2.0) * 0.4 + 0.3) + shadow_offset * 0.3;
        let ns1x = nc1_s.x * cos(na1) - nc1_s.y * sin(na1);
        let ns1y = nc1_s.x * sin(na1) + nc1_s.y * cos(na1);
        let needle1_shadow = step(abs(ns1x), needle1_width * 1.3) * step(abs(ns1y), 0.42)
            * needle1_on * (1.0 - needle1_body); // shadow only where needle isn't

        // Small needle clusters (pine needle litter — groups of 2-3)
        let ns2 = 0.06; // 6cm cells, denser
        let nc2 = floor(wp / ns2);
        let nf2 = fract(wp / ns2);
        let nh2 = hash(nc2 + vec2<f32>(17.0, 23.0));
        let nh2b = hash(nc2 + vec2<f32>(41.0, 7.0));
        let needle2_on = step(0.55, nh2); // 45% cells
        let na2 = nh2b * 6.28;
        let nc2_c = nf2 - 0.5;
        let nr2x = nc2_c.x * cos(na2) - nc2_c.y * sin(na2);
        let nr2y = nc2_c.x * sin(na2) + nc2_c.y * cos(na2);
        let needle2_body = step(abs(nr2x), 0.04) * step(abs(nr2y), 0.30);
        let needle2_vis = needle2_on * needle2_body;

        // Density: heavy at edges, lighter in tracks, moderate elsewhere
        let needle_density = mix(1.0, 0.15, track) * (0.4 + 0.6 * smoothstep(0.3, 0.9, edge));

        // Apply needles with depth
        // Shadow first (darkens ground under/beside needle)
        trail *= 1.0 - needle1_shadow * needle_density * 0.15 * mid_detail;
        // Then the needle on top
        let n1_final = n1_col * (0.75 + 0.25 * (0.5 + 0.5 * nr1y / 0.40)); // lighter in middle
        trail = mix(trail, n1_final, needle1_vis * needle_density * 0.90 * mid_detail);
        // Small needle cluster
        let n2_col = mix(needle_dark, needle_light, nh2) * 0.9;
        trail = mix(trail, n2_col, needle2_vis * needle_density * 0.80 * mid_detail);

        // Needle normal perturbation — they sit on top of the surface
        let needle_bump = (needle1_vis + needle2_vis * 0.5) * needle_density * close_detail;
        N_perturbed = normalize(N_perturbed + vec3<f32>(
            cos(na1) * needle1_vis + cos(na2) * needle2_vis * 0.5,
            0.0,
            sin(na1) * needle1_vis + sin(na2) * needle2_vis * 0.5
        ) * needle_density * 0.06 * close_detail);

        // ── Pinecones — 3D bumps with texture ──
        let cone_cell = 0.7;
        let cc = floor(wp / cone_cell);
        let cf = fract(wp / cone_cell);
        let cone_h = hash(cc + 42.0);
        let cone_h2 = hash(cc + vec2<f32>(55.0, 13.0));
        let cone_on = step(0.92, cone_h); // ~8% of cells
        let cone_pos = vec2<f32>(cone_h2 * 0.4 + 0.3, hash(cc + 77.0) * 0.4 + 0.3);
        let cone_d = length(cf - cone_pos);
        let cone_r = 0.04 + cone_h * 0.03; // 4-7cm radius
        let cone_body = 1.0 - smoothstep(0.0, cone_r, cone_d);
        let cone_vis = cone_on * cone_body;
        // Scale texture on cone surface
        let cone_scales = sin(cone_d * 120.0) * 0.5 + 0.5;
        let cone_col = cone_color * (0.8 + 0.2 * cone_scales);
        trail = mix(trail, cone_col, cone_vis * 0.85 * mid_detail);
        // Cone casts tiny shadow
        let cone_shadow_d = length(cf - cone_pos + vec2<f32>(0.04, 0.03));
        let cone_shadow = (1.0 - smoothstep(cone_r, cone_r + 0.04, cone_shadow_d)) * cone_on;
        trail *= 1.0 - cone_shadow * 0.12 * (1.0 - cone_body) * mid_detail;
        // Cone bump in normal
        N_perturbed = normalize(N_perturbed +
            vec3<f32>(cf.x - cone_pos.x, 0.0, cf.y - cone_pos.y) * cone_vis * 0.20 * close_detail);

        // ── Edge transition — thick organic debris ──
        let edge_debris = smoothstep(0.70, 1.0, edge);
        let debris_fbm = fbm_n(wp * 4.0, 3);
        let debris_fine = noise2d(wp * 25.0);
        let debris_col = mix(
            needle_dark * 0.7,
            vec3<f32>(0.30, 0.20, 0.08),
            debris_fbm
        ) * (0.85 + 0.15 * debris_fine);
        trail = mix(trail, debris_col, edge_debris * 0.70);
        // Debris has lumpy normals
        let debris_nx = (noise2d(wp * 20.0 + vec2<f32>(1.0, 0.0)) - noise2d(wp * 20.0 - vec2<f32>(1.0, 0.0)));
        let debris_nz = (noise2d(wp * 20.0 + vec2<f32>(0.0, 1.0)) - noise2d(wp * 20.0 - vec2<f32>(0.0, 1.0)));
        N_perturbed = normalize(N_perturbed +
            vec3<f32>(debris_nx, 0.0, debris_nz) * edge_debris * 0.08 * mid_detail);

        // ── Moisture variation — damp patches darker/shinier ──
        let damp = smoothstep(0.55, 0.70, noise2d(wp * 1.2 + 200.0));
        trail *= 1.0 - damp * 0.15;

        // Edge darkening
        let edge_darken = 1.0 - edge * edge * 0.35;
        base_color = trail * edge_darken;

        // Gravel is mostly matte, but damp spots and tracks have slight sheen
        surface_spec = 8.0 + track * 16.0 + damp * 20.0;
        spec_strength = 0.01 + track * 0.02 + damp * 0.03;

    } else {
        // ── Paved asphalt surface — aggregate texture + normal perturbation ──
        let wp = in.world_pos.xz;
        let cam_dist = length(camera.eye_pos.xyz - in.world_pos);
        let pave_detail = 1.0 - smoothstep(5.0, 20.0, cam_dist);

        // Base noise — boosted contrast
        let n1 = noise2d(wp * 2.0);
        let n2 = noise2d(wp * 8.0);
        var paved = material.base_color.xyz
            * (0.82 + 0.18 * n1)
            * (0.90 + 0.10 * n2);

        // Asphalt aggregate pattern — hash-based stone chips at ~2.5cm scale
        let agg_scale = 40.0; // ~2.5cm cells
        let agg_cell = floor(wp * agg_scale);
        let agg_f = fract(wp * agg_scale);
        let agg_h = hash(agg_cell);
        let agg_h2 = hash(agg_cell + vec2<f32>(17.3, 31.1));
        // Per-chip color: lighter or darker stones in aggregate mix
        let chip_tone = 0.88 + agg_h * 0.24; // 0.88-1.12 range
        paved *= mix(1.0, chip_tone, pave_detail * 0.5);
        // Chip bump: small rounded bumps for each stone in the aggregate
        let chip_center = vec2<f32>(agg_h * 0.4 + 0.3, agg_h2 * 0.4 + 0.3);
        let chip_dist = length(agg_f - chip_center);
        let chip_bump = 1.0 - smoothstep(0.0, 0.30, chip_dist);
        // Aggregate gap shadow (bitumen between stones)
        let gap_dark = smoothstep(0.28, 0.38, chip_dist);
        paved *= 1.0 - gap_dark * 0.06 * pave_detail;

        // Aggregate normal perturbation — visible bumpy surface under lighting
        let agg_nx = (hash(agg_cell + vec2<f32>(0.5, 0.0)) - hash(agg_cell - vec2<f32>(0.5, 0.0))) * chip_bump;
        let agg_nz = (hash(agg_cell + vec2<f32>(0.0, 0.5)) - hash(agg_cell - vec2<f32>(0.0, 0.5))) * chip_bump;
        N_perturbed = normalize(N + vec3<f32>(agg_nx, 0.0, agg_nz) * 0.10 * pave_detail);

        // Broad surface undulations — patches and repairs
        let broad_n = fbm_n(wp * 0.4, 2);
        N_perturbed = normalize(N_perturbed + vec3<f32>(
            noise2d(wp * 0.8 + vec2<f32>(1.0, 0.0)) - noise2d(wp * 0.8 - vec2<f32>(1.0, 0.0)),
            0.0,
            noise2d(wp * 0.8 + vec2<f32>(0.0, 1.0)) - noise2d(wp * 0.8 - vec2<f32>(0.0, 1.0)),
        ) * 0.04);

        // Wheel track wear darkening at lateral positions ~0.30 and ~0.70
        let wear_l = exp(-pow((ux - 0.30) / 0.08, 2.0));
        let wear_r = exp(-pow((ux - 0.70) / 0.08, 2.0));
        let wear = max(wear_l, wear_r);
        paved *= 1.0 - wear * 0.10;
        // Worn tracks are smoother (less aggregate bump)
        N_perturbed = mix(N_perturbed, N, wear * 0.4);

        let edge_darken = 1.0 - edge * edge * 0.25;
        base_color = paved * edge_darken;

        // European road markings (when zone_params.x > 0.5)
        if (material.zone_params.x > 0.5) {
            let road_dist = in.uv.y;
            let center = smoothstep(0.48, 0.485, ux) * (1.0 - smoothstep(0.515, 0.52, ux));
            let dash = step(0.5, fract(road_dist / 6.0));
            let left_edge = smoothstep(0.02, 0.03, ux) * (1.0 - smoothstep(0.06, 0.07, ux));
            let right_edge = smoothstep(0.93, 0.94, ux) * (1.0 - smoothstep(0.97, 0.98, ux));
            let mark = clamp(center * dash + left_edge + right_edge, 0.0, 1.0);
            base_color = mix(base_color, vec3<f32>(0.92, 0.92, 0.88), mark * 0.85);
        }
    }

    // Lighting — use perturbed normal for both paved and gravel
    let shade_N = N_perturbed;
    let shade_H = normalize(L + V);
    let NdotL = dot(shade_N, L);
    let diff = max(NdotL, 0.0);
    let spec = pow(max(dot(shade_N, shade_H), 0.0), surface_spec);

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, normalize(in.world_normal));

    let ambient = light.ambient.xyz * base_color * 0.6;
    let sun = light.color.xyz * vec3<f32>(1.0, 0.95, 0.85);
    let direct = shadow * (diff * sun * base_color + spec * sun * spec_strength);
    var color = ambient + direct;

    // Exponential atmospheric fog
    color = apply_atmosphere(color, in.world_pos);

    return vec4<f32>(color, 1.0);
}

// ── Grass shell texturing ────────────────────────────────────────
//
// Re-renders the terrain mesh N times at increasing Y offsets.
// @builtin(instance_index) encodes the shell layer — no push constants needed.
// Material uniform is repurposed:
//   base_color.xyz = root color, base_color.w = num_shells
//   mid_color.xyz  = tip color,  mid_color.w  = shell_height
//   high_color.x   = density,    high_color.y = wind_strength
//   high_color.zw  = wind_dir
//   zone_params.xy = fade_start, fade_end

struct GrassVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) shadow_pos: vec3<f32>,
    @location(4) shell_t: f32,
};

@vertex
fn vs_grass_shell(
    in: VertexInput,
    @builtin(instance_index) shell_index: u32,
) -> GrassVertexOutput {
    let num_shells = u32(material.base_color.w);
    let shell_height = material.mid_color.w;
    let wind_strength = material.high_color.y;
    let wind_dir = vec2<f32>(material.high_color.z, material.high_color.w);

    let t = f32(shell_index) / f32(max(num_shells - 1u, 1u));

    // Offset position along normal by shell height
    var pos = in.position + in.normal * shell_height * t;

    // Wind displacement — quadratic so tips move more than roots
    let time = camera.eye_pos.w;
    let wind_phase = dot(in.position.xz, wind_dir) * 2.0 + time * 3.0;
    let wind_offset = sin(wind_phase) * wind_strength * t * t * shell_height;
    pos.x += wind_dir.x * wind_offset;
    pos.z += wind_dir.y * wind_offset;

    var out: GrassVertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(pos, 1.0);
    out.world_pos = pos;
    out.world_normal = in.normal;
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(pos);
    out.shell_t = t;
    return out;
}

@fragment
fn fs_grass_shell(in: GrassVertexOutput) -> @location(0) vec4<f32> {
    let t = in.shell_t;
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);

    // Grass parameters from material uniform
    let root_color = material.base_color.xyz;
    let tip_color = material.mid_color.xyz;
    let density = material.high_color.x;
    let fade_start = material.zone_params.x;
    let fade_end = material.zone_params.y;

    // Steep slope — no grass on cliffs
    if (1.0 - N.y > 0.7) {
        discard;
    }

    // No grass on road surface
    let dist_from_center = abs(in.uv.x - 0.5) * 2.0;
    if (dist_from_center < 0.04) {
        discard;
    }

    // Hash-based blade density pattern
    // Base layers (t≈0) pass all fragments, upper layers are progressively sparse
    let cell = floor(in.world_pos.xz * 80.0);
    let blade_hash = hash(cell);
    if (t > 0.001 && blade_hash < t * t / max(density, 0.01)) {
        discard;
    }

    // Per-blade color variation with dead blade mix
    let blade_var = hash(floor(in.world_pos.xz * 12.0));
    var grass_color = mix(root_color, tip_color, t);
    // 15% of shell cells rendered as straw yellow-brown (dead grass)
    let dead_cell = hash(floor(in.world_pos.xz * 8.0 + vec2<f32>(17.3, 31.7)));
    if (dead_cell > 0.85) {
        let straw = mix(vec3<f32>(0.45, 0.38, 0.18), vec3<f32>(0.60, 0.50, 0.22), t);
        grass_color = straw;
    }
    grass_color *= 0.85 + blade_var * 0.3;

    // Self-shadowing — base is slightly darker (occluded by blades above)
    grass_color *= mix(0.75, 1.0, t);

    // Canopy proxy — darken under trees (further from path)
    let canopy = smoothstep(0.15, 0.5, dist_from_center);
    grass_color *= 1.0 - canopy * 0.12;

    // Lighting (diffuse + wrap, no specular for grass)
    let NdotL = dot(N, L);
    let diff = max(NdotL, 0.0);
    let wrap_diff = max(NdotL * 0.5 + 0.5, 0.0);
    let shadow = shadow_factor_biased(in.shadow_pos, N);

    let ambient = light.ambient.xyz * grass_color * (0.55 + 0.45 * wrap_diff);
    let sun = light.color.xyz * vec3<f32>(1.0, 0.95, 0.85);
    let direct = shadow * diff * sun * grass_color;
    var color = ambient + direct;

    // Atmospheric perspective
    color = apply_atmosphere(color, in.world_pos);

    // Distance fade
    let dist = length(camera.eye_pos.xyz - in.world_pos);
    let fade = 1.0 - smoothstep(fade_start, fade_end, dist);

    // Base shells fade in to preserve terrain splatting underneath
    let base_fade = smoothstep(0.0, 0.15, t);

    // Per-shell opacity — controlled so overlapping shells accumulate naturally
    let shell_alpha = 0.55;

    return vec4<f32>(color, fade * base_fade * shell_alpha);
}

// ── Instanced grass blade rendering ──────────────────────────────
//
// Individual grass blade geometry (7 verts, 5 tris) instanced per-blade.
// Instance data: position, height, rotation_y, lean_angle, lean_direction, color_variation
// Material uniform:
//   base_color.xyz = root color
//   mid_color.xyz  = tip color
//   high_color.x   = wind_strength
//   high_color.zw  = wind_dir
//   zone_params.xy = fade_start, fade_end

struct GrassBladeInput {
    @location(3) blade_pos: vec3<f32>,
    @location(4) blade_height: f32,
    @location(5) blade_rot_y: f32,
    @location(6) blade_lean: f32,
    @location(7) blade_lean_dir: f32,
    @location(8) blade_color_var: f32,
};

struct GrassBladeOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) shadow_pos: vec3<f32>,
    @location(4) blade_t: f32,
    @location(5) color_var: f32,
};

@vertex
fn vs_grass_blade(in: VertexInput, blade: GrassBladeInput) -> GrassBladeOutput {
    let time = camera.eye_pos.w;
    let wind_strength = material.high_color.x;
    let wind_dir = normalize(vec2<f32>(material.high_color.z, material.high_color.w));

    var local = in.position;
    let t = local.y; // 0=root, 1=tip (from blade mesh)

    // Scale by blade height
    local.y *= blade.blade_height;

    // Static lean
    let lean_cos = cos(blade.blade_lean_dir);
    let lean_sin = sin(blade.blade_lean_dir);
    let lean_x = lean_sin * blade.blade_lean * t;
    let lean_z = lean_cos * blade.blade_lean * t;
    local.x += lean_x * blade.blade_height;
    local.z += lean_z * blade.blade_height;

    // Wind bend — quadratic (tips move ~4x more than midpoints), boosted 1.5x
    let wind_phase = dot(blade.blade_pos.xz, wind_dir * 0.8) + time * 2.5;
    let gust = sin(time * 0.7 + blade.blade_pos.x * 0.1) * 0.3 + 0.7;
    // Secondary gust wave at different frequency for natural variation
    let gust2 = sin(time * 1.3 + blade.blade_pos.z * 0.15 + 2.7) * 0.2 + 0.8;
    let wind_bend = sin(wind_phase) * wind_strength * 1.5 * t * t * gust * gust2;
    local.x += wind_dir.x * wind_bend * blade.blade_height;
    local.z += wind_dir.y * wind_bend * blade.blade_height;

    // Length preservation: compress Y when bent
    let bend_amount = abs(wind_bend) + length(vec2<f32>(lean_x, lean_z)) * blade.blade_lean;
    local.y *= 1.0 - bend_amount * 0.15 * t;

    // Y-axis rotation
    let cos_r = cos(blade.blade_rot_y);
    let sin_r = sin(blade.blade_rot_y);
    let rx = local.x * cos_r + local.z * sin_r;
    let rz = -local.x * sin_r + local.z * cos_r;

    // World position
    let wp = vec3<f32>(rx + blade.blade_pos.x, local.y + blade.blade_pos.y, rz + blade.blade_pos.z);

    // Distance cull — move off screen if too far
    let dist = length(camera.eye_pos.xyz - wp);
    let fade_end = material.zone_params.y;
    var clip = camera.view_proj * vec4<f32>(wp, 1.0);
    if (dist > fade_end + 5.0) {
        clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    }

    // Blade normal — face camera for correct lighting on thin geometry
    let to_cam = normalize(camera.eye_pos.xyz - wp);
    let blade_normal = normalize(vec3<f32>(to_cam.x, 0.3 + t * 0.7, to_cam.z));

    var out: GrassBladeOutput;
    out.clip_position = clip;
    out.world_pos = wp;
    out.world_normal = blade_normal;
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(wp);
    out.blade_t = t;
    out.color_var = blade.blade_color_var;
    return out;
}

@fragment
fn fs_grass_blade(in: GrassBladeOutput) -> @location(0) vec4<f32> {
    let t = in.blade_t;
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let color_var = in.color_var;

    // Color gradient from root to tip
    let root_color = material.base_color.xyz;
    let tip_color = material.mid_color.xyz;
    var blade_color: vec3<f32>;

    if (color_var >= 0.95) {
        // Fallen leaf — brown/red/yellow with vein-like pattern
        let leaf_h = hash(floor(in.world_pos.xz * 3.0));
        let leaf_brown = vec3<f32>(0.42, 0.28, 0.10);
        let leaf_red = vec3<f32>(0.55, 0.18, 0.08);
        let leaf_yellow = vec3<f32>(0.65, 0.52, 0.12);
        blade_color = mix(leaf_brown, mix(leaf_red, leaf_yellow, leaf_h), fract(leaf_h * 7.0));
        // Vein pattern
        let vein = abs(sin(in.world_pos.x * 40.0 + in.world_pos.z * 20.0));
        blade_color *= 0.85 + vein * 0.15;
    } else if (color_var >= 0.85) {
        // Dead/dry grass — straw yellow-brown
        let straw_root = vec3<f32>(0.45, 0.38, 0.18);
        let straw_tip = vec3<f32>(0.62, 0.52, 0.22);
        blade_color = mix(straw_root, straw_tip, t);
        blade_color *= 0.88 + hash(floor(in.world_pos.xz * 5.0)) * 0.24;
    } else if (color_var >= 0.70) {
        // Weed — taller, darker green
        let weed_root = root_color * vec3<f32>(0.70, 0.75, 0.65);
        let weed_tip = vec3<f32>(0.22, 0.40, 0.10);
        blade_color = mix(weed_root, weed_tip, t);
        blade_color *= 0.85 + hash(floor(in.world_pos.xz * 4.0)) * 0.20;
    } else {
        // Normal grass
        blade_color = mix(root_color, tip_color, t);
        // Per-blade color variation (use color_var within 0-0.69 range scaled up)
        blade_color *= 0.85 + (color_var / 0.70) * 0.30;
    }

    // Self-shadowing / AO — base is darker
    blade_color *= mix(0.55, 1.0, t);

    // Diffuse lighting
    let NdotL = dot(N, L);
    let diff = max(NdotL, 0.0);
    let wrap = max(NdotL * 0.5 + 0.5, 0.0);

    // Subsurface translucency — warm backlit glow
    let back_dot = max(dot(-N, L), 0.0);
    let translucency = back_dot * back_dot * 0.35;
    let sss_color = vec3<f32>(0.45, 0.55, 0.15);

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, N);

    // Combine lighting
    let ambient = light.ambient.xyz * blade_color * (0.5 + 0.5 * wrap);
    let sun = light.color.xyz * vec3<f32>(1.0, 0.95, 0.85);
    let direct = shadow * diff * sun * blade_color;
    let sss = shadow * translucency * sss_color * blade_color;
    var color = ambient + direct + sss;

    // Atmospheric perspective
    color = apply_atmosphere(color, in.world_pos);

    // Distance fade
    let dist = length(camera.eye_pos.xyz - in.world_pos);
    let fade_start = material.zone_params.x;
    let fade_end = material.zone_params.y;
    let fade = 1.0 - smoothstep(fade_start, fade_end, dist);

    return vec4<f32>(color, fade);
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

// ── Cyclist bone matrix skinning ─────────────────────────────────

// Bone matrices storage buffer — CPU computes skin matrices per cyclist
// Layout: bone_matrices[instance_id * 25 + bone_id]
// UV.x directly encodes bone_id (0-24), matching skeleton.rs constants
@group(3) @binding(0) var<storage, read> bone_matrices: array<mat4x4<f32>>;

struct CyclistInstanceInput {
    @location(3) pos: vec3<f32>,
    @location(4) scale: f32,
    @location(5) forward: vec2<f32>,
    @location(6) pedal_phase: f32,
    @location(7) lean_angle: f32,
};

struct CyclistSolidOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) shadow_pos: vec3<f32>,
    @location(4) bind_pos: vec3<f32>,
};

@vertex
fn vs_cyclist(
    in: VertexInput,
    inst: CyclistInstanceInput,
    @builtin(instance_index) instance_id: u32,
) -> CyclistSolidOutput {
    var out: CyclistSolidOutput;

    // Look up bone matrix from CPU-computed storage buffer
    let bone_id = u32(round(in.uv.x));
    let skin_mat = bone_matrices[instance_id * 25u + bone_id];
    let local_pos = (skin_mat * vec4<f32>(in.position, 1.0)).xyz;
    let local_normal = normalize((skin_mat * vec4<f32>(in.normal, 0.0)).xyz);

    // Roll (lean) around local forward axis (Z), then Y-rotation for heading
    let cos_l = cos(inst.lean_angle);
    let sin_l = sin(inst.lean_angle);
    let lx = local_pos.x * cos_l - local_pos.y * sin_l;
    let ly = local_pos.x * sin_l + local_pos.y * cos_l;
    let lz = local_pos.z;
    let lnx = local_normal.x * cos_l - local_normal.y * sin_l;
    let lny = local_normal.x * sin_l + local_normal.y * cos_l;
    let lnz = local_normal.z;

    // Y-axis rotation to face direction of travel
    let cos_a = inst.forward.y;
    let sin_a = inst.forward.x;
    let rx = lx * cos_a + lz * sin_a;
    let ry = ly;
    let rz = -lx * sin_a + lz * cos_a;
    let rnx = lnx * cos_a + lnz * sin_a;
    let rny = lny;
    let rnz = -lnx * sin_a + lnz * cos_a;

    let wp = vec3<f32>(rx, ry, rz) * inst.scale + inst.pos;
    out.clip_position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp;
    out.world_normal = vec3<f32>(rnx, rny, rnz);
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(wp);
    out.bind_pos = in.position;
    return out;
}

// Shadow cyclist uses @group(1) for bone matrices (2-group shadow layout: [camera, bones])
@group(1) @binding(0) var<storage, read> shadow_bone_matrices: array<mat4x4<f32>>;

@vertex
fn vs_shadow_cyclist(
    in: VertexInput,
    inst: CyclistInstanceInput,
    @builtin(instance_index) instance_id: u32,
) -> @builtin(position) vec4<f32> {
    let bone_id = u32(round(in.uv.x));
    let skin_mat = shadow_bone_matrices[instance_id * 25u + bone_id];
    let local_pos = (skin_mat * vec4<f32>(in.position, 1.0)).xyz;

    // Roll (lean) then Y-rotation
    let cos_l = cos(inst.lean_angle);
    let sin_l = sin(inst.lean_angle);
    let lx = local_pos.x * cos_l - local_pos.y * sin_l;
    let ly = local_pos.x * sin_l + local_pos.y * cos_l;
    let lz = local_pos.z;

    let cos_a = inst.forward.y;
    let sin_a = inst.forward.x;
    let rx = lx * cos_a + lz * sin_a;
    let ry = ly;
    let rz = -lx * sin_a + lz * cos_a;

    let wp = vec3<f32>(rx, ry, rz) * inst.scale + inst.pos;
    return camera.light_vp * vec4<f32>(wp, 1.0);
}

// ── Cyclist fragment shader (procedural surface detail per material type) ────

@fragment
fn fs_cyclist(in: CyclistSolidOutput) -> @location(0) vec4<f32> {
    let bone_id = u32(round(in.uv.x));
    let camera_dist = length(camera.eye_pos.xyz - in.world_pos);

    // Pick base color (per-bone coloring for bike primitives)
    var base = material.base_color.xyz;
    let is_bike = material.terrain_params.x > 0.5;
    if (is_bike) {
        if (bone_id == 20u) {
            // Saddle: bone 20 vertices high up and behind BB — black
            let is_saddle = in.bind_pos.y > 0.90 && in.bind_pos.z < -0.05;
            if (is_saddle) {
                base = vec3<f32>(0.04, 0.04, 0.05);
            } else {
                base = material.mid_color.xyz;
            }
        } else if (bone_id == 21u) {
            base = material.high_color.xyz;
        } else if (bone_id == 22u) {
            base = vec3<f32>(0.02, 0.02, 0.02);
        }
    }

    // Helmet — override to yellow for head bone vertices with light base color
    if (!is_bike && bone_id == 5u) {
        let head_lum = dot(base, vec3<f32>(0.299, 0.587, 0.114));
        if (head_lum > 0.7) {
            base = vec3<f32>(1.0, 0.85, 0.0);
        }
    }

    // Material brightness distinguishes fabric/plastic/metal
    let luminance = dot(base, vec3<f32>(0.299, 0.587, 0.114));

    let N_raw = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    var N = N_raw;
    let detail_fade = 1.0 - smoothstep(6.0, 25.0, camera_dist);

    // Bind-pose cylindrical coordinates for stable procedural textures
    let theta = atan2(in.bind_pos.x, -in.bind_pos.z);
    let body_y = in.bind_pos.y;

    if (!is_bike) {
        if (luminance < 0.3) {
            // Dark fabric (shorts, shoes) — lycra knit weave
            let weave_scale = 100.0;
            let wu = theta * weave_scale / 6.28318;
            let wv = body_y * weave_scale;

            // Knit pattern: offset rows create interlocking loops
            let row_offset = step(0.5, fract(wv * 0.5)) * 0.5;
            let knit = sin((wu + row_offset) * 6.28318) * sin(wv * 6.28318);

            // Normal perturbation for fabric texture
            let fab_nx = cos((wu + row_offset) * 6.28318) * 0.08 * detail_fade;
            let fab_ny = cos(wv * 6.28318) * 0.04 * detail_fade;
            N = normalize(N + vec3<f32>(fab_nx, fab_ny, 0.0));

            // Color variation from weave structure
            base *= 0.88 + knit * 0.15 * detail_fade;

            // Seam lines at sides and inseam
            let side_seam = exp(-pow((abs(theta) - 1.57) / 0.04, 2.0));
            let inner_seam = exp(-pow(theta / 0.04, 2.0));
            base *= 1.0 - (side_seam + inner_seam * 0.5) * 0.25 * detail_fade;

            // Stitch dots along seams — periodic dark dots at ~60/m spacing
            let stitch_freq = body_y * 60.0;
            let stitch_dot = step(0.85, fract(stitch_freq));
            let seam_mask = max(side_seam, inner_seam * 0.5);
            base *= 1.0 - stitch_dot * seam_mask * 0.35 * detail_fade;

            // Chamois pad hint (subtle in inner shorts front)
            let chamois_y = smoothstep(0.42, 0.48, body_y) * (1.0 - smoothstep(0.58, 0.64, body_y));
            let chamois_t = exp(-pow(theta / 0.6, 2.0));
            base += vec3<f32>(0.015) * chamois_y * chamois_t * detail_fade;
        } else if (luminance > 0.3 && luminance < 0.7) {
            // Medium-luminance fabric (mid-range kit) — visible weave texture
            let weave_scale = 90.0;
            let wu = theta * weave_scale / 6.28318;
            let wv = body_y * weave_scale;
            let row_offset = step(0.5, fract(wv * 0.5)) * 0.5;
            let knit = sin((wu + row_offset) * 6.28318) * sin(wv * 6.28318);

            let fab_nx = cos((wu + row_offset) * 6.28318) * 0.06 * detail_fade;
            let fab_ny = cos(wv * 6.28318) * 0.03 * detail_fade;
            N = normalize(N + vec3<f32>(fab_nx, fab_ny, 0.0));

            base *= 0.90 + knit * 0.12 * detail_fade;

            // Seam lines
            let side_seam = exp(-pow((abs(theta) - 1.57) / 0.04, 2.0));
            base *= 1.0 - side_seam * 0.20 * detail_fade;

            // Stitch dots
            let stitch_freq = body_y * 60.0;
            let stitch_dot = step(0.85, fract(stitch_freq));
            base *= 1.0 - stitch_dot * side_seam * 0.30 * detail_fade;
        } else if (luminance > 0.7) {
            // Light hard surface (helmet) — glossy moulded shell
            let grain = noise2d(in.bind_pos.xz * 400.0);
            base *= 0.985 + grain * 0.03 * detail_fade;

            // Mold parting line along center
            let mold = exp(-pow(in.bind_pos.x / 0.002, 2.0));
            base *= 1.0 - mold * 0.04 * detail_fade;

            // Vent edge micro-detail
            let vent_noise = noise2d(in.bind_pos.xz * 60.0);
            base *= 0.98 + vent_noise * 0.04 * detail_fade;
        }
    } else {
        // Bike parts — metallic paint / carbon fiber
        let paint = noise2d(in.bind_pos.xz * 300.0);
        let flake = noise2d(in.bind_pos.xz * 800.0);
        base *= 0.97 + paint * 0.05 * detail_fade;

        // Metallic sparkle — view-dependent
        let VdotN = max(dot(V, N_raw), 0.0);
        base += vec3<f32>(flake * 0.03 * VdotN) * detail_fade;
    }

    // Fine overall color noise
    let fine_noise = noise2d(in.bind_pos.xz * 80.0);
    base *= 0.96 + fine_noise * 0.08;

    // Lighting
    let NdotL = max(dot(N, L), 0.0);
    let shadow = shadow_factor_biased(in.shadow_pos, N_raw);
    let diffuse = NdotL * shadow;

    // Material-adaptive specular
    var shininess: f32;
    var spec_strength: f32;
    if (is_bike) {
        shininess = 96.0;
        spec_strength = 0.55;
    } else if (luminance > 0.7) {
        // Helmet — high gloss
        shininess = 128.0;
        spec_strength = 0.65;
    } else {
        // Fabric — matte with subtle lycra sheen
        shininess = 24.0;
        spec_strength = 0.12;
        // Anisotropic-like sheen for lycra stretch direction
        let stretch = normalize(vec3<f32>(0.0, 1.0, 0.0));
        let aniso = pow(max(1.0 - abs(dot(H, stretch)), 0.0), 3.0);
        spec_strength += aniso * 0.25 * detail_fade;
    }
    let spec = pow(max(dot(N, H), 0.0), shininess) * shadow * spec_strength;

    // Rim light — subtle silhouette glow
    let rim = pow(1.0 - max(dot(V, N_raw), 0.0), 3.0) * 0.08;
    let rim_color = light.color.xyz * base * rim;

    var color = base * (light.ambient.xyz + light.color.xyz * diffuse)
              + light.color.xyz * spec + rim_color;
    color = apply_atmosphere(color, in.world_pos);

    return vec4<f32>(color, 1.0);
}

// ── Realistic skin cyclist vertex + fragment shaders ─────────────
//
// Multi-layer procedural skin: pre-integrated SSS, dual-lobe specular,
// translucency, muscle normal perturbation, effort flush, sweat gloss.
// Uses bind-pose position + bone_id for body region identification.
// Layout: [camera, material, shadow, bones] — same as solid cyclist.
//
// Material uniform repurposed for skin:
//   base_color.xyz = base skin tone
//   base_color.w   = effort level (0-1, driven by pedal power)
//   mid_color.xyz  = unused (reserved)
//   mid_color.w    = sweat time accumulator

struct SkinCyclistOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) shadow_pos: vec3<f32>,
    @location(4) bind_pos: vec3<f32>,
};

@vertex
fn vs_skin_cyclist(
    in: VertexInput,
    inst: CyclistInstanceInput,
    @builtin(instance_index) instance_id: u32,
) -> SkinCyclistOutput {
    var out: SkinCyclistOutput;

    // Bone skinning (same as vs_cyclist)
    let bone_id = u32(round(in.uv.x));
    let skin_mat = bone_matrices[instance_id * 25u + bone_id];
    let local_pos = (skin_mat * vec4<f32>(in.position, 1.0)).xyz;
    let local_normal = normalize((skin_mat * vec4<f32>(in.normal, 0.0)).xyz);

    // Roll (lean) around local forward axis (Z), then Y-rotation for heading
    let cos_l = cos(inst.lean_angle);
    let sin_l = sin(inst.lean_angle);
    let lx = local_pos.x * cos_l - local_pos.y * sin_l;
    let ly = local_pos.x * sin_l + local_pos.y * cos_l;
    let lz = local_pos.z;
    let lnx = local_normal.x * cos_l - local_normal.y * sin_l;
    let lny = local_normal.x * sin_l + local_normal.y * cos_l;
    let lnz = local_normal.z;

    // Y-axis rotation to face direction of travel
    let cos_a = inst.forward.y;
    let sin_a = inst.forward.x;
    let rx = lx * cos_a + lz * sin_a;
    let ry = ly;
    let rz = -lx * sin_a + lz * cos_a;
    let rnx = lnx * cos_a + lnz * sin_a;
    let rny = lny;
    let rnz = -lnx * sin_a + lnz * cos_a;

    let wp = vec3<f32>(rx, ry, rz) * inst.scale + inst.pos;
    out.clip_position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp;
    out.world_normal = vec3<f32>(rnx, rny, rnz);
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(wp);
    out.bind_pos = in.position; // pre-skinned position for body region

    return out;
}

// ── Skin helper functions ────────────────────────────────────────

/// Pre-integrated subsurface scattering diffuse.
/// Red scatters furthest (blood), green medium, blue least.
fn preintegrated_sss(ndotl: f32, curvature: f32) -> vec3<f32> {
    let wrap_r = 0.5 + curvature * 0.6;
    let wrap_g = 0.3 + curvature * 0.4;
    let wrap_b = 0.15 + curvature * 0.25;

    let diff_r = max(0.0, (ndotl + wrap_r) / (1.0 + wrap_r));
    let diff_g = max(0.0, (ndotl + wrap_g) / (1.0 + wrap_g));
    let diff_b = max(0.0, (ndotl + wrap_b) / (1.0 + wrap_b));

    return vec3<f32>(pow(diff_r, 0.8), pow(diff_g, 1.0), pow(diff_b, 1.2));
}

/// GGX normal distribution
fn ggx_d(NdotH: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (3.14159 * d * d);
}

/// Smith GGX geometry (combined)
fn smith_g(NdotV: f32, NdotL: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let gv = NdotV + sqrt(a2 + (1.0 - a2) * NdotV * NdotV);
    let gl = NdotL + sqrt(a2 + (1.0 - a2) * NdotL * NdotL);
    return 1.0 / (gv * gl);
}

/// Dual-lobe skin specular: sharp oily surface + broad rough surface
fn skin_specular(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32, sweat: f32) -> vec3<f32> {
    let H = normalize(V + L);
    let NdotH = max(dot(N, H), 0.0);
    let NdotV = max(dot(N, V), 0.001);
    let NdotL = max(dot(N, L), 0.0);
    let VdotH = max(dot(V, H), 0.001);

    // Lobe 1: oily surface layer (narrow, bright)
    let r1 = max(0.05, 0.15 - sweat * 0.1);
    let a1 = r1 * r1;
    let D1 = ggx_d(NdotH, a1);
    let F0 = vec3<f32>(0.028); // skin Fresnel
    let F1 = F0 + (1.0 - F0) * pow(1.0 - VdotH, 5.0);
    let G1 = smith_g(NdotV, NdotL, a1);
    let spec1 = D1 * F1 * G1 / max(4.0 * NdotV * NdotL, 0.001);

    // Lobe 2: rough skin surface (broad, soft)
    let r2 = roughness - sweat * 0.15;
    let a2 = r2 * r2;
    let D2 = ggx_d(NdotH, a2);
    let G2 = smith_g(NdotV, NdotL, a2);
    let spec2 = D2 * G2 / max(4.0 * NdotV * NdotL, 0.001) * vec3<f32>(0.012);

    return spec1 + spec2;
}

/// Backlight translucency — light through thin skin
fn skin_translucency(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, thickness: f32) -> vec3<f32> {
    let back_light = max(0.0, dot(-V, L));
    let through = pow(back_light, 2.0) * (1.0 - thickness);
    let sss_color = vec3<f32>(0.80, 0.15, 0.05);
    return sss_color * through * 0.5;
}

/// Muscle normal perturbation from bind-pose position.
/// Uses cylindrical coordinates to identify quad/calf/hamstring regions.
/// Strengthened for visible muscle definition even at medium camera distance.
fn muscle_normals(bind_pos: vec3<f32>, N: vec3<f32>, effort: f32) -> vec3<f32> {
    let y = bind_pos.y;
    let definition = 0.75 + effort * 0.25;

    // Cylindrical angle around the limb
    let theta = atan2(bind_pos.x, -bind_pos.z);

    var perturb = vec3<f32>(0.0);

    // Thigh region: y ~ 0.30 to 0.65 in bind pose
    if (y > 0.30 && y < 0.65) {
        let t = (y - 0.30) / 0.35;

        // Vastus lateralis (outer quad) — prominent bulge
        let vl = exp(-pow((theta + 1.2) / 0.5, 2.0));
        let vl_taper = smoothstep(0.0, 0.3, t) * (1.0 - smoothstep(0.7, 1.0, t));
        perturb.x += vl * vl_taper * 0.20 * definition;

        // Rectus femoris (front quad) — central ridge
        let rf = exp(-pow(theta / 0.3, 2.0));
        let rf_taper = smoothstep(0.1, 0.4, t) * (1.0 - smoothstep(0.8, 1.0, t));
        perturb.z += rf * rf_taper * 0.14 * definition;

        // Vastus medialis (inner quad, teardrop above knee)
        let vm = exp(-pow((theta - 1.2) / 0.4, 2.0));
        let vm_taper = smoothstep(0.0, 0.1, t) * (1.0 - smoothstep(0.3, 0.5, t));
        perturb.x -= vm * vm_taper * 0.16 * definition;

        // Hamstrings (back) — two ridges at theta ~ +/-2.8
        let ham_l = exp(-pow((theta + 2.5) / 0.3, 2.0));
        let ham_r = exp(-pow((theta - 2.5) / 0.3, 2.0));
        let ham_taper = smoothstep(0.2, 0.5, t) * (1.0 - smoothstep(0.8, 1.0, t));
        perturb.z -= (ham_l + ham_r) * ham_taper * 0.10 * definition;

        // IT band ridge (outer thigh, vertical stripe)
        let itb = exp(-pow((theta + 1.8) / 0.2, 2.0));
        let itb_taper = smoothstep(0.1, 0.3, t) * (1.0 - smoothstep(0.8, 1.0, t));
        perturb.x += itb * itb_taper * 0.06 * definition;

        // Adductor groove (inner thigh)
        let add = exp(-pow((theta - 1.8) / 0.3, 2.0));
        let add_taper = smoothstep(0.3, 0.6, t) * (1.0 - smoothstep(0.8, 1.0, t));
        perturb.x -= add * add_taper * 0.05 * definition;
    }

    // Calf region: y ~ 0.12 to 0.28
    if (y > 0.12 && y < 0.28) {
        let t = (y - 0.12) / 0.16;

        // Gastrocnemius — two-headed calf, bulges at back
        let gastro_med = exp(-pow((theta - 2.8) / 0.4, 2.0));
        let gastro_lat = exp(-pow((theta + 2.8) / 0.3, 2.0));
        let belly = smoothstep(0.2, 0.5, t) * (1.0 - smoothstep(0.7, 1.0, t));
        perturb.z -= (gastro_med * 0.16 + gastro_lat * 0.12) * belly * definition;

        // Soleus — deeper, broader calf muscle
        let soleus = exp(-pow((theta - 3.14) / 0.6, 2.0));
        let soleus_t = smoothstep(0.0, 0.3, t) * (1.0 - smoothstep(0.5, 0.8, t));
        perturb.z -= soleus * soleus_t * 0.06 * definition;

        // Tibialis anterior (front of shin) — sharper ridge
        let tib = exp(-pow(theta / 0.25, 2.0));
        let shin_t = smoothstep(0.1, 0.4, t) * (1.0 - smoothstep(0.7, 0.9, t));
        perturb.z += tib * shin_t * 0.08 * definition;

        // Peroneal muscles (outer calf)
        let peroneal = exp(-pow((theta + 1.5) / 0.3, 2.0));
        let per_t = smoothstep(0.2, 0.5, t) * (1.0 - smoothstep(0.6, 0.9, t));
        perturb.x += peroneal * per_t * 0.05 * definition;
    }

    // Knee tendon depression: y ~ 0.28 to 0.32
    if (y > 0.27 && y < 0.33) {
        let knee_t = 1.0 - smoothstep(0.0, 0.03, abs(y - 0.30));
        // Patellar tendon groove (front)
        let pat_front = exp(-pow(theta / 0.25, 2.0));
        perturb.z -= pat_front * knee_t * 0.04 * definition;
        // Lateral depressions beside kneecap
        let pat_side = exp(-pow((abs(theta) - 0.5) / 0.2, 2.0));
        perturb.x += sign(theta) * pat_side * knee_t * 0.03 * definition;
    }

    return normalize(N + perturb);
}

@fragment
fn fs_skin_cyclist(in: SkinCyclistOutput) -> @location(0) vec4<f32> {
    let bone_id = u32(round(in.uv.x));
    let effort = material.base_color.w;
    let sweat_time = material.mid_color.w;
    let time = camera.eye_pos.w;

    let N_raw = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let camera_dist = length(camera.eye_pos.xyz - in.world_pos);

    // ── Layer 1: Base skin albedo with multi-scale mottling ──
    var skin_color = material.base_color.xyz;

    // Body region variation using bind-pose Y
    let body_y = in.bind_pos.y;
    let theta = atan2(in.bind_pos.x, -in.bind_pos.z);

    // Tan line: legs below shorts line (~0.65) are tanned
    let tan_line = smoothstep(0.60, 0.68, body_y);
    let tan_factor = (1.0 - tan_line) * 0.8;
    skin_color = mix(skin_color, skin_color * vec3<f32>(0.85, 0.73, 0.58), tan_factor * 0.30);

    // Tan gradient — lower leg darker than upper (more sun exposure)
    let leg_tan = smoothstep(0.12, 0.50, body_y) * (1.0 - smoothstep(0.60, 0.68, body_y));
    skin_color = mix(skin_color, skin_color * vec3<f32>(0.92, 0.82, 0.68), leg_tan * 0.12);

    // Knees — rougher, darker, slightly reddened
    let knee_region = exp(-pow((body_y - 0.28) / 0.04, 2.0));
    skin_color = mix(skin_color, skin_color * vec3<f32>(0.88, 0.78, 0.72), knee_region * 0.22);

    // Ankle/shin bony area — slightly different tone
    let shin_region = exp(-pow((body_y - 0.18) / 0.03, 2.0));
    let shin_front = exp(-pow(theta / 0.4, 2.0));
    skin_color = mix(skin_color, skin_color * vec3<f32>(0.95, 0.88, 0.82), shin_region * shin_front * 0.12);

    // Effort flush — legs and face flush red under load
    let flush_mask = smoothstep(0.10, 0.40, body_y) * (1.0 - smoothstep(0.65, 0.80, body_y));
    let flush_color = vec3<f32>(0.85, 0.35, 0.30);
    skin_color = mix(skin_color, flush_color, effort * flush_mask * 0.25);

    // Multi-scale skin color mottling — breaks up flat uniform look
    // Large-scale: broad patches of slightly different tone
    let mottle_broad = fbm_n(in.bind_pos.xz * 15.0, 2);
    skin_color *= 0.93 + mottle_broad * 0.14;

    // Medium-scale: freckle/spot clusters
    let mottle_med = noise2d(in.bind_pos.xz * 60.0);
    let freckle = smoothstep(0.72, 0.82, mottle_med);
    skin_color = mix(skin_color, skin_color * vec3<f32>(0.82, 0.72, 0.62), freckle * 0.16);

    // Fine-scale: subtle skin grain visible at close range
    let skin_fine = noise2d(in.bind_pos.xz * 80.0);
    let fine_fade = 1.0 - smoothstep(5.0, 12.0, camera_dist);
    skin_color *= 0.96 + skin_fine * 0.08 * fine_fade;

    // ── Layer 2: Muscle normal perturbation ──
    var N = muscle_normals(in.bind_pos, N_raw, effort);

    // ── Layer 2b: Skin micro-normals (pores, follicles, leg hair) ──
    let micro_fade = 1.0 - smoothstep(5.0, 15.0, camera_dist);
    if (micro_fade > 0.01) {
        // Pore-like micro-bumps — high-frequency noise-derived normal perturbation
        let pore_p = in.bind_pos.xz * 180.0;
        let pore_dx = noise2d(pore_p + vec2<f32>(0.5, 0.0)) - noise2d(pore_p - vec2<f32>(0.5, 0.0));
        let pore_dz = noise2d(pore_p + vec2<f32>(0.0, 0.5)) - noise2d(pore_p - vec2<f32>(0.0, 0.5));
        N = normalize(N + vec3<f32>(pore_dx, 0.0, pore_dz) * 0.10 * micro_fade);

        // Hair follicle bumps on legs (below shorts line)
        if (body_y < 0.62) {
            let follicle_cell = floor(in.bind_pos.xz * 110.0);
            let follicle_hash = hash(follicle_cell);
            let follicle_active = step(0.78, follicle_hash);
            // Outward normal bump where follicle is present
            N = normalize(N + N_raw * follicle_active * 0.06 * micro_fade);

        }
    }

    // ── Layer 3: Curvature estimate for SSS ──
    let dn = fwidth(N_raw);
    let dp = fwidth(in.world_pos);
    let curvature = clamp(length(dn) / max(length(dp), 0.001) * 2.0, 0.0, 1.0);

    // ── Layer 4: Pre-integrated subsurface scattering ──
    let ndotl = dot(N, L);
    let sss_diffuse = preintegrated_sss(ndotl, curvature);
    let diffuse_light = skin_color * sss_diffuse * light.color.xyz;

    // Warm ambient (skin shadow has a warm red-brown tint)
    let ambient = skin_color * light.ambient.xyz * vec3<f32>(1.0, 0.88, 0.78) * 0.6;

    // ── Layer 5: Translucency (backlight through thin skin) ──
    // Thinner on extremities (ears, fingers), thicker on torso
    let thickness = smoothstep(0.10, 0.50, body_y) * 0.7 + 0.3;
    let translucency = skin_translucency(N, V, L, thickness);

    // ── Layer 6: Dual-lobe specular ──
    // Roughness varies: knees rougher, shins smoother
    var roughness = 0.45;
    roughness += knee_region * 0.12;
    roughness -= exp(-pow((body_y - 0.20) / 0.06, 2.0)) * 0.05; // shins slightly smoother

    // Sweat: onset above 50% effort
    let sweat_amount = smoothstep(0.4, 0.85, effort);
    let sweat_gloss = sweat_amount * 0.6;

    let specular = skin_specular(N, V, L, roughness, sweat_gloss);

    // ── Layer 7: Sweat sparkle ──
    // At close range and high effort, tiny specular highlights from droplets
    var sweat_spec = vec3<f32>(0.0);
    if (sweat_amount > 0.01 && camera_dist < 8.0) {
        let drop_cell = floor(in.world_pos.xz * 200.0);
        let drop_hash = hash(drop_cell);
        let drop_active = step(drop_hash * 10.0, sweat_time * sweat_amount);
        let H = normalize(V + L);
        sweat_spec = vec3<f32>(drop_active * sweat_amount * pow(max(dot(N, H), 0.0), 64.0) * 0.8);
        sweat_spec *= 1.0 - smoothstep(4.0, 8.0, camera_dist);
    }

    // ── Shadow ──
    let shadow = shadow_factor_biased(in.shadow_pos, N_raw);

    // ── Combine all layers ──
    var color = ambient
              + shadow * (diffuse_light + specular * light.color.xyz)
              + translucency * shadow * skin_color
              + sweat_spec * shadow * light.color.xyz;

    // Atmosphere
    color = apply_atmosphere(color, in.world_pos);

    return vec4<f32>(color, 1.0);
}

// ── Textured cyclist vertex + fragment shaders ───────────────────
//
// Uses cylindrical UVs computed from bind-pose positions to sample a jersey
// texture. Same bone skinning as vs_cyclist. Layout: [camera, textured_material, shadow, bones].

struct TexturedCyclistOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) shadow_pos: vec3<f32>,
    @location(4) tex_uv: vec2<f32>,
    @location(5) bind_pos: vec3<f32>,
};

@vertex
fn vs_textured_cyclist(
    in: VertexInput,
    inst: CyclistInstanceInput,
    @builtin(instance_index) instance_id: u32,
) -> TexturedCyclistOutput {
    var out: TexturedCyclistOutput;

    // Bone skinning (same as vs_cyclist)
    let bone_id = u32(round(in.uv.x));
    let skin_mat = bone_matrices[instance_id * 25u + bone_id];

    // Back pocket displacement — items stuffed in 3 pockets
    let TAU = 6.28318530718;
    let bp_theta = atan2(in.position.x, -in.position.z); // -PI..PI, 0=front
    let body_y = in.position.y;

    // Pocket zone: back-facing, lower torso
    let pocket_y_center = 0.75;
    let pocket_y_half = 0.08;
    let pocket_y_mask = 1.0 - smoothstep(0.0, 1.0, abs(body_y - pocket_y_center) / pocket_y_half);
    let back_amount = smoothstep(1.8, 2.3, abs(bp_theta));
    let pocket_mask = pocket_y_mask * back_amount;

    // Per-pocket item lumps — each pocket has different stuff in it
    let theta_abs = abs(bp_theta);

    // Left pocket (theta ~ 2.3–2.65): energy gels — two small round lumps
    let lp_theta = smoothstep(2.25, 2.40, theta_abs) * (1.0 - smoothstep(2.55, 2.65, theta_abs));
    let gel1_y = exp(-pow((body_y - 0.73) / 0.02, 2.0));
    let gel2_y = exp(-pow((body_y - 0.77) / 0.02, 2.0));
    let left_lump = lp_theta * max(gel1_y, gel2_y) * 0.025;

    // Center pocket (theta ~ 2.7–3.1): phone — one big rectangular bulge
    let cp_theta = smoothstep(2.68, 2.80, theta_abs) * (1.0 - smoothstep(3.00, 3.10, theta_abs));
    let phone_y = smoothstep(0.69, 0.72, body_y) * (1.0 - smoothstep(0.79, 0.82, body_y));
    let center_lump = cp_theta * phone_y * 0.035;

    // Right pocket (theta on the other side, mirrored via abs): food wrapper — irregular
    // Since we use abs(theta), right pocket mirrors left. Use sign to differentiate.
    let rp_side = step(0.0, bp_theta); // 1 for positive theta side
    let rp_theta = smoothstep(2.25, 2.40, theta_abs) * (1.0 - smoothstep(2.55, 2.65, theta_abs));
    let bar_y = exp(-pow((body_y - 0.74) / 0.03, 2.0));
    let right_lump = rp_theta * rp_side * bar_y * 0.020;
    // Left side gets the gels
    let left_side_lump = lp_theta * (1.0 - rp_side) * max(gel1_y, gel2_y) * 0.025;

    let item_bulge = left_side_lump + center_lump + right_lump;

    // Subtle bounce from pedaling on top of static bulge
    let time = camera.eye_pos.w;
    let phase = inst.pedal_phase;
    let bounce = sin(phase * TAU + time * 4.0) * 0.15 + 1.0;

    // Combined: permanent item bulge + overall pocket sag + bounce
    let pocket_bulge = pocket_mask * 0.012 + item_bulge * bounce;

    // Gravity sag — items pull the pocket bottom down slightly
    let sag_y = smoothstep(0.70, 0.68, body_y) * back_amount * 0.008;

    var displaced_pos = in.position + in.normal * pocket_bulge + vec3<f32>(0.0, -sag_y, 0.0);

    let local_pos = (skin_mat * vec4<f32>(displaced_pos, 1.0)).xyz;
    let local_normal = normalize((skin_mat * vec4<f32>(in.normal, 0.0)).xyz);

    // Roll (lean) around local forward axis (Z), then Y-rotation for heading
    let cos_l = cos(inst.lean_angle);
    let sin_l = sin(inst.lean_angle);
    let lx = local_pos.x * cos_l - local_pos.y * sin_l;
    let ly = local_pos.x * sin_l + local_pos.y * cos_l;
    let lz = local_pos.z;
    let lnx = local_normal.x * cos_l - local_normal.y * sin_l;
    let lny = local_normal.x * sin_l + local_normal.y * cos_l;
    let lnz = local_normal.z;

    // Y-axis rotation to face direction of travel
    let cos_a = inst.forward.y;
    let sin_a = inst.forward.x;
    let rx = lx * cos_a + lz * sin_a;
    let ry = ly;
    let rz = -lx * sin_a + lz * cos_a;
    let rnx = lnx * cos_a + lnz * sin_a;
    let rny = lny;
    let rnz = -lnx * sin_a + lnz * cos_a;

    let wp = vec3<f32>(rx, ry, rz) * inst.scale + inst.pos;
    out.clip_position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp;
    out.world_normal = vec3<f32>(rnx, rny, rnz);
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(wp);
    out.bind_pos = in.position;

    // Procedural cylindrical UV from bind-pose (pre-skinned) position
    let tex_u = atan2(in.position.x, -in.position.z) / TAU + 0.5;
    let tex_v = (in.position.y - 0.65) / 0.45;
    out.tex_uv = vec2<f32>(tex_u, tex_v);

    return out;
}

@fragment
fn fs_textured_cyclist(in: TexturedCyclistOutput) -> @location(0) vec4<f32> {
    let camera_dist = length(camera.eye_pos.xyz - in.world_pos);
    let detail_fade = 1.0 - smoothstep(6.0, 25.0, camera_dist);

    // Sample jersey texture and tint
    let tex_color = textureSample(base_tex, base_samp, in.tex_uv);
    var base = tex_color.xyz * material.base_color.xyz;

    let N_raw = normalize(in.world_normal);
    var N = N_raw;
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    // Fabric weave micro-texture — jersey knit pattern
    let weave_freq = 90.0;
    let wu = in.tex_uv.x * weave_freq;
    let wv = in.tex_uv.y * weave_freq;
    let row_shift = step(0.5, fract(wv * 0.5)) * 0.5;
    let knit_u = sin((wu + row_shift) * 6.28318);
    let knit_v = sin(wv * 6.28318);
    let weave = knit_u * knit_v;

    // Normal perturbation from fabric weave
    let fab_nx = cos((wu + row_shift) * 6.28318) * 0.08 * detail_fade;
    let fab_nz = cos(wv * 6.28318) * 0.04 * detail_fade;
    N = normalize(N + vec3<f32>(fab_nx, 0.0, fab_nz));

    // Fabric color variation from weave structure
    base *= 0.88 + weave * 0.15 * detail_fade;

    // Seam lines — shoulder seams, side seams
    let shoulder_seam = exp(-pow((in.tex_uv.y - 0.85) / 0.02, 2.0));
    let side_seam_l = exp(-pow(in.tex_uv.x / 0.03, 2.0));
    let side_seam_r = exp(-pow((in.tex_uv.x - 1.0) / 0.03, 2.0));
    let seam = max(shoulder_seam, max(side_seam_l, side_seam_r));
    base *= 1.0 - seam * 0.25 * detail_fade;

    // Stitch dots along seams — periodic dark dots at ~60/m spacing
    let stitch_freq_t = in.bind_pos.y * 60.0;
    let stitch_dot_t = step(0.85, fract(stitch_freq_t));
    base *= 1.0 - stitch_dot_t * seam * 0.35 * detail_fade;

    // Back pocket details — 3 pockets stuffed with items
    let bp_theta = atan2(in.bind_pos.x, -in.bind_pos.z);
    let bp_y = in.bind_pos.y;
    let bp_theta_abs = abs(bp_theta);
    let is_back = smoothstep(1.8, 2.3, bp_theta_abs);
    let pocket_zone_y = 1.0 - smoothstep(0.0, 1.0, abs(bp_y - 0.75) / 0.08);
    let in_pocket = is_back * pocket_zone_y;

    if (in_pocket > 0.01) {
        // Pocket opening seam (top edge) — darker stitch line with elastic gather
        let pocket_top_seam = exp(-pow((bp_y - 0.82) / 0.004, 2.0)) * is_back;
        base *= 1.0 - pocket_top_seam * 0.20 * detail_fade;

        // Elastic gather ripple at opening
        let gather = sin(bp_theta * 40.0) * 0.5 + 0.5;
        let gather_line = exp(-pow((bp_y - 0.82) / 0.008, 2.0)) * is_back;
        base *= 1.0 - gather * gather_line * 0.08 * detail_fade;

        // Vertical divider seams between 3 pockets
        let divider1 = exp(-pow((bp_theta_abs - 2.65) / 0.025, 2.0));
        let divider2 = exp(-pow((bp_theta_abs - 3.10) / 0.025, 2.0));
        let pocket_dividers = max(divider1, divider2) * pocket_zone_y;
        base *= 1.0 - pocket_dividers * 0.18 * detail_fade;

        // Item outlines visible through stretched fabric
        let bp_side = step(0.0, bp_theta);

        // Left pocket: energy gel outlines (two bumps)
        let lp_mask = smoothstep(2.25, 2.40, bp_theta_abs) * (1.0 - smoothstep(2.55, 2.65, bp_theta_abs));
        let gel1 = exp(-pow((bp_y - 0.73) / 0.015, 2.0)) * lp_mask * (1.0 - bp_side);
        let gel2 = exp(-pow((bp_y - 0.77) / 0.015, 2.0)) * lp_mask * (1.0 - bp_side);
        let gel_outline = max(gel1, gel2);
        // Fabric stretches lighter over item tops, darker in creases around them
        base *= 1.0 + gel_outline * 0.06 * detail_fade;
        let gel_edge1 = exp(-pow((bp_y - 0.71) / 0.006, 2.0)) * lp_mask * (1.0 - bp_side);
        let gel_edge2 = exp(-pow((bp_y - 0.79) / 0.006, 2.0)) * lp_mask * (1.0 - bp_side);
        base *= 1.0 - max(gel_edge1, gel_edge2) * 0.10 * detail_fade;

        // Center pocket: phone rectangle outline
        let cp_mask = smoothstep(2.68, 2.80, bp_theta_abs) * (1.0 - smoothstep(3.00, 3.10, bp_theta_abs));
        let phone_top = exp(-pow((bp_y - 0.80) / 0.005, 2.0)) * cp_mask;
        let phone_bottom = exp(-pow((bp_y - 0.70) / 0.005, 2.0)) * cp_mask;
        let phone_body = smoothstep(0.70, 0.72, bp_y) * (1.0 - smoothstep(0.79, 0.80, bp_y)) * cp_mask;
        // Phone makes fabric taut and slightly shinier in center
        base *= 1.0 + phone_body * 0.04 * detail_fade;
        // Phone edges create crease shadows
        base *= 1.0 - (phone_top + phone_bottom) * 0.12 * detail_fade;
        // Vertical phone edges
        let phone_left_e = exp(-pow((bp_theta_abs - 2.72) / 0.02, 2.0)) * pocket_zone_y;
        let phone_right_e = exp(-pow((bp_theta_abs - 3.06) / 0.02, 2.0)) * pocket_zone_y;
        base *= 1.0 - max(phone_left_e, phone_right_e) * 0.08 * detail_fade;

        // Right pocket: food bar — wider lump
        let rp_mask = smoothstep(2.25, 2.40, bp_theta_abs) * (1.0 - smoothstep(2.55, 2.65, bp_theta_abs));
        let bar_outline = exp(-pow((bp_y - 0.74) / 0.025, 2.0)) * rp_mask * bp_side;
        base *= 1.0 + bar_outline * 0.05 * detail_fade;
        let bar_edge = exp(-pow((bp_y - 0.71) / 0.006, 2.0)) * rp_mask * bp_side;
        base *= 1.0 - bar_edge * 0.08 * detail_fade;

        // Pocket bottom shadow — fabric sags under weight
        let pocket_bottom_shadow = exp(-pow((bp_y - 0.67) / 0.010, 2.0)) * is_back;
        base *= 1.0 - pocket_bottom_shadow * 0.12 * detail_fade;

        // Fabric tension wrinkles radiating from heavy items
        let wrinkle_freq = bp_theta * 25.0 + bp_y * 40.0;
        let wrinkles = sin(wrinkle_freq) * 0.5 + 0.5;
        let wrinkle_zone = in_pocket * smoothstep(0.69, 0.73, bp_y) * (1.0 - smoothstep(0.78, 0.82, bp_y));
        base *= 1.0 - wrinkles * wrinkle_zone * 0.04 * detail_fade;
    }

    // Subtle fabric wear/pilling variation
    let wear = fbm_n(in.tex_uv * 15.0, 2);
    base *= 0.95 + wear * 0.10;

    // Lighting
    let NdotL = max(dot(N, L), 0.0);
    let shadow = shadow_factor_biased(in.shadow_pos, N_raw);
    let diffuse = NdotL * shadow;

    // Lycra specular — base spec + anisotropic stretch sheen
    let base_spec = pow(max(dot(N, H), 0.0), 32.0) * 0.15;
    let stretch = normalize(vec3<f32>(0.0, 1.0, 0.0));
    let aniso = pow(max(1.0 - abs(dot(H, stretch)), 0.0), 3.0);
    let lycra_sheen = aniso * 0.20 * detail_fade;
    let spec = (base_spec + lycra_sheen) * shadow;

    // Rim light — silhouette definition
    let rim = pow(1.0 - max(dot(V, N_raw), 0.0), 3.0) * 0.06;
    let rim_color = light.color.xyz * base * rim;

    var color = base * (light.ambient.xyz + light.color.xyz * diffuse)
              + light.color.xyz * spec + rim_color;
    color = apply_atmosphere(color, in.world_pos);

    return vec4<f32>(color, 1.0);
}

// ── Textured tree instance vertex + fragment shaders ─────────────

struct TreeInstanceInput {
    @location(3) tree_pos: vec3<f32>,
    @location(4) tree_scale: f32,
    @location(5) tree_rot_y: f32,
    @location(6) tree_flags: f32,
};

@vertex
fn vs_tree_instanced(in: VertexInput, inst: TreeInstanceInput) -> VertexOutput {
    var out: VertexOutput;
    var local = in.position;
    var norm = in.normal;

    // Y-axis rotation
    let cos_r = cos(inst.tree_rot_y);
    let sin_r = sin(inst.tree_rot_y);
    let rx = local.x * cos_r + local.z * sin_r;
    let rz = -local.x * sin_r + local.z * cos_r;
    local.x = rx;
    local.z = rz;
    let rnx = norm.x * cos_r + norm.z * sin_r;
    let rnz = -norm.x * sin_r + norm.z * cos_r;
    norm.x = rnx;
    norm.z = rnz;

    // Fallen tilt (~82 degrees around Z axis) if flags > 0.5
    if (inst.tree_flags > 0.5) {
        let fallen_angle = 1.43; // ~82 degrees
        let cf = cos(fallen_angle);
        let sf = sin(fallen_angle);
        let fy = local.y * cf - local.x * sf;
        let fx = local.y * sf + local.x * cf;
        local.y = fy;
        local.x = fx;
        let fny = norm.y * cf - norm.x * sf;
        let fnx = norm.y * sf + norm.x * cf;
        norm.y = fny;
        norm.x = fnx;
    }

    let wp = local * inst.tree_scale + inst.tree_pos;
    out.clip_position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp;
    out.world_normal = norm;
    out.uv = in.uv;
    out.shadow_pos = world_to_shadow(wp);
    return out;
}

@vertex
fn vs_shadow_tree_inst(in: VertexInput, inst: TreeInstanceInput) -> @builtin(position) vec4<f32> {
    var local = in.position;

    // Y-axis rotation
    let cos_r = cos(inst.tree_rot_y);
    let sin_r = sin(inst.tree_rot_y);
    let rx = local.x * cos_r + local.z * sin_r;
    let rz = -local.x * sin_r + local.z * cos_r;
    local.x = rx;
    local.z = rz;

    // Fallen tilt
    if (inst.tree_flags > 0.5) {
        let fallen_angle = 1.43;
        let cf = cos(fallen_angle);
        let sf = sin(fallen_angle);
        let fy = local.y * cf - local.x * sf;
        let fx = local.y * sf + local.x * cf;
        local.y = fy;
        local.x = fx;
    }

    let wp = local * inst.tree_scale + inst.tree_pos;
    return camera.light_vp * vec4<f32>(wp, 1.0);
}

@fragment
fn fs_textured(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    // Sample texture and tint by material base_color
    let tex_color = textureSample(base_tex, base_samp, in.uv);
    let base_color = tex_color.xyz * material.base_color.xyz;

    // Lighting — diffuse + specular + subsurface translucency
    let NdotL = dot(N, L);
    let diff = max(NdotL, 0.0);
    let wrap_diff = max(NdotL * 0.5 + 0.5, 0.0);
    let spec = pow(max(dot(N, H), 0.0), 64.0);

    // Subsurface translucency for foliage backlight
    let back_light = max(dot(-N, L), 0.0);
    let sss_color = vec3<f32>(0.45, 0.55, 0.1);
    let sss = back_light * back_light * 0.35 * sss_color * base_color;

    // Snow-on-normals (if flagged)
    var final_base = base_color;
    if (material.mid_color.w > 1.5) {
        let snow_color = vec3<f32>(0.94, 0.94, 0.91);
        let snow_t = smoothstep(0.25, 0.65, N.y);
        final_base = mix(final_base, snow_color, snow_t);
    }

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, N);

    // Ground bounce
    let ground_bounce = max(N.y, 0.0) * 0.08;
    let bounce_color = vec3<f32>(0.3, 0.35, 0.1) * final_base * ground_bounce;

    // Combine
    let ambient = light.ambient.xyz * final_base * (0.55 + 0.45 * wrap_diff);
    let sun_warm = light.color.xyz * vec3<f32>(1.0, 0.95, 0.85);
    let direct = shadow * (diff * sun_warm * final_base + spec * sun_warm * 0.06);
    var color = ambient + direct + sss * shadow + bounce_color;

    // Volumetric light shafts
    color += volumetric_light(in.world_pos);

    // Fog
    color = apply_atmosphere(color, in.world_pos);

    // Discard fully transparent texels (alpha cutout for foliage cards)
    if (tex_color.a < 0.1) {
        discard;
    }

    return vec4<f32>(color, 1.0);
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

/// Gerstner wave — returns normal perturbation for a single wave component
fn gerstner_wave(pos: vec2<f32>, dir: vec2<f32>, wavelength: f32, amplitude: f32, time: f32) -> vec3<f32> {
    let k = 6.2832 / wavelength;
    let phase = dot(dir, pos) * k + time * sqrt(9.81 * k);
    let dx = dir.x * amplitude * k * cos(phase);
    let dz = dir.y * amplitude * k * cos(phase);
    return vec3<f32>(-dx, 0.0, -dz);
}

/// Sky gradient for water reflections
fn sample_sky_color(y_reflect: f32) -> vec3<f32> {
    let horizon = vec3<f32>(0.7, 0.75, 0.85);
    let zenith = vec3<f32>(0.3, 0.5, 0.8);
    return mix(horizon, zenith, max(y_reflect, 0.0));
}

@fragment
fn fs_water(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = camera.eye_pos.w;
    let L = normalize(-light.direction.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);

    // Layered Gerstner wave normals — 4 frequency layers for natural ripple pattern
    let flow_dir = normalize(vec2<f32>(0.3, 0.8));  // default current direction

    let w1 = gerstner_wave(in.world_pos.xz, normalize(flow_dir + vec2<f32>(0.3, 0.1)), 5.0, 0.015, time * 0.5);
    let w2 = gerstner_wave(in.world_pos.xz, normalize(flow_dir + vec2<f32>(-0.2, 0.4)), 1.5, 0.008, time * 1.2);
    let w3 = gerstner_wave(in.world_pos.xz, normalize(flow_dir + vec2<f32>(0.1, -0.3)), 0.4, 0.003, time * 2.5);
    let w4 = gerstner_wave(in.world_pos.xz, normalize(vec2<f32>(0.7, 0.7)), 0.1, 0.001, time * 4.0);

    let N = normalize(vec3<f32>(0.0, 1.0, 0.0) + w1 * 1.0 + w2 * 0.6 + w3 * 0.3 + w4 * 0.15);
    let H = normalize(L + V);

    // Fresnel — more reflective at glancing angles
    let fresnel_base = pow(1.0 - max(dot(V, vec3<f32>(0.0, 1.0, 0.0)), 0.0), 4.0);
    let fresnel = mix(0.02, 0.8, fresnel_base);

    // Deep water color with flow variation
    let flow_uv = in.world_pos.xz * 0.15 + vec2<f32>(time * 0.08, time * 0.12);
    let flow = fbm(flow_uv);
    let water_shallow = vec3<f32>(0.10, 0.22, 0.25);
    let water_deep = vec3<f32>(0.04, 0.10, 0.16);
    let water_color = mix(water_deep, water_shallow, flow * 0.4);

    // Sky reflection
    let reflected_view = reflect(-V, N);
    let sky_reflect = sample_sky_color(reflected_view.y);

    // Sun specular sparkle — sharp point highlights on ripple peaks
    let reflect_dir = reflect(-L, N);
    let spec = pow(max(dot(V, reflect_dir), 0.0), 256.0) * 2.0;

    // Shadow
    let shadow = shadow_factor_biased(in.shadow_pos, N);

    // Composite water surface
    let surface = mix(water_color, sky_reflect, fresnel);

    // Lighting
    let NdotL = max(dot(N, L), 0.0);
    let ambient = light.ambient.xyz * water_color * 0.45;
    let direct = shadow * (NdotL * light.color.xyz * surface * 0.6 + spec * light.color.xyz * 0.9);
    var color = ambient + direct + surface * 0.3;

    // Atmospheric perspective
    color = apply_atmosphere(color, in.world_pos);

    // Opaque water — no transparency
    return vec4<f32>(color, 1.0);
}

// ── Emissive fragment shader (cabin windows, no lighting) ───────

@fragment
fn fs_emissive(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = material.base_color.xyz;
    // Fog only, no lighting
    color = apply_atmosphere(color, in.world_pos);
    return vec4<f32>(color, 1.0);
}

// ── Fullscreen vertex shader (sky, post-process, bloom) ─────────
//
// Generates a fullscreen triangle from vertex index alone (no vertex buffer).
// Vertex 0: (-1,-1), Vertex 1: (3,-1), Vertex 2: (-1,3)
// UV coords: (0,1), (2,1), (0,-1) — flipped Y for texture sampling

struct FullscreenOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOutput {
    var out: FullscreenOutput;
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    out.position = vec4<f32>(uv * 2.0 - 1.0, 1.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

// ── Procedural sky ──────────────────────────────────────────────
//
// Bind group 0 for sky pass: sky uniforms only
// (separate pipeline layout from main pass)

struct SkyUniforms {
    inv_view_proj: mat4x4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    sky_zenith: vec4<f32>,    // .w = cloud_coverage
    sky_horizon: vec4<f32>,   // .w = haze_density
};

@group(0) @binding(0) var<uniform> sky: SkyUniforms;

@fragment
fn fs_sky(in: FullscreenOutput) -> @location(0) vec4<f32> {
    // Reconstruct view direction from screen UV via inverse view-projection
    let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0, 1.0, 1.0);
    let world_pos = sky.inv_view_proj * ndc;
    let view_dir = normalize(world_pos.xyz / world_pos.w);

    let sun_dir = normalize(sky.sun_direction.xyz);
    let sun_dot = dot(view_dir, sun_dir);
    let y = view_dir.y;

    // Vertical gradient: zenith → horizon
    let horizon_t = smoothstep(-0.02, 0.3, y);
    var color = mix(sky.sky_horizon.xyz, sky.sky_zenith.xyz, horizon_t);

    // Below horizon: fade to ground color
    let ground_color = sky.sky_horizon.xyz * 0.5;
    let ground_t = smoothstep(0.0, -0.05, y);
    color = mix(color, ground_color, ground_t);

    // Sun disc — bright, HDR
    let sun_disc = smoothstep(0.9995, 0.9999, sun_dot) * 50.0;
    color += sky.sun_color.xyz * sun_disc;

    // Sun glow — broad warm halo
    let sun_glow = pow(max(sun_dot, 0.0), 8.0) * 0.4;
    color += sky.sun_color.xyz * sun_glow;

    // Mie scattering — forward scatter around sun
    let mie = pow(max(sun_dot, 0.0), 64.0) * 0.8;
    color += sky.sun_color.xyz * mie;

    // Horizon haze
    let haze = sky.sky_horizon.w;
    let haze_t = 1.0 - smoothstep(0.0, 0.15, abs(y));
    color = mix(color, sky.sky_horizon.xyz * 1.2, haze_t * haze);

    // Procedural clouds — FBM projected onto y=1 plane
    let cloud_coverage = sky.sky_zenith.w;
    if (y > 0.02 && cloud_coverage > 0.01) {
        let cloud_scale = 0.0004;
        let cloud_uv = view_dir.xz / max(view_dir.y, 0.05) * cloud_scale;
        let cloud_noise = fbm(cloud_uv * 800.0);
        let cloud_shape = smoothstep(0.4 - cloud_coverage * 0.3, 0.6, cloud_noise);
        // Horizon fade — clouds thin out near horizon
        let cloud_horizon_fade = smoothstep(0.02, 0.15, y);
        let cloud_alpha = cloud_shape * cloud_horizon_fade * 0.8;
        // Cloud lit by sun — brighter on sun-facing side
        let cloud_bright = 0.9 + max(sun_dot, 0.0) * 0.3;
        let cloud_color = vec3<f32>(cloud_bright);
        color = mix(color, cloud_color, cloud_alpha);
    }

    return vec4<f32>(color, 1.0);
}

// ── Post-processing ─────────────────────────────────────────────
//
// Reads HDR scene texture + bloom texture, applies tone mapping, vignette, color grading.
// Bind group: [uniforms, scene_texture, scene_sampler, bloom_texture, bloom_sampler]

struct PostProcessUniforms {
    exposure: f32,
    bloom_intensity: f32,
    vignette_intensity: f32,
    saturation: f32,
    color_temperature: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> post: PostProcessUniforms;
@group(0) @binding(1) var scene_tex: texture_2d<f32>;
@group(0) @binding(2) var scene_samp: sampler;
@group(0) @binding(3) var bloom_tex: texture_2d<f32>;
@group(0) @binding(4) var bloom_samp: sampler;

@fragment
fn fs_post_process(in: FullscreenOutput) -> @location(0) vec4<f32> {
    var color = textureSample(scene_tex, scene_samp, in.uv).xyz;
    let bloom = textureSample(bloom_tex, bloom_samp, in.uv).xyz;

    // Apply bloom
    color += bloom * post.bloom_intensity;

    // Exposure
    color *= post.exposure;

    // ACES filmic tone mapping (Narkowicz approximation)
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3(0.0), vec3(1.0));

    // Warm shift
    color = color * vec3<f32>(1.025, 1.0, 0.96);

    // Saturation adjustment
    let lum = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(vec3(lum), color, post.saturation);

    // Vignette
    let center = in.uv - 0.5;
    let vignette = 1.0 - dot(center, center) * post.vignette_intensity * 4.0;
    color *= clamp(vignette, 0.0, 1.0);

    return vec4<f32>(clamp(color, vec3(0.0), vec3(1.0)), 1.0);
}

// ── Bloom passes ────────────────────────────────────────────────
//
// Pass A: Brightness extract + downsample (reads HDR scene, outputs half-res)
// Pass B: Horizontal gaussian blur
// Pass C: Vertical gaussian blur

struct BloomUniforms {
    texel_size: vec2<f32>,
    direction: vec2<f32>,
};

@group(0) @binding(0) var<uniform> bloom: BloomUniforms;
@group(0) @binding(1) var bloom_input_tex: texture_2d<f32>;
@group(0) @binding(2) var bloom_input_samp: sampler;

@fragment
fn fs_bloom_extract(in: FullscreenOutput) -> @location(0) vec4<f32> {
    let color = textureSample(bloom_input_tex, bloom_input_samp, in.uv).xyz;
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    // Soft threshold at 1.0 — smoothly extract bright areas
    let contribution = max(luminance - 1.0, 0.0) / max(luminance, 0.001);
    return vec4<f32>(color * contribution, 1.0);
}

@fragment
fn fs_bloom_blur(in: FullscreenOutput) -> @location(0) vec4<f32> {
    // 9-tap gaussian blur along bloom.direction
    let weights = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    let offset = bloom.texel_size * bloom.direction;

    var result = textureSample(bloom_input_tex, bloom_input_samp, in.uv).xyz * weights[0];
    for (var i = 1; i < 5; i++) {
        let off = offset * f32(i);
        result += textureSample(bloom_input_tex, bloom_input_samp, in.uv + off).xyz * weights[i];
        result += textureSample(bloom_input_tex, bloom_input_samp, in.uv - off).xyz * weights[i];
    }

    return vec4<f32>(result, 1.0);
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
