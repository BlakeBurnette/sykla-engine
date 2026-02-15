"""
Inspect the purchased FBX cyclist model — dump bones, materials, vertex groups.
Run: blender --background --python tools/inspect_fbx.py
"""
import bpy
import os

bpy.ops.wm.read_factory_settings(use_empty=True)

BASE = "/Users/blakeburnette/My project/Assets/Male_Cyclist"

# Import cyclist A-Pose
cyclist_fbx = os.path.join(BASE, "Skeletal_Meshes", "SK_Male_Cyclist_A-Pose.FBX")
bpy.ops.import_scene.fbx(filepath=cyclist_fbx)

print("\n=== CYCLIST A-POSE ===")
for obj in bpy.data.objects:
    print(f"\nObject: {obj.name}  type={obj.type}")
    if obj.type == 'MESH':
        mesh = obj.data
        print(f"  Vertices: {len(mesh.vertices)}")
        print(f"  Polygons: {len(mesh.polygons)}")
        print(f"  Materials ({len(mesh.materials)}): {[m.name for m in mesh.materials]}")
        print(f"  Vertex groups ({len(obj.vertex_groups)}): {[vg.name for vg in obj.vertex_groups]}")

        # Count vertices per vertex group (dominant weight)
        vg_counts = {}
        for v in mesh.vertices:
            if v.groups:
                dominant = max(v.groups, key=lambda g: g.weight)
                gname = obj.vertex_groups[dominant.group].name
                vg_counts[gname] = vg_counts.get(gname, 0) + 1
        if vg_counts:
            print(f"  Vertex group counts (dominant):")
            for name, count in sorted(vg_counts.items(), key=lambda x: -x[1]):
                print(f"    {name}: {count}")

    elif obj.type == 'ARMATURE':
        arm = obj.data
        print(f"  Bones ({len(arm.bones)}):")
        for bone in arm.bones:
            parent = bone.parent.name if bone.parent else "None"
            print(f"    {bone.name}  parent={parent}  head={tuple(round(x,3) for x in bone.head_local)}  tail={tuple(round(x,3) for x in bone.tail_local)}")

# Now import bike
bpy.ops.import_scene.fbx(filepath=os.path.join(BASE, "Skeletal_Meshes", "SK_Racing_Bike.FBX"))

print("\n=== RACING BIKE ===")
for obj in bpy.data.objects:
    if "Bike" in obj.name or "bike" in obj.name or "Racing" in obj.name:
        print(f"\nObject: {obj.name}  type={obj.type}")
        if obj.type == 'MESH':
            mesh = obj.data
            print(f"  Vertices: {len(mesh.vertices)}")
            print(f"  Polygons: {len(mesh.polygons)}")
            print(f"  Materials ({len(mesh.materials)}): {[m.name for m in mesh.materials]}")
            print(f"  Vertex groups ({len(obj.vertex_groups)}): {[vg.name for vg in obj.vertex_groups]}")
        elif obj.type == 'ARMATURE':
            arm = obj.data
            print(f"  Bones ({len(arm.bones)}):")
            for bone in arm.bones:
                parent = bone.parent.name if bone.parent else "None"
                print(f"    {bone.name}  parent={parent}")

# Print material info
print("\n=== ALL MATERIALS ===")
for mat in bpy.data.materials:
    print(f"\nMaterial: {mat.name}")
    if mat.use_nodes and mat.node_tree:
        for node in mat.node_tree.nodes:
            if node.type == 'BSDF_PRINCIPLED':
                bc = node.inputs.get("Base Color")
                if bc:
                    if bc.links:
                        link = bc.links[0]
                        print(f"  Base Color: linked from {link.from_node.type}")
                        if link.from_node.type == 'TEX_IMAGE':
                            img = link.from_node.image
                            print(f"    Image: {img.name if img else 'None'}")
                    else:
                        print(f"  Base Color: {tuple(bc.default_value)}")
