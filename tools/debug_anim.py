"""Debug: check if animation is actually applied to the cyclist armature."""
import bpy, os

bpy.ops.wm.read_factory_settings(use_empty=True)
BASE = "/Users/blakeburnette/My project/Assets/Male_Cyclist"

# Import cyclist A-Pose
bpy.ops.import_scene.fbx(filepath=os.path.join(BASE, "Skeletal_Meshes", "SK_Male_Cyclist_A-Pose.FBX"))

armature = None
for obj in bpy.data.objects:
    if obj.type == 'ARMATURE':
        armature = obj
        break

print(f"\nArmature: {armature.name}")

# Rest pose
scene = bpy.context.scene
scene.frame_set(1)
bpy.context.view_layer.update()

print(f"\n=== REST POSE ===")
for bn in ['Pelvis', 'Thigh_R', 'calf_r', 'Foot_R', 'Thigh_L', 'calf_l', 'Foot_L', 'head']:
    pb = armature.pose.bones.get(bn)
    if pb:
        pos = armature.matrix_world @ pb.head
        print(f"  {bn:12s}: ({pos.x:7.4f}, {pos.y:7.4f}, {pos.z:7.4f})")

# Import animation
bpy.ops.import_scene.fbx(filepath=os.path.join(BASE, "Animations", "ANIM_Male_Cyclist_Easy_Pedaling.FBX"))

# Apply action
for action in bpy.data.actions:
    if 'Easy_Pedaling' in action.name:
        if not armature.animation_data:
            armature.animation_data_create()
        armature.animation_data.action = action

        # Blender 5.0 slot API
        if hasattr(action, 'slots'):
            for slot in action.slots:
                try:
                    armature.animation_data.action_slot = slot
                    print(f"  Set slot: {slot.identifier if hasattr(slot, 'identifier') else '?'}")
                except Exception as e:
                    print(f"  Slot error: {e}")
                break
        print(f"Applied: {action.name}")
        break

# Test frames
print(f"\n=== ANIMATED POSE (should differ from rest) ===")
for frame in [1, 8, 15, 25]:
    scene.frame_set(frame)
    bpy.context.view_layer.update()
    print(f"\nFrame {frame}:")
    for bn in ['Pelvis', 'Thigh_R', 'calf_r', 'Foot_R', 'Thigh_L', 'calf_l', 'Foot_L', 'head']:
        pb = armature.pose.bones.get(bn)
        if pb:
            pos = armature.matrix_world @ pb.head
            print(f"  {bn:12s}: ({pos.x:7.4f}, {pos.y:7.4f}, {pos.z:7.4f})")

# Also try: import animation directly into the cyclist armature by using NLA
# Alternative: use the Root.001 armature directly
print(f"\n=== Trying Root.001 armature ===")
arm2 = None
for obj in bpy.data.objects:
    if obj.type == 'ARMATURE' and obj.name == 'Root.001':
        arm2 = obj
        break

if arm2:
    for frame in [1, 15]:
        scene.frame_set(frame)
        bpy.context.view_layer.update()
        print(f"\nRoot.001 Frame {frame}:")
        for bn in ['Pelvis', 'Thigh_R', 'calf_r', 'Foot_R', 'head']:
            pb = arm2.pose.bones.get(bn)
            if pb:
                pos = arm2.matrix_world @ pb.head
                print(f"  {bn:12s}: ({pos.x:7.4f}, {pos.y:7.4f}, {pos.z:7.4f})")
