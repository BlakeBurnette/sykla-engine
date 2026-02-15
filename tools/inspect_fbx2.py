"""Quick check: bike object names + animation frame count."""
import bpy, os

bpy.ops.wm.read_factory_settings(use_empty=True)
BASE = "/Users/blakeburnette/My project/Assets/Male_Cyclist"

# Import bike
bpy.ops.import_scene.fbx(filepath=os.path.join(BASE, "Skeletal_Meshes", "SK_Racing_Bike.FBX"))

print("\n=== ALL OBJECTS AFTER BIKE IMPORT ===")
for obj in bpy.data.objects:
    print(f"  {obj.name}  type={obj.type}")
    if obj.type == 'MESH':
        mesh = obj.data
        print(f"    Verts={len(mesh.vertices)} Polys={len(mesh.polygons)} Mats={[m.name for m in mesh.materials]}")
        print(f"    VGroups={[vg.name for vg in obj.vertex_groups]}")
    elif obj.type == 'ARMATURE':
        print(f"    Bones={[b.name for b in obj.data.bones]}")

# Now import an animation to check frame range
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=os.path.join(BASE, "Skeletal_Meshes", "SK_Male_Cyclist_A-Pose.FBX"))

# Import Easy Pedaling animation
bpy.ops.import_scene.fbx(filepath=os.path.join(BASE, "Animations", "ANIM_Male_Cyclist_Easy_Pedaling.FBX"))

print("\n=== ACTIONS (ANIMATIONS) ===")
for action in bpy.data.actions:
    print(f"  {action.name}: frames {action.frame_range[0]:.0f}-{action.frame_range[1]:.0f}")
