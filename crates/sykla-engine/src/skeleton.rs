use glam::{Mat4, Quat, Vec3, Vec4};
use std::f32::consts::PI;

pub const NUM_BONES: usize = 25;
pub const MAX_CYCLISTS: usize = 32;

// Bone IDs — must match convert_cyclist.py BONE_ID_MAP
pub const HIPS: usize = 0;
pub const SPINE0: usize = 1;
pub const SPINE1: usize = 2;
pub const SPINE2: usize = 3;
pub const NECK: usize = 4;
pub const HEAD: usize = 5;
pub const L_CLAVICLE: usize = 6;
pub const L_UPPER_ARM: usize = 7;
pub const L_FOREARM: usize = 8;
pub const L_HAND: usize = 9;
pub const R_CLAVICLE: usize = 10;
pub const R_UPPER_ARM: usize = 11;
pub const R_FOREARM: usize = 12;
pub const R_HAND: usize = 13;
pub const L_THIGH: usize = 14;
pub const L_SHIN: usize = 15;
pub const L_FOOT: usize = 16;
pub const R_THIGH: usize = 17;
pub const R_SHIN: usize = 18;
pub const R_FOOT: usize = 19;
pub const BIKE_FRAME: usize = 20;
pub const CRANKSET: usize = 21;
pub const HANDLEBAR: usize = 22;

// IK constants — bike geometry (engine coords: Y-up, Z-forward)
// BB position derived from crankset mesh bounding box center.
// Crank radius confirmed: Y extent = 0.182..0.429, R = (0.429-0.182)/2 = 0.124.
const IK_BB_Y: f32 = 0.306;
const IK_BB_Z: f32 = -0.102;
const IK_CRANK_R: f32 = 0.124;
const IK_THIGH: f32 = 0.4476; // from A-pose measurement
const IK_SHIN: f32 = 0.4796;
// Foot-to-pedal offset: ankle joint is 110mm above shoe sole (from model: foot bone Y=0.110, sole Y=0.000).
// Placing the ankle this far above the pedal platform puts the shoe sole ON the pedal.
const FOOT_TO_PEDAL: f32 = 0.110;

// Riding-pose positions — measured from actual mesh geometry:
//   Saddle: center=(0,1.011,-0.321) max_Y=1.083 Z=[-0.494..-0.059]
//   Hoods:  center=(0,1.001, 0.505)
// Hip Y=1.08: sit bones on saddle surface (max_Y=1.083).
// Hip Z=-0.32: rear of saddle where sit bones contact.
const RIDING_HIP: Vec3 = Vec3::new(0.0, 1.08, -0.32);
const RIDING_HIP_L: Vec3 = Vec3::new(0.114, 1.08, -0.32);
const RIDING_HIP_R: Vec3 = Vec3::new(-0.114, 1.08, -0.32);
// Handlebar grip targets — on the hoods (brake lever tops on the bar).
// Handlebar mesh top Y=1.077; hoods sit on top of that.
const GRIP_L: Vec3 = Vec3::new(0.20, 1.09, 0.46);
const GRIP_R: Vec3 = Vec3::new(-0.20, 1.09, 0.46);

/// A-pose bind positions in engine coords (X-right, Y-up, Z-forward).
/// Measured from Blender A-pose export, converted:
///   engine_X = blender_X
///   engine_Y = blender_Z  (height)
///   engine_Z = -blender_Y (forward, due to Y-mirror + export_yup + Z-flip chain)
const BIND_POS: [Vec3; NUM_BONES] = [
    Vec3::new( 0.0000, 1.0324,  0.0128),  //  0: Hips (Pelvis)
    Vec3::new( 0.0000, 1.1381,  0.0127),  //  1: Spine0 (spine_01)
    Vec3::new( 0.0000, 1.3040,  0.0012),  //  2: Spine1 (spine_02)
    Vec3::new( 0.0000, 1.4692, -0.0180),  //  3: Spine2 (spine_03)
    Vec3::new( 0.0000, 1.6316, -0.0295),  //  4: Neck (neck_01)
    Vec3::new( 0.0000, 1.7089,  0.0153),  //  5: Head
    Vec3::new( 0.0460, 1.6326, -0.0488),  //  6: L_Clavicle
    Vec3::new( 0.1542, 1.6098, -0.0507),  //  7: L_UpperArm
    Vec3::new( 0.4106, 1.3926, -0.0659),  //  8: L_Forearm (lowerarm_l)
    Vec3::new( 0.6359, 1.2395, -0.0147),  //  9: L_Hand
    Vec3::new(-0.0460, 1.6326, -0.0488),  // 10: R_Clavicle
    Vec3::new(-0.1542, 1.6098, -0.0507),  // 11: R_UpperArm
    Vec3::new(-0.4106, 1.3926, -0.0659),  // 12: R_Forearm (lowerarm_r)
    Vec3::new(-0.6359, 1.2395, -0.0147),  // 13: R_Hand
    Vec3::new( 0.1140, 1.0324,  0.0128),  // 14: L_Thigh
    Vec3::new( 0.1136, 0.5870, -0.0321),  // 15: L_Shin (calf_l)
    Vec3::new( 0.1133, 0.1099, -0.0809),  // 16: L_Foot
    Vec3::new(-0.1140, 1.0324,  0.0128),  // 17: R_Thigh
    Vec3::new(-0.1136, 0.5870, -0.0321),  // 18: R_Shin (calf_r)
    Vec3::new(-0.1133, 0.1099, -0.0809),  // 19: R_Foot
    Vec3::new( 0.0, 0.542, 0.0),            // 20: BikeFrame (mesh center)
    Vec3::new( 0.0, IK_BB_Y, IK_BB_Z),    // 21: Crankset (at BB)
    Vec3::new( 0.0, 0.660, 0.017),         // 22: Handlebar (mesh center)
    Vec3::ZERO,                             // 23: reserved
    Vec3::ZERO,                             // 24: reserved
];

pub struct SkeletonDef {
    pub bind_world: [Mat4; NUM_BONES],
    pub inverse_bind: [Mat4; NUM_BONES],
}

pub struct SkeletonSystem {
    pub def: SkeletonDef,
    pub gpu_matrices: Vec<[[f32; 4]; 4]>,
}

impl SkeletonDef {
    pub fn cyclist() -> Self {
        // Bind pose = A-pose. Each bone's world transform is just a translation
        // to its bind position (no rotation in A-pose — arms out, legs straight).
        let mut bind_world = [Mat4::IDENTITY; NUM_BONES];
        let mut inverse_bind = [Mat4::IDENTITY; NUM_BONES];

        for i in 0..NUM_BONES {
            bind_world[i] = Mat4::from_translation(BIND_POS[i]);
            inverse_bind[i] = bind_world[i].inverse();
        }

        Self {
            bind_world,
            inverse_bind,
        }
    }
}

impl SkeletonSystem {
    pub fn new(max_cyclists: usize) -> Self {
        let def = SkeletonDef::cyclist();
        let gpu_matrices = vec![Mat4::IDENTITY.to_cols_array_2d(); max_cyclists * NUM_BONES];
        Self { def, gpu_matrices }
    }

    pub fn update(&mut self, cyclist_data: &[crate::cyclist::CyclistInstanceData]) {
        let count = cyclist_data.len().min(self.gpu_matrices.len() / NUM_BONES);
        for i in 0..count {
            let phase = cyclist_data[i].pedal_phase;
            let lean = cyclist_data[i].lean_angle;
            let offset = i * NUM_BONES;
            compute_skin_matrices(
                phase,
                lean,
                &self.def,
                &mut self.gpu_matrices[offset..offset + NUM_BONES],
            );
        }
    }
}

/// Compute skin matrices for one cyclist.
///
/// For each bone: skin_matrix = animated_world * inverse_bind
/// This transforms vertices from bind pose (A-pose) to animated (riding) pose.
fn compute_skin_matrices(
    pedal_phase: f32,
    lean_angle: f32,
    def: &SkeletonDef,
    out: &mut [[[f32; 4]; 4]],
) {
    let mut animated_world = [Mat4::IDENTITY; NUM_BONES];

    // --- Hips: positioned at riding position with full forward lean ---
    // All lean is on the hips bone so that Pelvis-weighted AND spine-weighted
    // vertices lean together (single-bone skinning — no blend weights).
    let hip_lean = 1.05; // ~60° forward lean — aggressive hoods position (long reach frame)
    // Extra lateral lean: rider upper body leans 15% more into turn than the bike
    animated_world[HIPS] = Mat4::from_translation(RIDING_HIP)
        * rotation_x(hip_lean)
        * rotation_z(lean_angle * 0.15);

    // --- Spine chain: follows hips, no additional rotation ---
    // (All lean is already on hips to avoid tearing between Pelvis/spine vertex groups)
    {
        let parent_world = animated_world[HIPS];
        let local_offset = BIND_POS[SPINE0] - BIND_POS[HIPS];
        animated_world[SPINE0] = parent_world * Mat4::from_translation(local_offset);
    }
    {
        let parent_world = animated_world[SPINE0];
        let local_offset = BIND_POS[SPINE1] - BIND_POS[SPINE0];
        animated_world[SPINE1] = parent_world * Mat4::from_translation(local_offset);
    }
    {
        let parent_world = animated_world[SPINE1];
        let local_offset = BIND_POS[SPINE2] - BIND_POS[SPINE1];
        animated_world[SPINE2] = parent_world * Mat4::from_translation(local_offset);
    }

    // --- Neck: compensate upward so head looks at road, turn into corner ---
    {
        let parent_world = animated_world[SPINE2];
        let local_offset = BIND_POS[NECK] - BIND_POS[SPINE2];
        animated_world[NECK] = parent_world
            * Mat4::from_translation(local_offset)
            * rotation_x(-0.75)
            * rotation_y(lean_angle * 0.5); // ~15° head turn at 30° lean
    }

    // --- Head: follows neck ---
    {
        let parent_world = animated_world[NECK];
        let local_offset = BIND_POS[HEAD] - BIND_POS[NECK];
        animated_world[HEAD] = parent_world * Mat4::from_translation(local_offset);
    }

    // --- Arms: reach toward handlebar grips ---
    compute_arm_chain(
        &mut animated_world,
        L_CLAVICLE, L_UPPER_ARM, L_FOREARM, L_HAND,
        SPINE2,
        GRIP_L,
    );
    compute_arm_chain(
        &mut animated_world,
        R_CLAVICLE, R_UPPER_ARM, R_FOREARM, R_HAND,
        SPINE2,
        GRIP_R,
    );

    // --- Pedal coasting in sharp corners ---
    // When lean > ~20°, blend pedal phase toward outside-pedal-down position
    let coast_blend = smoothstep(0.30, 0.40, lean_angle.abs());
    let coast_phase = if lean_angle > 0.0 { PI } else { 0.0 };
    let effective_phase = if coast_blend > 0.001 {
        // Shortest-arc blend to avoid leg sweep through the bike
        let mut diff = coast_phase - pedal_phase;
        if diff > PI { diff -= std::f32::consts::TAU; }
        if diff < -PI { diff += std::f32::consts::TAU; }
        pedal_phase + coast_blend * diff
    } else {
        pedal_phase
    };

    // --- Legs: IK-driven from pedal positions ---
    // Crank angle θ = effective_phase, θ=0 = TDC (right pedal at top).
    // Right leg at θ, left leg at θ+π (opposite).
    compute_leg_ik(&mut animated_world, effective_phase + PI, true);
    compute_leg_ik(&mut animated_world, effective_phase, false);

    // --- Bike parts ---
    animated_world[BIKE_FRAME] = Mat4::from_translation(BIND_POS[BIKE_FRAME]);
    animated_world[CRANKSET] = Mat4::from_translation(BIND_POS[CRANKSET])
        * rotation_x(effective_phase);
    // Handlebar: slight counter-steer into corner
    let steer_angle = lean_angle * 0.17; // ~5° per 30° lean
    animated_world[HANDLEBAR] = Mat4::from_translation(BIND_POS[HANDLEBAR])
        * rotation_y(steer_angle);

    // Reserved
    animated_world[23] = Mat4::IDENTITY;
    animated_world[24] = Mat4::IDENTITY;

    // --- Compute skin matrices: animated_world * inverse_bind ---
    for i in 0..NUM_BONES {
        out[i] = (animated_world[i] * def.inverse_bind[i]).to_cols_array_2d();
    }
}

/// Compute arm chain reaching toward a target (handlebar grip).
///
/// The IK root is the shoulder joint (upper arm origin), NOT the clavicle.
/// The clavicle-to-shoulder offset (~11cm outward) must be preserved or the
/// shoulder area collapses inward.
fn compute_arm_chain(
    animated_world: &mut [Mat4; NUM_BONES],
    clavicle: usize,
    upper_arm: usize,
    forearm: usize,
    hand: usize,
    parent_bone: usize,
    target: Vec3,
) {
    let parent_world = animated_world[parent_bone];

    // Clavicle position (inherits lean from parent)
    let clav_offset = BIND_POS[clavicle] - BIND_POS[parent_bone];
    let clav_leaned = parent_world * Mat4::from_translation(clav_offset);
    let clav_pos = clav_leaned.col(3).truncate();

    // Shoulder joint = upper-arm origin, offset from clavicle through the lean
    let shoulder_offset = BIND_POS[upper_arm] - BIND_POS[clavicle];
    let shoulder_pos = (clav_leaned * shoulder_offset.extend(1.0)).truncate();

    // Rotate clavicle to aim from its bind direction toward the grip target,
    // giving the shoulder mesh proper protraction (reaching forward on the bike).
    let bind_clav_dir = shoulder_offset.normalize();
    let anim_clav_dir = (target - clav_pos).normalize_or_zero();
    let clav_rot = rotation_between_dirs(bind_clav_dir, anim_clav_dir);
    animated_world[clavicle] = Mat4::from_translation(clav_pos) * clav_rot;

    // --- 2-link IK from shoulder joint to grip target ---
    let to_target = target - shoulder_pos;
    let dist = to_target.length();

    let upper_len = (BIND_POS[forearm] - BIND_POS[upper_arm]).length();
    let fore_len = (BIND_POS[hand] - BIND_POS[forearm]).length();
    let total_len = upper_len + fore_len;

    if dist < 0.001 || dist > total_len * 0.99 {
        // Degenerate: point straight at target
        let dir = if dist > 0.001 { to_target / dist } else { Vec3::NEG_Y };
        let bind_upper_dir = (BIND_POS[forearm] - BIND_POS[upper_arm]).normalize();
        let rot = rotation_between_dirs(bind_upper_dir, dir);
        animated_world[upper_arm] = Mat4::from_translation(shoulder_pos) * rot;
        animated_world[forearm] = Mat4::from_translation(shoulder_pos + dir * upper_len) * rot;
        animated_world[hand] = Mat4::from_translation(target);
        return;
    }

    // 2-link IK for elbow
    let d = dist.clamp(0.001, total_len - 0.001);
    let cos_a = ((upper_len * upper_len + d * d - fore_len * fore_len)
        / (2.0 * upper_len * d))
        .clamp(-1.0, 1.0);
    let angle_a = cos_a.acos();

    let forward = to_target / dist;
    // Elbow hint: primarily DOWN with some outward — elbows down and out
    let side = if clavicle == L_CLAVICLE {
        Vec3::new(0.3, -1.0, 0.0).normalize()
    } else {
        Vec3::new(-0.3, -1.0, 0.0).normalize()
    };
    let up = (side - forward * side.dot(forward)).normalize_or_zero();

    let elbow_dir = forward * angle_a.cos() + up * angle_a.sin();
    let elbow_pos = shoulder_pos + elbow_dir * upper_len;

    // Rotation: bind-pose direction → animated direction
    let bind_upper_dir = (BIND_POS[forearm] - BIND_POS[upper_arm]).normalize();
    let anim_upper_dir = (elbow_pos - shoulder_pos).normalize();
    let upper_rot = rotation_between_dirs(bind_upper_dir, anim_upper_dir);
    animated_world[upper_arm] = Mat4::from_translation(shoulder_pos) * upper_rot;

    let bind_fore_dir = (BIND_POS[hand] - BIND_POS[forearm]).normalize();
    let anim_fore_dir = (target - elbow_pos).normalize();
    let fore_rot = rotation_between_dirs(bind_fore_dir, anim_fore_dir);
    animated_world[forearm] = Mat4::from_translation(elbow_pos) * fore_rot;

    animated_world[hand] = Mat4::from_translation(target);
}

/// Compute leg IK for one leg given crank phase.
///
/// Phase convention: θ=0 = TDC (pedal at top, 12 o'clock).
/// Pedal circle: Y = BB_Y + R*cos(θ), Z = BB_Z + R*sin(θ).
/// Ankle target = pedal position + (0, FOOT_TO_PEDAL, 0) so shoe sole rests on pedal.
fn compute_leg_ik(
    animated_world: &mut [Mat4; NUM_BONES],
    phase: f32,
    is_left: bool,
) {
    let (thigh, shin, foot) = if is_left {
        (L_THIGH, L_SHIN, L_FOOT)
    } else {
        (R_THIGH, R_SHIN, R_FOOT)
    };

    let hip_pos = if is_left { RIDING_HIP_L } else { RIDING_HIP_R };

    // Pedal position on crank circle (θ=0 = TDC, forward pedaling = θ increasing)
    let pedal_y = IK_BB_Y + IK_CRANK_R * phase.cos();
    let pedal_z = IK_BB_Z + IK_CRANK_R * phase.sin();

    // Ankle target: shoe sole on pedal, ankle is FOOT_TO_PEDAL above
    let ankle = Vec3::new(hip_pos.x, pedal_y + FOOT_TO_PEDAL, pedal_z);

    // 3D two-bone IK: hip → knee → ankle, knees bend forward (+Z)
    let knee = two_bone_ik_3d(hip_pos, ankle, IK_THIGH, IK_SHIN, Vec3::Z);

    // Thigh: hip → knee
    let bind_thigh_dir = (BIND_POS[shin] - BIND_POS[thigh]).normalize();
    let anim_thigh_dir = (knee - hip_pos).normalize();
    let thigh_rot = rotation_between_dirs(bind_thigh_dir, anim_thigh_dir);
    animated_world[thigh] = Mat4::from_translation(hip_pos) * thigh_rot;

    // Shin: knee → ankle
    let bind_shin_dir = (BIND_POS[foot] - BIND_POS[shin]).normalize();
    let anim_shin_dir = (ankle - knee).normalize();
    let shin_rot = rotation_between_dirs(bind_shin_dir, anim_shin_dir);
    animated_world[shin] = Mat4::from_translation(knee) * shin_rot;

    // Foot at ankle position — no rotation, shoe sits flat on pedal
    animated_world[foot] = Mat4::from_translation(ankle);
}

/// 3D two-bone IK solver. Returns knee position given hip, ankle, bone lengths, and pole vector.
///
/// Uses law of cosines for the hip angle, then rotates from the hip→ankle axis
/// toward the pole direction by that angle to find the knee.
fn two_bone_ik_3d(hip: Vec3, ankle: Vec3, thigh_len: f32, shin_len: f32, pole: Vec3) -> Vec3 {
    let to_ankle = ankle - hip;
    let dist = to_ankle.length().max(0.001);
    let d = dist.clamp(0.001, thigh_len + shin_len - 0.001);

    // Law of cosines: angle at hip between hip→ankle and hip→knee
    let cos_a = ((thigh_len * thigh_len + d * d - shin_len * shin_len)
        / (2.0 * thigh_len * d))
        .clamp(-1.0, 1.0);
    let angle_a = cos_a.acos();

    let forward = to_ankle / dist;

    // Project pole direction onto plane perpendicular to hip→ankle axis
    let pole_on_axis = forward * forward.dot(pole);
    let pole_perp = (pole - pole_on_axis).normalize_or_zero();

    // Knee direction: rotate from hip→ankle axis toward pole by angle_a
    let knee_dir = forward * angle_a.cos() + pole_perp * angle_a.sin();
    hip + knee_dir * thigh_len
}

/// Smoothstep interpolation: 0 for x<=edge0, 1 for x>=edge1, smooth in between.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rotation around Z axis (in XY plane).
fn rotation_z(angle: f32) -> Mat4 {
    let c = angle.cos();
    let s = angle.sin();
    Mat4::from_cols(
        Vec4::new(c, s, 0.0, 0.0),
        Vec4::new(-s, c, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Rotation around Y axis (in XZ plane).
fn rotation_y(angle: f32) -> Mat4 {
    let c = angle.cos();
    let s = angle.sin();
    Mat4::from_cols(
        Vec4::new(c, 0.0, -s, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(s, 0.0, c, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Rotation around X axis (in YZ plane).
fn rotation_x(angle: f32) -> Mat4 {
    let c = angle.cos();
    let s = angle.sin();
    Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, c, s, 0.0),
        Vec4::new(0.0, -s, c, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Rotation matrix from direction `from` to direction `to`.
fn rotation_between_dirs(from: Vec3, to: Vec3) -> Mat4 {
    let from_n = from.normalize_or_zero();
    let to_n = to.normalize_or_zero();
    let dot = from_n.dot(to_n).clamp(-0.9999, 0.9999);
    let cross = from_n.cross(to_n);
    let cross_len = cross.length();
    if cross_len < 0.0001 {
        return if dot > 0.0 { Mat4::IDENTITY } else { rotation_x(PI) };
    }
    Mat4::from_quat(Quat::from_axis_angle(cross / cross_len, dot.acos()))
}
