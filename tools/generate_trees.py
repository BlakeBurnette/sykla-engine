#!/usr/bin/env python3
"""
Generate tree GLB assets for sykla-engine.

Usage:
    blender --background --python tools/generate_trees.py

Produces:
    assets/trees/loblolly_pine.glb
    assets/trees/oak.glb
    assets/trees/redbud.glb

Each GLB contains bark (trunk+branches) and foliage mesh parts with baked
512x512 procedural textures as baseColorTexture.

All geometry uses Blender Z-up convention. The glTF exporter converts to Y-up.
"""

import bpy
import bmesh
import math
import os
import sys
from mathutils import Vector, Matrix

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.dirname(SCRIPT_DIR)
OUTPUT_DIR = os.path.join(PROJECT_ROOT, "assets", "trees")
BAKE_SIZE = 512


def reset_scene():
    """Remove all objects, meshes, materials, images from the scene."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    if not bpy.context.scene:
        bpy.ops.scene.new()


def new_material(name, base_color=(0.5, 0.5, 0.5, 1.0)):
    """Create a simple Principled BSDF material."""
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    nodes.clear()
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Base Color"].default_value = base_color
    bsdf.inputs["Roughness"].default_value = 0.9
    bsdf.inputs["Specular IOR Level"].default_value = 0.1
    output = nodes.new("ShaderNodeOutputMaterial")
    mat.node_tree.links.new(bsdf.outputs["BSDF"], output.inputs["Surface"])
    return mat


def add_procedural_bark(mat, color1, color2, noise_scale=8.0, detail=4.0):
    """Add noise procedural texture to a bark material."""
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    bsdf = None
    for n in nodes:
        if n.type == "BSDF_PRINCIPLED":
            bsdf = n
            break
    if not bsdf:
        return

    tex_coord = nodes.new("ShaderNodeTexCoord")
    mapping = nodes.new("ShaderNodeMapping")
    mapping.inputs["Scale"].default_value = (noise_scale, noise_scale, noise_scale)
    links.new(tex_coord.outputs["UV"], mapping.inputs["Vector"])

    noise = nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = noise_scale
    noise.inputs["Detail"].default_value = detail
    noise.inputs["Roughness"].default_value = 0.7
    links.new(mapping.outputs["Vector"], noise.inputs["Vector"])

    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].color = (*color1, 1.0)
    ramp.color_ramp.elements[1].color = (*color2, 1.0)
    ramp.color_ramp.elements[0].position = 0.3
    ramp.color_ramp.elements[1].position = 0.7
    links.new(noise.outputs["Fac"], ramp.inputs["Fac"])
    links.new(ramp.outputs["Color"], bsdf.inputs["Base Color"])


def add_procedural_foliage(mat, color1, color2, noise_scale=12.0):
    """Add noise procedural texture to a foliage material."""
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    bsdf = None
    for n in nodes:
        if n.type == "BSDF_PRINCIPLED":
            bsdf = n
            break
    if not bsdf:
        return

    tex_coord = nodes.new("ShaderNodeTexCoord")
    noise = nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = noise_scale
    noise.inputs["Detail"].default_value = 6.0
    noise.inputs["Roughness"].default_value = 0.6
    links.new(tex_coord.outputs["UV"], noise.inputs["Vector"])

    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].color = (*color1, 1.0)
    ramp.color_ramp.elements[1].color = (*color2, 1.0)
    ramp.color_ramp.elements[0].position = 0.25
    ramp.color_ramp.elements[1].position = 0.75
    links.new(noise.outputs["Fac"], ramp.inputs["Fac"])
    links.new(ramp.outputs["Color"], bsdf.inputs["Base Color"])


def smart_uv_project(obj):
    """Apply Smart UV Project to an object."""
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=66, island_margin=0.02)
    bpy.ops.object.mode_set(mode="OBJECT")


def bake_diffuse(obj, mat, image_name):
    """Bake diffuse color to an image texture and reassign to material."""
    img = bpy.data.images.new(image_name, BAKE_SIZE, BAKE_SIZE)
    nodes = mat.node_tree.nodes
    img_node = nodes.new("ShaderNodeTexImage")
    img_node.image = img
    img_node.name = "BakeTarget"
    for n in nodes:
        n.select = False
    img_node.select = True
    nodes.active = img_node

    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj

    bpy.context.scene.render.engine = "CYCLES"
    bpy.context.scene.cycles.device = "CPU"
    bpy.context.scene.cycles.samples = 4
    bpy.context.scene.cycles.bake_type = "DIFFUSE"
    bpy.context.scene.render.bake.use_pass_direct = False
    bpy.context.scene.render.bake.use_pass_indirect = False
    bpy.context.scene.render.bake.use_pass_color = True

    bpy.ops.object.bake(type="DIFFUSE")

    bsdf = None
    for n in nodes:
        if n.type == "BSDF_PRINCIPLED":
            bsdf = n
            break

    if bsdf:
        for link in list(mat.node_tree.links):
            if link.to_socket == bsdf.inputs["Base Color"]:
                mat.node_tree.links.remove(link)
        mat.node_tree.links.new(img_node.outputs["Color"], bsdf.inputs["Base Color"])

    used_nodes = set()
    for link in mat.node_tree.links:
        used_nodes.add(link.from_node)
        used_nodes.add(link.to_node)
    for n in list(nodes):
        if n not in used_nodes and n.type not in ("OUTPUT_MATERIAL",):
            nodes.remove(n)

    return img


# ── Geometry helpers ───────────────────────────────────────────


def add_trunk_to_bmesh(bm, base_r, top_r, height, segments=10, sections=6,
                       base_pos=(0, 0, 0), flare_strength=0.3):
    """Add a tapered trunk cylinder to an existing bmesh. Z-up."""
    offset = len(bm.verts)
    bx, by, bz = base_pos

    for s in range(sections + 1):
        t = s / sections
        z = bz + t * height
        r = base_r + (top_r - base_r) * t
        # Root flare at base
        flare = math.sin((1 - t) * math.pi * 0.5) ** 1.5 * base_r * flare_strength
        r += flare

        for i in range(segments):
            angle = (i / segments) * math.pi * 2
            bump = 1.0 + 0.05 * math.sin(angle * 3.0 + s * 0.5)
            x = bx + math.cos(angle) * r * bump
            y = by + math.sin(angle) * r * bump
            bm.verts.new((x, y, z))

    bm.verts.ensure_lookup_table()

    for s in range(sections):
        for i in range(segments):
            bl = offset + s * segments + i
            br = offset + s * segments + (i + 1) % segments
            tl = bl + segments
            tr = br + segments
            try:
                bm.faces.new([bm.verts[bl], bm.verts[tl], bm.verts[tr], bm.verts[br]])
            except ValueError:
                pass


def add_branch_to_bmesh(bm, start, direction, length, base_r, tip_r,
                        segments=6, sections=4):
    """Add a branch (tapered cylinder along arbitrary direction) to bmesh.
    start: (x, y, z) branch origin
    direction: (dx, dy, dz) normalized direction vector
    """
    offset = len(bm.verts)
    d = Vector(direction).normalized()

    # Build local coordinate frame
    up = Vector((0, 0, 1))
    if abs(d.dot(up)) > 0.95:
        up = Vector((1, 0, 0))
    right = d.cross(up).normalized()
    fwd = right.cross(d).normalized()

    sx, sy, sz = start

    for s in range(sections + 1):
        t = s / sections
        r = base_r + (tip_r - base_r) * t
        cx = sx + d.x * length * t
        cy = sy + d.y * length * t
        cz = sz + d.z * length * t

        for i in range(segments):
            angle = (i / segments) * math.pi * 2
            cos_a = math.cos(angle)
            sin_a = math.sin(angle)
            x = cx + (right.x * cos_a + fwd.x * sin_a) * r
            y = cy + (right.y * cos_a + fwd.y * sin_a) * r
            z = cz + (right.z * cos_a + fwd.z * sin_a) * r
            bm.verts.new((x, y, z))

    bm.verts.ensure_lookup_table()

    for s in range(sections):
        for i in range(segments):
            bl = offset + s * segments + i
            br = offset + s * segments + (i + 1) % segments
            tl = bl + segments
            tr = br + segments
            try:
                bm.faces.new([bm.verts[bl], bm.verts[tl], bm.verts[tr], bm.verts[br]])
            except ValueError:
                pass


def add_canopy_lobe(bm, center, radius, z_scale=1.0, stacks=6, slices=10,
                    lump_freq=3.0, lump_amp=0.15):
    """Add a lumpy sphere canopy lobe to bmesh. Z-up."""
    offset = len(bm.verts)
    row = slices + 1

    for i in range(stacks + 1):
        phi = math.pi * i / stacks
        for j in range(slices + 1):
            theta = math.pi * 2 * j / slices
            lump = 1.0 + lump_amp * math.sin(theta * lump_freq) * math.cos(phi * 2)
            r = radius * lump
            x = center[0] + math.cos(theta) * math.sin(phi) * r
            y = center[1] + math.sin(theta) * math.sin(phi) * r
            z = center[2] + math.cos(phi) * r * z_scale
            bm.verts.new((x, y, z))

    bm.verts.ensure_lookup_table()

    for i in range(stacks):
        for j in range(slices):
            a = offset + i * row + j
            b = a + row
            try:
                bm.faces.new([bm.verts[a], bm.verts[b], bm.verts[b + 1], bm.verts[a + 1]])
            except (ValueError, IndexError):
                pass


def bmesh_to_object(bm, name, mat):
    """Convert bmesh to Blender object with material, UV project, and bake."""
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()

    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    obj.data.materials.append(mat)
    smart_uv_project(obj)
    bake_diffuse(obj, mat, f"{name}_bake")
    return obj


# ── Species generators ──────────────────────────────────────────


def generate_loblolly_pine():
    """Loblolly Pine — ~24m (80ft), long bare trunk with sparse upper crown."""
    reset_scene()
    objects = []

    # --- Trunk + branches (single bark mesh) ---
    bm = bmesh.new()

    # Main trunk: 24m tall, straight
    add_trunk_to_bmesh(bm, base_r=0.40, top_r=0.10, height=24.0,
                       segments=8, sections=10, flare_strength=0.25)

    # Branch whorls in upper 40% (from ~14m to ~22m)
    # Loblolly pine: sparse whorls of 4-5 branches, slightly drooping
    whorl_heights = [14.0, 15.5, 17.0, 18.5, 20.0, 21.5]
    for wi, wh in enumerate(whorl_heights):
        n_branches = 4 + (wi % 2)  # alternate 4 and 5
        angle_offset = wi * 0.6  # rotate each whorl
        # Branches get shorter toward top
        br_length = 3.5 - (wi / len(whorl_heights)) * 2.0
        br_base_r = 0.08 - wi * 0.005
        br_tip_r = 0.02

        for bi in range(n_branches):
            angle = angle_offset + (bi / n_branches) * math.pi * 2
            # Slightly downward angle (pine branches droop)
            dip = -0.25 - (1.0 - wi / len(whorl_heights)) * 0.15
            dx = math.cos(angle) * 0.85
            dy = math.sin(angle) * 0.85
            dz = dip
            add_branch_to_bmesh(bm, start=(0, 0, wh),
                                direction=(dx, dy, dz),
                                length=br_length,
                                base_r=max(br_base_r, 0.03),
                                tip_r=br_tip_r,
                                segments=5, sections=3)

    bark_mat = new_material("PineBark", (0.40, 0.22, 0.10, 1.0))
    add_procedural_bark(bark_mat,
                        color1=(0.55, 0.28, 0.12),
                        color2=(0.30, 0.15, 0.06),
                        noise_scale=6.0)
    trunk_obj = bmesh_to_object(bm, "PineTrunk", bark_mat)
    objects.append(trunk_obj)

    # --- Crown (foliage) ---
    bm = bmesh.new()

    # Elongated conical crown: taller than wide, centered in upper portion
    # Multiple tiers of foliage clusters
    crown_lobes = [
        # (center_xyz, radius, z_scale)
        ((0, 0, 20.0), 3.0, 0.6),    # main upper mass
        ((0, 0, 17.5), 3.8, 0.5),    # widest mid section
        ((0, 0, 15.5), 3.2, 0.45),   # lower crown
        ((0, 0, 22.5), 2.0, 0.7),    # top cone
        ((1.5, 0, 18.0), 2.5, 0.45), # asymmetric bulges
        ((-1.0, 1.5, 19.0), 2.2, 0.5),
        ((0, -1.5, 16.5), 2.8, 0.4),
    ]
    for center, radius, zs in crown_lobes:
        add_canopy_lobe(bm, center, radius, z_scale=zs,
                        stacks=6, slices=8, lump_freq=4.0, lump_amp=0.2)

    foliage_mat = new_material("PineFoliage", (0.06, 0.22, 0.04, 1.0))
    add_procedural_foliage(foliage_mat,
                           color1=(0.04, 0.18, 0.03),
                           color2=(0.10, 0.28, 0.06),
                           noise_scale=15.0)
    crown_obj = bmesh_to_object(bm, "PineCrown", foliage_mat)
    objects.append(crown_obj)

    export_glb(objects, os.path.join(OUTPUT_DIR, "loblolly_pine.glb"))


def generate_oak():
    """Oak — ~15m (50ft), thick trunk forking into major spreading limbs."""
    reset_scene()
    objects = []

    # --- Trunk + branches (single bark mesh) ---
    bm = bmesh.new()

    # Main trunk: thick, to about 7m before major fork
    add_trunk_to_bmesh(bm, base_r=0.65, top_r=0.35, height=7.0,
                       segments=10, sections=6, flare_strength=0.35)

    # 5 major limbs radiating from upper trunk (5-7m)
    major_branches = [
        # (start_z, angle, elevation_angle, length, base_r, tip_r)
        (6.0, 0.0,    0.50, 5.5, 0.25, 0.08),    # front
        (6.5, 1.25,   0.55, 5.0, 0.22, 0.07),    # right-front
        (5.5, 2.50,   0.40, 6.0, 0.28, 0.09),    # right-back
        (6.8, 3.75,   0.60, 4.5, 0.20, 0.06),    # left-back
        (5.8, 5.00,   0.45, 5.5, 0.24, 0.08),    # left-front
    ]

    for start_z, angle, elev, length, br, tr in major_branches:
        dx = math.cos(angle) * math.cos(elev)
        dy = math.sin(angle) * math.cos(elev)
        dz = math.sin(elev)
        add_branch_to_bmesh(bm, start=(0, 0, start_z),
                            direction=(dx, dy, dz),
                            length=length, base_r=br, tip_r=tr,
                            segments=6, sections=4)

        # Secondary branches off each major limb
        for sub_t in [0.4, 0.7]:
            sub_start_x = dx * length * sub_t
            sub_start_y = dy * length * sub_t
            sub_start_z = start_z + dz * length * sub_t
            sub_angle = angle + 0.6 * (1 if sub_t < 0.5 else -1)
            sub_elev = elev + 0.2
            sub_dx = math.cos(sub_angle) * math.cos(sub_elev)
            sub_dy = math.sin(sub_angle) * math.cos(sub_elev)
            sub_dz = math.sin(sub_elev)
            sub_len = length * 0.4
            add_branch_to_bmesh(bm,
                                start=(sub_start_x, sub_start_y, sub_start_z),
                                direction=(sub_dx, sub_dy, sub_dz),
                                length=sub_len,
                                base_r=tr * 1.5, tip_r=tr * 0.4,
                                segments=5, sections=3)

    bark_mat = new_material("OakBark", (0.28, 0.16, 0.06, 1.0))
    add_procedural_bark(bark_mat,
                        color1=(0.35, 0.22, 0.10),
                        color2=(0.20, 0.12, 0.05),
                        noise_scale=5.0, detail=6.0)
    trunk_obj = bmesh_to_object(bm, "OakTrunk", bark_mat)
    objects.append(trunk_obj)

    # --- Crown (foliage) ---
    # Broad spreading dome, matching the branch structure
    bm = bmesh.new()

    crown_lobes = [
        # Main dome centered above trunk
        ((0, 0, 11.0),  4.5, 0.50),   # central dome
        # Lobes over each major limb direction
        ((3.5, 0, 10.5),  3.5, 0.45),
        ((1.5, 3.2, 10.8), 3.2, 0.48),
        ((-2.8, 2.0, 10.0), 3.8, 0.45),
        ((-2.0, -3.0, 11.0), 3.5, 0.46),
        ((2.0, -3.2, 10.5), 3.2, 0.48),
        # Top crown
        ((0, 0, 13.5),  3.0, 0.40),
        # Low outer extensions
        ((4.5, 1.0, 9.5), 2.5, 0.40),
        ((-3.5, -2.5, 9.0), 2.8, 0.42),
    ]

    for center, radius, zs in crown_lobes:
        add_canopy_lobe(bm, center, radius, z_scale=zs,
                        stacks=6, slices=10, lump_freq=3.0, lump_amp=0.15)

    foliage_mat = new_material("OakFoliage", (0.10, 0.28, 0.05, 1.0))
    add_procedural_foliage(foliage_mat,
                           color1=(0.08, 0.22, 0.04),
                           color2=(0.15, 0.35, 0.08),
                           noise_scale=10.0)
    crown_obj = bmesh_to_object(bm, "OakCrown", foliage_mat)
    objects.append(crown_obj)

    export_glb(objects, os.path.join(OUTPUT_DIR, "oak.glb"))


def generate_redbud():
    """Redbud — ~9m (30ft), multi-stemmed with low branching, heart-shaped crown."""
    reset_scene()
    objects = []

    # --- Trunk + branches (single bark mesh) ---
    bm = bmesh.new()

    # Primary stem — slightly leaning
    add_trunk_to_bmesh(bm, base_r=0.10, top_r=0.04, height=7.0,
                       segments=6, sections=6, flare_strength=0.2)

    # Second stem — offset and leaning opposite
    add_trunk_to_bmesh(bm, base_r=0.08, top_r=0.03, height=6.0,
                       segments=6, sections=5,
                       base_pos=(0.15, 0.10, 0), flare_strength=0.15)

    # Third stem — smaller
    add_trunk_to_bmesh(bm, base_r=0.06, top_r=0.025, height=5.5,
                       segments=6, sections=5,
                       base_pos=(-0.10, 0.12, 0), flare_strength=0.15)

    # Branches from primary stem — start low, spread wide
    redbud_branches = [
        # (start_z, angle, elev, length, base_r, tip_r)
        (3.0, 0.3,  0.35, 3.0, 0.05, 0.015),
        (3.5, 2.0,  0.40, 2.8, 0.045, 0.012),
        (4.5, 3.8,  0.50, 2.5, 0.04, 0.012),
        (5.0, 5.2,  0.55, 2.2, 0.035, 0.010),
        (2.5, 1.2,  0.30, 3.2, 0.05, 0.015),
        (4.0, 4.5,  0.45, 2.6, 0.04, 0.012),
    ]

    for start_z, angle, elev, length, br, tr in redbud_branches:
        dx = math.cos(angle) * math.cos(elev)
        dy = math.sin(angle) * math.cos(elev)
        dz = math.sin(elev)
        add_branch_to_bmesh(bm, start=(0, 0, start_z),
                            direction=(dx, dy, dz),
                            length=length, base_r=br, tip_r=tr,
                            segments=5, sections=3)

    # Branches from second stem
    for start_z, angle, elev, length, br, tr in [
        (2.5, 1.5, 0.35, 2.5, 0.04, 0.012),
        (3.5, 3.5, 0.45, 2.2, 0.035, 0.010),
        (4.0, 5.0, 0.50, 2.0, 0.03, 0.010),
    ]:
        dx = math.cos(angle) * math.cos(elev)
        dy = math.sin(angle) * math.cos(elev)
        dz = math.sin(elev)
        add_branch_to_bmesh(bm, start=(0.15, 0.10, start_z),
                            direction=(dx, dy, dz),
                            length=length, base_r=br, tip_r=tr,
                            segments=5, sections=3)

    bark_mat = new_material("RedbudBark", (0.35, 0.28, 0.22, 1.0))
    add_procedural_bark(bark_mat,
                        color1=(0.40, 0.32, 0.25),
                        color2=(0.30, 0.24, 0.18),
                        noise_scale=3.0, detail=2.0)
    trunk_obj = bmesh_to_object(bm, "RedbudTrunk", bark_mat)
    objects.append(trunk_obj)

    # --- Crown (foliage) ---
    # Heart-shaped: two main lobes with a notch at top center
    bm = bmesh.new()

    crown_lobes = [
        # Two side lobes (the "bumps" of the heart)
        ((1.8, 0, 7.0),  2.8, 0.55),   # right lobe
        ((-1.8, 0, 7.0), 2.8, 0.55),   # left lobe
        # Lower fill connecting to trunk
        ((0, 0, 5.5),    2.5, 0.50),    # lower center
        # Side extensions
        ((0.8, 1.8, 6.5),  2.0, 0.45),
        ((-0.8, -1.8, 6.5), 2.0, 0.45),
        ((0, 1.5, 7.0),   1.8, 0.50),
        ((0, -1.5, 7.0),  1.8, 0.50),
        # Upper — NOT centered (leaves gap for heart notch)
        ((1.5, 0, 8.5),   1.5, 0.45),
        ((-1.5, 0, 8.5),  1.5, 0.45),
    ]

    for center, radius, zs in crown_lobes:
        add_canopy_lobe(bm, center, radius, z_scale=zs,
                        stacks=6, slices=8, lump_freq=4.0, lump_amp=0.2)

    foliage_mat = new_material("RedbudFoliage", (0.20, 0.30, 0.12, 1.0))
    # Mix green with pink/magenta for spring blooms
    nodes = foliage_mat.node_tree.nodes
    links = foliage_mat.node_tree.links
    bsdf = None
    for n in nodes:
        if n.type == "BSDF_PRINCIPLED":
            bsdf = n
            break

    if bsdf:
        tex_coord = nodes.new("ShaderNodeTexCoord")

        noise1 = nodes.new("ShaderNodeTexNoise")
        noise1.inputs["Scale"].default_value = 10.0
        noise1.inputs["Detail"].default_value = 5.0
        links.new(tex_coord.outputs["UV"], noise1.inputs["Vector"])

        ramp1 = nodes.new("ShaderNodeValToRGB")
        ramp1.color_ramp.elements[0].color = (0.12, 0.28, 0.06, 1.0)
        ramp1.color_ramp.elements[1].color = (0.22, 0.40, 0.10, 1.0)
        links.new(noise1.outputs["Fac"], ramp1.inputs["Fac"])

        noise2 = nodes.new("ShaderNodeTexNoise")
        noise2.inputs["Scale"].default_value = 6.0
        noise2.inputs["Detail"].default_value = 3.0
        links.new(tex_coord.outputs["UV"], noise2.inputs["Vector"])

        ramp2 = nodes.new("ShaderNodeValToRGB")
        ramp2.color_ramp.elements[0].color = (0.75, 0.20, 0.45, 1.0)
        ramp2.color_ramp.elements[1].color = (0.90, 0.55, 0.65, 1.0)
        ramp2.color_ramp.elements[0].position = 0.4
        ramp2.color_ramp.elements[1].position = 0.6
        links.new(noise2.outputs["Fac"], ramp2.inputs["Fac"])

        mix = nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.inputs["Factor"].default_value = 0.35
        links.new(ramp1.outputs["Color"], mix.inputs[6])
        links.new(ramp2.outputs["Color"], mix.inputs[7])
        links.new(mix.outputs[2], bsdf.inputs["Base Color"])

    crown_obj = bmesh_to_object(bm, "RedbudCrown", foliage_mat)
    objects.append(crown_obj)

    export_glb(objects, os.path.join(OUTPUT_DIR, "redbud.glb"))


def export_glb(objects, filepath):
    """Export selected objects as GLB with embedded textures."""
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]

    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

    os.makedirs(os.path.dirname(filepath), exist_ok=True)

    bpy.ops.export_scene.gltf(
        filepath=filepath,
        use_selection=True,
        export_format="GLB",
        export_apply=True,
        export_image_format="AUTO",
        export_materials="EXPORT",
        export_normals=True,
        export_texcoords=True,
    )
    print(f"Exported: {filepath}")


# ── Main ────────────────────────────────────────────────────────

if __name__ == "__main__":
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    print("Generating Loblolly Pine...")
    generate_loblolly_pine()

    print("Generating Oak...")
    generate_oak()

    print("Generating Redbud...")
    generate_redbud()

    print("Done! GLB files written to:", OUTPUT_DIR)
