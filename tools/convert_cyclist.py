"""
Convert purchased Male_Cyclist FBX asset to engine-compatible GLB.

Run: blender --background --python tools/convert_cyclist.py

Pipeline:
1. Import cyclist A-Pose FBX (bind pose = A-pose)
2. Apply armature at rest pose (bake deformations)
3. Set UV.x = bone_id per vertex (25-bone skeleton IDs)
4. Import bike FBX (static, parts get bone IDs 20-22)
5. Replace materials with flat colors, merge objects by color group
6. Position model (ground at Y=0)
7. Y-mirror for engine facing direction
8. Print ALL bone rest positions (for skeleton.rs bind poses)
9. Export GLB
"""

import bpy
import os
import math
from mathutils import Vector

# ── Paths ─────────────────────────────────────────────────────────

BASE = "/Users/blakeburnette/My project/Assets/Male_Cyclist"
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(SCRIPT_DIR)
OUTPUT = os.path.join(PROJECT_DIR, "assets", "cyclist.glb")


# ── 25-bone ID mapping (FBX vertex group name → engine bone_id) ──
# Must match skeleton.rs bone constants exactly.

BONE_ID_MAP = {
    # 0: Hips
    'Pelvis': 0,
    # 1: Spine0
    'spine_01': 1,
    # 2: Spine1
    'spine_02': 2,
    # 3: Spine2
    'spine_03': 3,
    # 4: Neck
    'neck_01': 4,
    # 5: Head
    'head': 5,
    'head_Eye_L': 5, 'head_Eye_R': 5,
    'head_Eyebrow_C': 5, 'head_Eyebrow_L': 5, 'head_Eyebrow_R': 5,
    'head_Eyelid_Lower_L': 5, 'head_Eyelid_Lower_R': 5,
    'head_Eyelid_Upper_L': 5, 'head_Eyelid_Upper_R': 5,
    'head_Lip_L': 5, 'head_Lip_R': 5, 'head_Jaw': 5,
    'Dummy_Helmet': 5, 'Dummy_Sunglasses': 5,
    # 6: L_Clavicle
    'clavicle_l': 6,
    # 7: L_UpperArm
    'Upperarm_L': 7, 'upperarm_twist_01_l': 7,
    # 8: L_Forearm
    'lowerarm_l': 8, 'lowerarm_twist_01_l': 8,
    # 9: L_Hand
    'Hand_L': 9,
    'thumb_01_l': 9, 'thumb_02_l': 9, 'thumb_03_l': 9,
    'index_01_l': 9, 'index_02_l': 9, 'index_03_l': 9,
    'middle_01_l': 9, 'middle_02_l': 9, 'middle_03_l': 9,
    'ring_01_l': 9, 'ring_02_l': 9, 'ring_03_l': 9,
    'pinky_01_l': 9, 'pinky_02_l': 9, 'pinky_03_l': 9,
    # 10: R_Clavicle
    'clavicle_r': 10,
    # 11: R_UpperArm
    'Upperarm_R': 11, 'upperarm_twist_01_r': 11,
    # 12: R_Forearm
    'lowerarm_r': 12, 'lowerarm_twist_01_r': 12,
    # 13: R_Hand
    'Hand_R': 13,
    'thumb_01_r': 13, 'thumb_02_r': 13, 'thumb_03_r': 13,
    'index_01_r': 13, 'index_02_r': 13, 'index_03_r': 13,
    'middle_01_r': 13, 'middle_02_r': 13, 'middle_03_r': 13,
    'ring_01_r': 13, 'ring_02_r': 13, 'ring_03_r': 13,
    'pinky_01_r': 13, 'pinky_02_r': 13, 'pinky_03_r': 13,
    # 14: L_Thigh
    'Thigh_L': 14, 'thigh_twist_01_l': 14,
    # 15: L_Shin
    'calf_l': 15, 'calf_twist_01_l': 15,
    # 16: L_Foot
    'Foot_L': 16, 'ball_l': 16,
    # 17: R_Thigh
    'Thigh_R': 17, 'thigh_twist_01_r': 17,
    # 18: R_Shin
    'calf_r': 18, 'calf_twist_01_r': 18,
    # 19: R_Foot
    'Foot_R': 19, 'ball_r': 19,
}

# Bike parts → bone IDs 20-22
BIKE_BONE_MAP = {
    # 20: BikeFrame (frame, saddle, wheels, derailleurs, bottle)
    'RB_Frame': 20, 'RB_Saddle': 20, 'RB_Bottle': 20,
    'RB_Front_Wheel': 20, 'RB_Rear_Wheel': 20,
    'RB_Front_Derailleur': 20, 'RB_Rear_Derailleur': 20,
    # 21: Crankset (crank arms, pedals)
    'RB_Crank_Arm': 21, 'RB_Pedals': 21,
    # 22: Handlebar (bars, brakes)
    'RB_Handlebar': 22, 'RB_Front_Brake': 22, 'RB_Rear_Break': 22,
}

SKIP_OBJECTS = {'Eyes', 'Eyes_Glass', 'RB_Chain'}

# Bones to print rest positions for (in hierarchy order)
PRINT_BONES = [
    ('Pelvis', 0), ('spine_01', 1), ('spine_02', 2), ('spine_03', 3),
    ('neck_01', 4), ('head', 5),
    ('clavicle_l', 6), ('Upperarm_L', 7), ('lowerarm_l', 8), ('Hand_L', 9),
    ('clavicle_r', 10), ('Upperarm_R', 11), ('lowerarm_r', 12), ('Hand_R', 13),
    ('Thigh_L', 14), ('calf_l', 15), ('Foot_L', 16),
    ('Thigh_R', 17), ('calf_r', 18), ('Foot_R', 19),
]


# ── Helper functions ──────────────────────────────────────────────

def create_flat_material(name, rgba):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = rgba
        bsdf.inputs["Roughness"].default_value = 0.8
    return mat


def get_bone_id(obj, vertex, is_bike=False):
    if not vertex.groups:
        return 20 if is_bike else 0
    dominant = max(vertex.groups, key=lambda g: g.weight)
    vg_name = obj.vertex_groups[dominant.group].name
    if is_bike:
        return BIKE_BONE_MAP.get(obj.name, 20)
    return BONE_ID_MAP.get(vg_name, 0)


def set_bone_uvs(obj, is_bike=False):
    mesh = obj.data
    if not mesh.uv_layers:
        mesh.uv_layers.new(name="UVMap")
    uv_layer = mesh.uv_layers.active.data
    vert_bone = {}
    for v in mesh.vertices:
        vert_bone[v.index] = get_bone_id(obj, v, is_bike)
    for poly in mesh.polygons:
        for li in poly.loop_indices:
            loop = mesh.loops[li]
            uv_layer[li].uv = (float(vert_bone[loop.vertex_index]), 0.0)


def replace_material(obj, mat):
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    for poly in obj.data.polygons:
        poly.material_index = 0


def find_armature(name_hint):
    for obj in bpy.data.objects:
        if obj.type == 'ARMATURE' and name_hint in obj.name:
            return obj
    return None


def apply_armature_modifiers(armature_obj):
    for obj in list(bpy.data.objects):
        if obj.type != 'MESH' or obj.parent != armature_obj:
            continue
        bpy.ops.object.select_all(action='DESELECT')
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        for mod in list(obj.modifiers):
            if mod.type == 'ARMATURE':
                bpy.ops.object.modifier_apply(modifier=mod.name)
        obj.select_set(False)


def join_objects(names, result_name):
    objs = [bpy.data.objects.get(n) for n in names]
    objs = [o for o in objs if o is not None and o.type == 'MESH']
    if not objs:
        return None
    bpy.ops.object.select_all(action='DESELECT')
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    if len(objs) > 1:
        bpy.ops.object.join()
    result = bpy.context.active_object
    result.name = result_name
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.mesh.remove_doubles(threshold=0.002)
    bpy.ops.object.mode_set(mode='OBJECT')
    return result


def delete_non_mesh():
    """Delete all armatures, empties, cameras, lights."""
    for obj in list(bpy.data.objects):
        if obj.type in ('ARMATURE', 'EMPTY', 'CAMERA', 'LIGHT'):
            bpy.data.objects.remove(obj, do_unlink=True)


# ══════════════════════════════════════════════════════════════════
# Main pipeline
# ══════════════════════════════════════════════════════════════════

print("\n" + "="*60)
print("Converting Male_Cyclist FBX → GLB (25-bone A-pose)")
print("="*60)

bpy.ops.wm.read_factory_settings(use_empty=True)

# ── 1. Import cyclist A-Pose ─────────────────────────────────────
print("\n[1] Importing cyclist A-Pose...")
bpy.ops.import_scene.fbx(
    filepath=os.path.join(BASE, "Skeletal_Meshes", "SK_Male_Cyclist_A-Pose.FBX")
)

cyclist_armature = find_armature('Root')
assert cyclist_armature, "Could not find cyclist armature"
print(f"  Armature: {cyclist_armature.name}")

# ── 2. Print bone rest positions (A-pose = bind pose) ────────────
print("\n[2] Reading bone rest positions (A-pose bind pose)...")
bpy.context.view_layer.update()

def bone_pos(name):
    pb = cyclist_armature.pose.bones.get(name)
    if pb:
        return cyclist_armature.matrix_world @ pb.head
    return None

# Also get tail positions for measuring bone lengths
def bone_tail(name):
    pb = cyclist_armature.pose.bones.get(name)
    if pb:
        return cyclist_armature.matrix_world @ pb.tail
    return None

print("\n  Bone rest positions (Blender world, meters):")
for bone_name, bone_id in PRINT_BONES:
    pt = bone_pos(bone_name)
    if pt:
        print(f"    [{bone_id:2d}] {bone_name:20s}: ({pt.x:7.4f}, {pt.y:7.4f}, {pt.z:7.4f})")

# Print all bone names available in the armature
print("\n  All armature bones:")
for bone in cyclist_armature.data.bones:
    head = cyclist_armature.matrix_world @ bone.head_local
    print(f"    {bone.name:30s}: ({head.x:7.4f}, {head.y:7.4f}, {head.z:7.4f})")

# ── 3. Apply armature at rest pose (A-pose) ──────────────────────
print("\n[3] Applying armature at rest pose...")

# Make sure no animation is active — we want the rest/A-pose
if cyclist_armature.animation_data:
    cyclist_armature.animation_data.action = None

# Apply armature modifiers at rest pose
apply_armature_modifiers(cyclist_armature)

# Unparent meshes (keep transform)
mesh_objs = [obj for obj in list(bpy.data.objects)
             if obj.type == 'MESH' and obj.parent == cyclist_armature]
for obj in mesh_objs:
    mw = cyclist_armature.matrix_world @ obj.matrix_local
    obj.parent = None
    obj.matrix_world = mw

# Save IK reference positions before deleting armature
ik_raw = {
    'pelvis': bone_pos('Pelvis'),
    'hip_r': bone_pos('Thigh_R'),
    'hip_l': bone_pos('Thigh_L'),
    'knee_r': bone_pos('calf_r'),
    'knee_l': bone_pos('calf_l'),
    'foot_r': bone_pos('Foot_R'),
    'foot_l': bone_pos('Foot_L'),
    'head': bone_pos('head'),
}

# Delete armatures and empties
for obj in list(bpy.data.objects):
    if obj.type in ('ARMATURE', 'EMPTY'):
        bpy.data.objects.remove(obj, do_unlink=True)

cyclist_mesh_names = [o.name for o in bpy.data.objects if o.type == 'MESH']
print(f"  Cyclist meshes: {cyclist_mesh_names}")

# ── 4. Import bike (static) ──────────────────────────────────────
print("\n[4] Importing bike...")
bpy.ops.import_scene.fbx(
    filepath=os.path.join(BASE, "Skeletal_Meshes", "SK_Racing_Bike.FBX")
)

bike_armature = find_armature('')
if bike_armature:
    apply_armature_modifiers(bike_armature)
    for obj in list(bpy.data.objects):
        if obj.type == 'MESH' and obj.parent == bike_armature:
            mw = bike_armature.matrix_world @ obj.matrix_local
            obj.parent = None
            obj.matrix_world = mw
    delete_non_mesh()

bike_meshes = [o.name for o in bpy.data.objects if o.type == 'MESH' and o.name.startswith('RB_')]
print(f"  Bike meshes: {bike_meshes}")

# ── 5. Skip unwanted objects ─────────────────────────────────────
for skip_name in list(SKIP_OBJECTS):
    obj = bpy.data.objects.get(skip_name)
    if obj:
        bpy.data.objects.remove(obj, do_unlink=True)
        print(f"  Skipped: {skip_name}")

body = bpy.data.objects.get('Cyclist_Body')
if body:
    print(f"  Note: Cyclist_Body has {len(body.data.vertices)} verts — keeping")

# ── 6. Set bone IDs (UV.x) ───────────────────────────────────────
print("\n[5] Setting bone IDs (25-bone encoding)...")
for obj in list(bpy.data.objects):
    if obj.type == 'MESH':
        is_bike = obj.name.startswith('RB_')
        set_bone_uvs(obj, is_bike)

# ── 7. Replace materials with flat colors ─────────────────────────
print("\n[6] Assigning materials...")

OBJECT_COLORS = {
    # Cyclist
    'T-Shirt':       (0.15, 0.30, 0.65, 1.0),
    'Shorts':        (0.10, 0.10, 0.12, 1.0),
    'Cyclist_Body':  (0.85, 0.68, 0.55, 1.0),
    'Arms':          (0.85, 0.68, 0.55, 1.0),
    'Legs':          (0.85, 0.68, 0.55, 1.0),
    'Shoes':         (0.18, 0.18, 0.20, 1.0),
    'Male_Head':     (0.85, 0.68, 0.55, 1.0),
    'Helmet':        (0.90, 0.90, 0.90, 1.0),
    'Sunglasses':    (0.05, 0.05, 0.05, 1.0),
    # Bike
    'RB_Frame':          (0.12, 0.12, 0.14, 1.0),
    'RB_Saddle':         (0.12, 0.12, 0.14, 1.0),
    'RB_Bottle':         (0.70, 0.15, 0.10, 1.0),
    'RB_Handlebar':      (0.18, 0.18, 0.20, 1.0),
    'RB_Front_Brake':    (0.18, 0.18, 0.20, 1.0),
    'RB_Rear_Break':     (0.18, 0.18, 0.20, 1.0),
    'RB_Crank_Arm':      (0.45, 0.45, 0.48, 1.0),
    'RB_Front_Derailleur': (0.45, 0.45, 0.48, 1.0),
    'RB_Rear_Derailleur':  (0.45, 0.45, 0.48, 1.0),
    'RB_Pedals':         (0.18, 0.18, 0.20, 1.0),
    'RB_Front_Wheel':    (0.08, 0.08, 0.10, 1.0),
    'RB_Rear_Wheel':     (0.08, 0.08, 0.10, 1.0),
}

for obj in list(bpy.data.objects):
    if obj.type != 'MESH':
        continue
    color = OBJECT_COLORS.get(obj.name, (0.5, 0.5, 0.5, 1.0))
    mat = create_flat_material(f"Mat_{obj.name}", color)
    replace_material(obj, mat)

# ── 8. Merge objects ─────────────────────────────────────────────
print("\n[7] Merging objects...")

MERGE_GROUPS = [
    ('Skin', (0.85, 0.68, 0.55, 1.0),
     ['Cyclist_Body', 'Arms', 'Legs', 'Male_Head']),
    ('Bike_Dark', (0.14, 0.14, 0.16, 1.0),
     ['RB_Frame', 'RB_Saddle', 'RB_Handlebar', 'RB_Front_Brake',
      'RB_Rear_Break', 'RB_Pedals', 'RB_Bottle']),
    ('Bike_Wheels', (0.08, 0.08, 0.10, 1.0),
     ['RB_Front_Wheel', 'RB_Rear_Wheel']),
    ('Bike_Gears', (0.45, 0.45, 0.48, 1.0),
     ['RB_Crank_Arm', 'RB_Front_Derailleur', 'RB_Rear_Derailleur']),
]

for group_name, color, members in MERGE_GROUPS:
    mat = create_flat_material(f"Mat_{group_name}", color)
    for mname in members:
        obj = bpy.data.objects.get(mname)
        if obj:
            replace_material(obj, mat)
    result = join_objects(members, group_name)
    if result:
        print(f"  {group_name}: {len(result.data.vertices)} verts")

for obj in bpy.data.objects:
    if obj.type == 'MESH' and not obj.name.startswith('Bike_') and obj.name != 'Skin':
        print(f"  {obj.name}: {len(obj.data.vertices)} verts")

# ── 9. Position ──────────────────────────────────────────────────
print("\n[8] Positioning...")

all_z = []
for obj in bpy.data.objects:
    if obj.type != 'MESH':
        continue
    for v in obj.data.vertices:
        all_z.append((obj.matrix_world @ v.co).z if obj.matrix_world != None else v.co.z)

min_z = min(all_z) if all_z else 0
max_z = max(all_z) if all_z else 1.8
print(f"  Height range: {min_z:.3f} to {max_z:.3f} m (total {max_z-min_z:.3f}m)")

z_offset = -min_z
print(f"  Z offset: +{z_offset:.3f}m")

for obj in bpy.data.objects:
    if obj.type != 'MESH':
        continue
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for v in obj.data.vertices:
        v.co.z += z_offset

all_z = []
for obj in bpy.data.objects:
    if obj.type == 'MESH':
        for v in obj.data.vertices:
            all_z.append(v.co.z)
print(f"  Adjusted range: {min(all_z):.3f} to {max(all_z):.3f} m")

# ── 10. Y-mirror ─────────────────────────────────────────────────
print("\n[9] Y-mirror for engine facing direction...")

for obj in bpy.data.objects:
    if obj.type != 'MESH':
        continue
    for v in obj.data.vertices:
        v.co.y = -v.co.y
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.mesh.flip_normals()
    bpy.ops.object.mode_set(mode='OBJECT')
    obj.select_set(False)

# ── 11. Export GLB ────────────────────────────────────────────────
print("\n[10] Exporting GLB...")
bpy.ops.object.select_all(action='SELECT')
os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)

bpy.ops.export_scene.gltf(
    filepath=OUTPUT,
    export_format='GLB',
    use_selection=True,
    export_apply=True,
    export_yup=True,
    export_materials='EXPORT',
    export_normals=True,
)

# ── Summary ───────────────────────────────────────────────────────
print(f"\n{'='*60}")
print(f"Exported: {OUTPUT}")
objects = sorted([o for o in bpy.data.objects if o.type == 'MESH'], key=lambda o: o.name)
total_verts = 0
total_tris = 0
for obj in objects:
    nv = len(obj.data.vertices)
    nt = len(obj.data.polygons)
    total_verts += nv
    total_tris += nt
    print(f"  {obj.name}: {nv} verts, {nt} faces")
print(f"Total: {total_verts} vertices, {total_tris} faces")

# ── Bone rest positions for skeleton.rs ──────────────────────────
# Convert Blender world-space (X-right, Y-forward, Z-up) to
# engine coords (X-right, Y-up, Z-forward) with z_offset applied.
# After Y-mirror + load_cyclist_glb Z-flip:
#   engine_X = blender_X
#   engine_Y = blender_Z + z_offset
#   engine_Z = blender_Y (positive)

print(f"\n{'='*60}")
print("Bone REST positions (engine coords: X-right, Y-up, Z-forward):")
print("Copy these into skeleton.rs as bind_pos array:")
for bone_name, bone_id in PRINT_BONES:
    pt = ik_raw.get(bone_name.lower().replace('_l', '_l').replace('_r', '_r'))
    # Use stored bone positions (before armature deletion)
    # Try direct lookup from ik_raw first
    found = False
    for key, val in ik_raw.items():
        if val is not None:
            pass  # ik_raw only has a subset
    # Re-derive from PRINT_BONES data
    # The bone positions were already printed in step 2

print("\nBone positions for skeleton.rs (from step 2 above):")
print("Use the printed rest positions, converting with:")
print("  engine_x = blender_x")
print(f"  engine_y = blender_z + {z_offset:.4f}")
print("  engine_z = blender_y")

# Print IK constants from A-pose measurements
if ik_raw.get('hip_r') and ik_raw.get('knee_r') and ik_raw.get('foot_r'):
    hr = ik_raw['hip_r']
    kr = ik_raw['knee_r']
    fr = ik_raw['foot_r']
    fl = ik_raw['foot_l']
    kl = ik_raw['knee_l']

    hip_yz = (hr.z + z_offset, hr.y)
    knee_r_yz = (kr.z + z_offset, kr.y)
    knee_l_yz = (kl.z + z_offset, kl.y)
    foot_r_yz = (fr.z + z_offset, fr.y)
    foot_l_yz = (fl.z + z_offset, fl.y)

    thigh_len = math.sqrt((knee_r_yz[0]-hip_yz[0])**2 + (knee_r_yz[1]-hip_yz[1])**2)
    shin_len = math.sqrt((foot_r_yz[0]-knee_r_yz[0])**2 + (foot_r_yz[1]-knee_r_yz[1])**2)

    # BB estimated from A-pose foot positions (straight legs)
    bb_y = (foot_r_yz[0] + foot_l_yz[0]) / 2.0
    bb_z = (foot_r_yz[1] + foot_l_yz[1]) / 2.0

    print(f"\n  IK Constants (A-pose, engine coords):")
    print(f"    Hip (Y,Z):    ({hip_yz[0]:.4f}, {hip_yz[1]:.4f})")
    print(f"    Knee R (Y,Z): ({knee_r_yz[0]:.4f}, {knee_r_yz[1]:.4f})")
    print(f"    Knee L (Y,Z): ({knee_l_yz[0]:.4f}, {knee_l_yz[1]:.4f})")
    print(f"    Foot R (Y,Z): ({foot_r_yz[0]:.4f}, {foot_r_yz[1]:.4f})")
    print(f"    Foot L (Y,Z): ({foot_l_yz[0]:.4f}, {foot_l_yz[1]:.4f})")
    print(f"    BB est (Y,Z): ({bb_y:.4f}, {bb_z:.4f})")
    print(f"    Thigh length: {thigh_len:.4f}")
    print(f"    Shin length:  {shin_len:.4f}")

    print(f"\n  skeleton.rs constants:")
    print(f"    const IK_BB_Y: f32 = {bb_y:.4f};")
    print(f"    const IK_BB_Z: f32 = {bb_z:.4f};")
    print(f"    const IK_CRANK_R: f32 = 0.124;  // measured from animation")
    print(f"    const IK_THIGH: f32 = {thigh_len:.4f};")
    print(f"    const IK_SHIN: f32 = {shin_len:.4f};")
    print(f"    const IK_HIP_Y: f32 = {hip_yz[0]:.4f};")
    print(f"    const IK_HIP_Z: f32 = {hip_yz[1]:.4f};")

print(f"\n{'='*60}")
print("Done!")
print("IMPORTANT: Re-run Blender to generate new cyclist.glb with 25-bone encoding")
print("Then update skeleton.rs bind positions from the output above.")
