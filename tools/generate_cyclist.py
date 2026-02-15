"""
Generate a cyclist GLB model using Blender Python API.
Run headlessly: blender --background --python tools/generate_cyclist.py

Outputs: assets/cyclist.glb

Objects are grouped by IK bone ID (not material), with UV.x = bone_id
so the engine shader can animate each part correctly:
  0 = static,  +/-1 = thigh,  +/-2 = shin,  +/-3 = crank/shoe

Coordinate system: Blender Z-up, glTF export converts to Y-up.
"""

import bpy
import math
import os

# ── Cleanup ──────────────────────────────────────────────────────

bpy.ops.wm.read_factory_settings(use_empty=True)

# ── Helper functions ─────────────────────────────────────────────

def new_material(name, color):
    """Create a simple PBR material with base color."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = color
        bsdf.inputs["Roughness"].default_value = 0.8
    return mat

def set_bone_uv(obj, bone_id):
    """Set UV.x = bone_id, UV.y = 0 for all loops in the object."""
    mesh = obj.data
    if not mesh.uv_layers:
        mesh.uv_layers.new(name="UVMap")
    uv_layer = mesh.uv_layers.active.data
    for loop_uv in uv_layer:
        loop_uv.uv = (float(bone_id), 0.0)

def create_limb(name, p0, p1, r0, r1, segments=8, material=None):
    """Create a tapered cylinder (limb) between two points."""
    bpy.ops.mesh.primitive_cone_add(
        vertices=segments,
        radius1=r0,
        radius2=r1,
        depth=1.0,
        location=(0, 0, 0),
    )
    obj = bpy.context.active_object
    obj.name = name

    dx = p1[0] - p0[0]
    dy = p1[1] - p0[1]
    dz = p1[2] - p0[2]
    length = math.sqrt(dx*dx + dy*dy + dz*dz)

    obj.scale = (1, 1, length)
    bpy.context.view_layer.update()
    bpy.ops.object.transform_apply(scale=True)

    direction = (dx/length, dy/length, dz/length) if length > 0.001 else (0, 0, 1)
    up = (0, 0, 1)
    dot = up[0]*direction[0] + up[1]*direction[1] + up[2]*direction[2]
    if abs(dot - 1.0) < 0.001:
        pass
    elif abs(dot + 1.0) < 0.001:
        obj.rotation_euler = (math.pi, 0, 0)
    else:
        cross = (
            up[1]*direction[2] - up[2]*direction[1],
            up[2]*direction[0] - up[0]*direction[2],
            up[0]*direction[1] - up[1]*direction[0],
        )
        cl = math.sqrt(cross[0]**2 + cross[1]**2 + cross[2]**2)
        angle = math.acos(max(-1, min(1, dot)))
        axis = (cross[0]/cl, cross[1]/cl, cross[2]/cl)
        obj.rotation_mode = 'AXIS_ANGLE'
        obj.rotation_axis_angle = (angle, axis[0], axis[1], axis[2])

    bpy.context.view_layer.update()
    bpy.ops.object.transform_apply(rotation=True)

    mid = ((p0[0]+p1[0])/2, (p0[1]+p1[1])/2, (p0[2]+p1[2])/2)
    obj.location = mid
    bpy.context.view_layer.update()
    bpy.ops.object.transform_apply(location=True)

    if material:
        obj.data.materials.clear()
        obj.data.materials.append(material)

    for poly in obj.data.polygons:
        poly.use_smooth = True

    return obj

def create_body_part(name, location, scale, material, subdivisions=2):
    """Create a subdivided and smoothed ellipsoid body part."""
    bpy.ops.mesh.primitive_ico_sphere_add(
        subdivisions=subdivisions,
        radius=1.0,
        location=location,
    )
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = scale
    bpy.context.view_layer.update()
    bpy.ops.object.transform_apply(scale=True)

    obj.data.materials.clear()
    obj.data.materials.append(material)

    for poly in obj.data.polygons:
        poly.use_smooth = True

    return obj

def join_objects(objects, name):
    """Join multiple objects into one, remove doubles."""
    if not objects:
        return None
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.join()
    result = bpy.context.active_object
    result.name = name

    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.mesh.remove_doubles(threshold=0.002)
    bpy.ops.object.mode_set(mode='OBJECT')

    return result


# ── Materials ────────────────────────────────────────────────────

mat_frame   = new_material("Frame",   (0.18, 0.18, 0.20, 1.0))
mat_jersey  = new_material("Jersey",  (0.15, 0.30, 0.65, 1.0))
mat_shorts  = new_material("Shorts",  (0.10, 0.10, 0.12, 1.0))
mat_skin    = new_material("Skin",    (0.85, 0.68, 0.55, 1.0))
mat_helmet  = new_material("Helmet",  (0.90, 0.90, 0.90, 1.0))

# ── Reference points (Blender coords: X-right, Y-forward, Z-up) ──

BB      = (0.0, 0.0, 0.30)
HIP_C   = (0.0, -0.06, 0.90)
SHOULDER = (0.0, 0.20, 1.26)
WAIST    = (0.0, 0.06, 1.04)
HEAD     = (0.0, 0.28, 1.38)

HIP_X = 0.14
HIP_R = (HIP_X, -0.06, 0.90)
HIP_L = (-HIP_X, -0.06, 0.90)

# IK rest pose: right at BDC (phase=0), left at TDC (phase=pi)
KNEE_R  = (HIP_X, 0.086, 0.549)
KNEE_L  = (-HIP_X, 0.218, 0.641)
ANKLE_R = (0.10, 0.0, 0.22)
ANKLE_L = (-0.10, 0.0, 0.38)

HT_TOP   = (0.0, 0.42, 0.86)
STEM_END = (0.0, 0.46, 0.88)
BAR_R    = (0.18, 0.46, 0.88)
BAR_L    = (-0.18, 0.46, 0.88)
HAND_R   = (0.20, 0.46, 0.88)
HAND_L   = (-0.20, 0.46, 0.88)
ELBOW_R  = (0.20, 0.34, 1.06)
ELBOW_L  = (-0.20, 0.34, 1.06)


# ── Build objects grouped by bone ID ─────────────────────────────

# ── 1. Frame (static bike) ── bone_id = 0 ──

frame_parts = []

seat_cluster = (0, -0.07, 0.84)
saddle_top = (0, -0.10, 0.92)
ht_bot = (0, 0.44, 0.52)

frame_parts.append(create_limb("seat_tube", BB, seat_cluster, 0.020, 0.018, segments=8, material=mat_frame))
frame_parts.append(create_limb("seatpost", seat_cluster, saddle_top, 0.014, 0.012, segments=8, material=mat_frame))
frame_parts.append(create_limb("top_tube", seat_cluster, HT_TOP, 0.018, 0.018, segments=8, material=mat_frame))
frame_parts.append(create_limb("down_tube", ht_bot, BB, 0.020, 0.020, segments=8, material=mat_frame))
frame_parts.append(create_limb("head_tube", ht_bot, HT_TOP, 0.022, 0.022, segments=8, material=mat_frame))
frame_parts.append(create_limb("chainstay_r", BB, (0.03, -0.44, 0.34), 0.014, 0.010, segments=6, material=mat_frame))
frame_parts.append(create_limb("chainstay_l", BB, (-0.03, -0.44, 0.34), 0.014, 0.010, segments=6, material=mat_frame))
frame_parts.append(create_limb("seatstay_r", (0.03, -0.07, 0.82), (0.03, -0.44, 0.34), 0.010, 0.008, segments=6, material=mat_frame))
frame_parts.append(create_limb("seatstay_l", (-0.03, -0.07, 0.82), (-0.03, -0.44, 0.34), 0.010, 0.008, segments=6, material=mat_frame))
frame_parts.append(create_limb("fork", ht_bot, (0, 0.50, 0.34), 0.016, 0.012, segments=6, material=mat_frame))
frame_parts.append(create_body_part("wheel_f", (0, 0.50, 0.34), (0.02, 0.34, 0.34), mat_frame, subdivisions=2))
frame_parts.append(create_body_part("wheel_r", (0, -0.44, 0.34), (0.02, 0.34, 0.34), mat_frame, subdivisions=2))
frame_parts.append(create_limb("stem", HT_TOP, STEM_END, 0.014, 0.014, segments=6, material=mat_frame))
frame_parts.append(create_limb("bars", BAR_L, BAR_R, 0.010, 0.010, segments=6, material=mat_frame))
# Drops
frame_parts.append(create_limb("drop_r1", BAR_R, (0.18, 0.49, 0.84), 0.010, 0.010, segments=6, material=mat_frame))
frame_parts.append(create_limb("drop_r2", (0.18, 0.49, 0.84), (0.18, 0.50, 0.78), 0.010, 0.010, segments=6, material=mat_frame))
frame_parts.append(create_limb("drop_r3", (0.18, 0.50, 0.78), (0.18, 0.46, 0.74), 0.010, 0.010, segments=6, material=mat_frame))
frame_parts.append(create_limb("drop_l1", BAR_L, (-0.18, 0.49, 0.84), 0.010, 0.010, segments=6, material=mat_frame))
frame_parts.append(create_limb("drop_l2", (-0.18, 0.49, 0.84), (-0.18, 0.50, 0.78), 0.010, 0.010, segments=6, material=mat_frame))
frame_parts.append(create_limb("drop_l3", (-0.18, 0.50, 0.78), (-0.18, 0.46, 0.74), 0.010, 0.010, segments=6, material=mat_frame))
# Saddle
frame_parts.append(create_body_part("saddle", (0, -0.10, 0.94), (0.06, 0.12, 0.02), mat_frame, subdivisions=2))

frame = join_objects(frame_parts, "Frame")
set_bone_uv(frame, 0)

# ── 2. Jersey (torso + upper arms) ── bone_id = 0 ──

jersey_parts = []

jersey_parts.append(create_limb("torso_lower", HIP_C, WAIST, 0.13, 0.11, segments=12, material=mat_jersey))
jersey_parts.append(create_limb("torso_upper", WAIST, SHOULDER, 0.11, 0.14, segments=12, material=mat_jersey))
jersey_parts.append(create_body_part("pelvis", HIP_C, (0.17, 0.10, 0.08), mat_jersey, subdivisions=2))
jersey_parts.append(create_body_part("ribcage", (0, 0.14, 1.16), (0.14, 0.11, 0.10), mat_jersey, subdivisions=2))
jersey_parts.append(create_body_part("deltoid_r", (0.16, 0.20, 1.24), (0.07, 0.06, 0.06), mat_jersey, subdivisions=2))
jersey_parts.append(create_body_part("deltoid_l", (-0.16, 0.20, 1.24), (0.07, 0.06, 0.06), mat_jersey, subdivisions=2))
jersey_parts.append(create_limb("upper_arm_r", (0.19, 0.20, 1.24), ELBOW_R, 0.048, 0.038, segments=10, material=mat_jersey))
jersey_parts.append(create_limb("upper_arm_l", (-0.19, 0.20, 1.24), ELBOW_L, 0.048, 0.038, segments=10, material=mat_jersey))

jersey = join_objects(jersey_parts, "Jersey")
set_bone_uv(jersey, 0)

# ── 3. Glutes (static shorts) ── bone_id = 0 ──

glute_parts = []
glute_parts.append(create_body_part("glute_r", (0.06, -0.10, 0.86), (0.12, 0.09, 0.07), mat_shorts, subdivisions=2))
glute_parts.append(create_body_part("glute_l", (-0.06, -0.10, 0.86), (0.12, 0.09, 0.07), mat_shorts, subdivisions=2))
glutes = join_objects(glute_parts, "Glutes")
set_bone_uv(glutes, 0)

# ── 4. Right thigh ── bone_id = +1 ──

thigh_r_parts = []
thigh_r_parts.append(create_limb("thigh_r", HIP_R, KNEE_R, 0.08, 0.055, segments=10, material=mat_shorts))
rt_y = HIP_R[1] + 0.3 * (KNEE_R[1] - HIP_R[1])
rt_z = HIP_R[2] + 0.3 * (KNEE_R[2] - HIP_R[2])
thigh_r_parts.append(create_body_part("quad_r", (HIP_X, rt_y, rt_z), (0.085, 0.075, 0.09), mat_shorts, subdivisions=2))
thigh_r = join_objects(thigh_r_parts, "Thigh_R")
set_bone_uv(thigh_r, 1)

# ── 5. Left thigh ── bone_id = -1 ──

thigh_l_parts = []
thigh_l_parts.append(create_limb("thigh_l", HIP_L, KNEE_L, 0.08, 0.055, segments=10, material=mat_shorts))
lt_y = HIP_L[1] + 0.3 * (KNEE_L[1] - HIP_L[1])
lt_z = HIP_L[2] + 0.3 * (KNEE_L[2] - HIP_L[2])
thigh_l_parts.append(create_body_part("quad_l", (-HIP_X, lt_y, lt_z), (0.085, 0.075, 0.09), mat_shorts, subdivisions=2))
thigh_l = join_objects(thigh_l_parts, "Thigh_L")
set_bone_uv(thigh_l, -1)

# ── 6. Skin static (neck, forearms, hands) ── bone_id = 0 ──

skin_static_parts = []
skin_static_parts.append(create_limb("neck", SHOULDER, (0, 0.24, 1.32), 0.04, 0.035, segments=8, material=mat_skin))
skin_static_parts.append(create_limb("forearm_r", ELBOW_R, HAND_R, 0.038, 0.030, segments=8, material=mat_skin))
skin_static_parts.append(create_limb("forearm_l", ELBOW_L, HAND_L, 0.038, 0.030, segments=8, material=mat_skin))
skin_static_parts.append(create_body_part("hand_r", HAND_R, (0.025, 0.030, 0.020), mat_skin, subdivisions=1))
skin_static_parts.append(create_body_part("hand_l", HAND_L, (0.025, 0.030, 0.020), mat_skin, subdivisions=1))
skin_static = join_objects(skin_static_parts, "Skin_Static")
set_bone_uv(skin_static, 0)

# ── 7. Right shin ── bone_id = +2 ──

shin_r_parts = []
shin_r_parts.append(create_limb("shin_r", KNEE_R, ANKLE_R, 0.050, 0.025, segments=8, material=mat_skin))
calf_r_y = KNEE_R[1] + 0.3 * (ANKLE_R[1] - KNEE_R[1])
calf_r_z = KNEE_R[2] + 0.3 * (ANKLE_R[2] - KNEE_R[2])
shin_r_parts.append(create_body_part("calf_r", (HIP_X, calf_r_y, calf_r_z), (0.050, 0.045, 0.070), mat_skin, subdivisions=2))
shin_r = join_objects(shin_r_parts, "Shin_R")
set_bone_uv(shin_r, 2)

# ── 8. Left shin ── bone_id = -2 ──

shin_l_parts = []
shin_l_parts.append(create_limb("shin_l", KNEE_L, ANKLE_L, 0.050, 0.025, segments=8, material=mat_skin))
calf_l_y = KNEE_L[1] + 0.3 * (ANKLE_L[1] - KNEE_L[1])
calf_l_z = KNEE_L[2] + 0.3 * (ANKLE_L[2] - KNEE_L[2])
shin_l_parts.append(create_body_part("calf_l", (-HIP_X, calf_l_y, calf_l_z), (0.050, 0.045, 0.070), mat_skin, subdivisions=2))
shin_l = join_objects(shin_l_parts, "Shin_L")
set_bone_uv(shin_l, -2)

# ── 9. Helmet ── bone_id = 0 ──

helmet = create_body_part("Helmet", HEAD, (0.09, 0.13, 0.10), mat_helmet, subdivisions=2)
set_bone_uv(helmet, 0)

# ── 10. Right crank + shoe ── bone_id = +3 ──

crank_r_parts = []
crank_r_parts.append(create_limb("crank_r", BB, ANKLE_R, 0.010, 0.008, segments=6, material=mat_frame))
crank_r_parts.append(create_body_part("shoe_r", ANKLE_R, (0.030, 0.060, 0.020), mat_frame, subdivisions=1))
crank_r = join_objects(crank_r_parts, "Crank_R")
set_bone_uv(crank_r, 3)

# ── 11. Left crank + shoe ── bone_id = -3 ──

crank_l_parts = []
crank_l_parts.append(create_limb("crank_l", BB, ANKLE_L, 0.010, 0.008, segments=6, material=mat_frame))
crank_l_parts.append(create_body_part("shoe_l", ANKLE_L, (0.030, 0.060, 0.020), mat_frame, subdivisions=1))
crank_l = join_objects(crank_l_parts, "Crank_L")
set_bone_uv(crank_l, -3)


# ── Mirror Y axis ────────────────────────────────────────────────
# glTF export maps Blender +Y → engine -Z. The engine expects +Z forward,
# so we negate Y on all vertices and flip normals to fix winding.

for obj in bpy.data.objects:
    if obj.type != 'MESH':
        continue
    for v in obj.data.vertices:
        v.co.y = -v.co.y
    # Mirror reverses face winding — flip normals to correct
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.mesh.flip_normals()
    bpy.ops.object.mode_set(mode='OBJECT')


# ── Export ────────────────────────────────────────────────────────

bpy.ops.object.select_all(action='SELECT')

output_path = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "cyclist.glb")
os.makedirs(os.path.dirname(output_path), exist_ok=True)

bpy.ops.export_scene.gltf(
    filepath=output_path,
    export_format='GLB',
    use_selection=True,
    export_apply=True,
    export_yup=True,
    export_materials='EXPORT',
    export_normals=True,
)

print(f"\nExported cyclist to: {output_path}")
objects = [obj for obj in bpy.data.objects if obj.type == 'MESH']
print(f"Objects ({len(objects)}): {[obj.name for obj in objects]}")
total_verts = sum(len(obj.data.vertices) for obj in objects)
total_tris = sum(len(obj.data.polygons) for obj in objects)
print(f"Total: {total_verts} vertices, {total_tris} faces")
