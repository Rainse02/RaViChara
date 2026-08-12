"""Headless smoke test for the bounded RaViChara preview encoder."""

from __future__ import annotations

import base64
import importlib.util
import json
from pathlib import Path
import struct
import sys
from types import ModuleType
import zlib

import bpy
import numpy as np


class RAVICHARA_TEST_MMD_BONE(bpy.types.PropertyGroup):
    name_j: bpy.props.StringProperty(default="")
    name_e: bpy.props.StringProperty(default="")
    bone_id: bpy.props.IntProperty(default=-1)
    transform_order: bpy.props.IntProperty(default=0)
    is_controllable: bpy.props.BoolProperty(default=True)
    transform_after_dynamics: bpy.props.BoolProperty(default=False)
    enabled_fixed_axis: bpy.props.BoolProperty(default=False)
    fixed_axis: bpy.props.FloatVectorProperty(size=3, default=(0.0, 0.0, 0.0))
    enabled_local_axes: bpy.props.BoolProperty(default=False)
    local_axis_x: bpy.props.FloatVectorProperty(size=3, default=(1.0, 0.0, 0.0))
    local_axis_z: bpy.props.FloatVectorProperty(size=3, default=(0.0, 0.0, 1.0))
    is_tip: bpy.props.BoolProperty(default=False)
    ik_rotation_constraint: bpy.props.FloatProperty(default=1.0)
    has_additional_rotation: bpy.props.BoolProperty(default=False)
    has_additional_location: bpy.props.BoolProperty(default=False)
    additional_transform_bone: bpy.props.StringProperty(default="")
    additional_transform_influence: bpy.props.FloatProperty(default=1.0)


def ensure_test_mmd_properties() -> None:
    if not hasattr(bpy.types.PoseBone, "mmd_bone"):
        bpy.utils.register_class(RAVICHARA_TEST_MMD_BONE)
        bpy.types.PoseBone.mmd_bone = bpy.props.PointerProperty(
            type=RAVICHARA_TEST_MMD_BONE
        )
    if not hasattr(bpy.types.PoseBone, "mmd_ik_toggle"):
        bpy.types.PoseBone.mmd_ik_toggle = bpy.props.BoolProperty(default=True)
    if not hasattr(bpy.types.PoseBone, "is_mmd_shadow_bone"):
        bpy.types.PoseBone.is_mmd_shadow_bone = bpy.props.BoolProperty(default=False)
    if not hasattr(bpy.types.PoseBone, "mmd_shadow_bone_type"):
        bpy.types.PoseBone.mmd_shadow_bone_type = bpy.props.StringProperty(default="")


def load_bridge():
    source = Path(__file__).with_name("ravichara_preview_bridge") / "__init__.py"
    spec = importlib.util.spec_from_file_location(
        "ravichara_preview_bridge_verification",
        source,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load preview bridge from {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def build_test_scene():
    ensure_test_mmd_properties()
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    bpy.ops.object.empty_add(type="PLAIN_AXES", location=(0.0, 0.0, 0.0))
    model = bpy.context.object
    model.name = "RaViCharaTestModel"
    bpy.ops.mesh.primitive_cube_add(location=(0.0, 0.0, 0.0))
    mesh = bpy.context.object
    mesh.name = "HairMesh"
    mesh.parent = model
    material = bpy.data.materials.new("HairMaterial")
    material.use_nodes = True
    mesh.data.materials.append(material)
    mesh.shape_key_add(name="Basis")
    smile = mesh.shape_key_add(name="SmileCustom")
    smile.value = 0.0
    bpy.ops.object.camera_add(location=(4.0, -4.0, 3.0))
    camera = bpy.context.object
    camera.rotation_euler = (1.109, 0.0, 0.785)
    bpy.context.scene.camera = camera
    bpy.ops.object.light_add(type="AREA", location=(2.0, -2.0, 4.0))
    bpy.context.object.data.energy = 800.0
    bpy.ops.object.armature_add(location=(0.0, 0.0, 0.0))
    armature = bpy.context.object
    armature.name = "RaViCharaTestArmature"
    armature.data.bones[0].name = "PmxHeadInternal"
    armature.parent = model
    bpy.context.view_layer.objects.active = armature
    bpy.ops.object.mode_set(mode="EDIT")
    for name in (
        "UpperBody", "UpperBody2", "Neck", "Shoulder.L", "Shoulder.R",
        "UpperArm.L", "UpperArm.R", "LowerArm.L", "LowerArm.R",
        "Hand.L", "Hand.R", "UpperLeg.L", "UpperLeg.R",
        "LowerLeg.L", "LowerLeg.R", "LeftArmIK", "MMDShadowHelper",
    ):
        bone = armature.data.edit_bones.new(name)
        bone.head = (0.0, 0.0, 0.0)
        bone.tail = (0.0, 0.0, 0.2)
    edit_bones = armature.data.edit_bones
    # Model the imported neutral stance as a T-pose.  The idle initializer must
    # lower it anatomically instead of assuming a particular local Euler axis.
    t_pose = {
        "Shoulder.L": ((0.0, 0.0, 1.2), (0.2, 0.0, 1.2)),
        "UpperArm.L": ((0.2, 0.0, 1.2), (0.5, 0.0, 1.2)),
        "LowerArm.L": ((0.5, 0.0, 1.2), (0.8, 0.0, 1.2)),
        "Hand.L": ((0.8, 0.0, 1.2), (0.95, 0.0, 1.2)),
        "Shoulder.R": ((0.0, 0.0, 1.2), (-0.2, 0.0, 1.2)),
        "UpperArm.R": ((-0.2, 0.0, 1.2), (-0.5, 0.0, 1.2)),
        "LowerArm.R": ((-0.5, 0.0, 1.2), (-0.8, 0.0, 1.2)),
        "Hand.R": ((-0.8, 0.0, 1.2), (-0.95, 0.0, 1.2)),
        "UpperLeg.L": ((0.12, 0.0, 0.9), (0.12, 0.0, 0.5)),
        "LowerLeg.L": ((0.12, 0.0, 0.5), (0.12, 0.0, 0.1)),
        "UpperLeg.R": ((-0.12, 0.0, 0.9), (-0.12, 0.0, 0.5)),
        "LowerLeg.R": ((-0.12, 0.0, 0.5), (-0.12, 0.0, 0.1)),
    }
    for name, (head, tail) in t_pose.items():
        edit_bones[name].head = head
        edit_bones[name].tail = tail
    for parent_name, child_name in (
        ("UpperArm.L", "LowerArm.L"),
        ("LowerArm.L", "Hand.L"),
        ("UpperArm.R", "LowerArm.R"),
        ("LowerArm.R", "Hand.R"),
        ("UpperLeg.L", "LowerLeg.L"),
        ("UpperLeg.R", "LowerLeg.R"),
    ):
        edit_bones[child_name].parent = edit_bones[parent_name]
    bpy.ops.object.mode_set(mode="OBJECT")
    armature.data.bones["LeftArmIK"].use_deform = False
    armature.data.bones["MMDShadowHelper"].use_deform = False
    japanese_names = {
        "PmxHeadInternal": "頭",
        "UpperBody": "上半身",
        "UpperBody2": "上半身2",
        "Neck": "首",
        "Shoulder.L": "左肩",
        "Shoulder.R": "右肩",
        "UpperArm.L": "左腕",
        "UpperArm.R": "右腕",
        "LowerArm.L": "左ひじ",
        "LowerArm.R": "右ひじ",
        "Hand.L": "左手首",
        "Hand.R": "右手首",
        "UpperLeg.L": "左足",
        "UpperLeg.R": "右足",
        "LowerLeg.L": "左ひざ",
        "LowerLeg.R": "右ひざ",
        "LeftArmIK": "左腕ＩＫ",
        "MMDShadowHelper": "左腕補助",
    }
    for bone_id, (name, name_j) in enumerate(japanese_names.items()):
        pose_bone = armature.pose.bones[name]
        pose_bone.mmd_bone.name_j = name_j
        pose_bone.mmd_bone.name_e = name
        pose_bone.mmd_bone.bone_id = bone_id
    armature.pose.bones["UpperBody2"].mmd_bone.has_additional_rotation = True
    armature.pose.bones["UpperBody2"].mmd_bone.additional_transform_bone = "UpperBody"
    armature.pose.bones["UpperBody2"].mmd_bone.additional_transform_influence = 0.45
    armature.pose.bones["LowerLeg.L"].mmd_bone.enabled_fixed_axis = True
    armature.pose.bones["LowerLeg.L"].mmd_bone.fixed_axis = (1.0, 0.0, 0.0)
    armature.pose.bones["MMDShadowHelper"].is_mmd_shadow_bone = True
    armature.pose.bones["MMDShadowHelper"].mmd_shadow_bone_type = "SHADOW"
    armature.pose.bones["MMDShadowHelper"].mmd_bone.is_controllable = False
    constraint = armature.pose.bones["LowerArm.L"].constraints.new("IK")
    constraint.target = armature
    constraint.subtarget = "LeftArmIK"
    constraint.chain_count = 2
    constraint.influence = 1.0
    armature.pose.bones["LeftArmIK"].mmd_ik_toggle = True
    return armature


bridge = load_bridge()


def verify_transport_limit() -> None:
    package = ModuleType("virtual_c_blender_addon")
    dispatcher = ModuleType("virtual_c_blender_addon.dispatcher")
    dispatcher.COMMANDS = {}
    wire = ModuleType("virtual_c_blender_addon.wire")
    wire.MAX_FRAME_BYTES = 4 * 1024 * 1024
    previous = {
        name: sys.modules.get(name)
        for name in (
            package.__name__, dispatcher.__name__, wire.__name__
        )
    }
    sys.modules[package.__name__] = package
    sys.modules[dispatcher.__name__] = dispatcher
    sys.modules[wire.__name__] = wire
    try:
        resolved = bridge._configure_transport_frame_limit(dispatcher)
        assert resolved == 32 * 1024 * 1024
        assert wire.MAX_FRAME_BYTES == resolved
    finally:
        for name, module in previous.items():
            if module is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module


def verify_dispatcher_selection() -> None:
    candidate_suffixes = (
        "virtual_c_blender_addon.dispatcher",
        "virtual_c_blender_addon.server",
    )
    previous_candidates = {
        name: module
        for name, module in tuple(sys.modules.items())
        if name.endswith(candidate_suffixes)
    }
    for name in previous_candidates:
        sys.modules.pop(name, None)

    extension_package = "bl_ext.user_default.virtual_c_blender_addon"
    direct_package = "virtual_c_blender_addon"
    extension_dispatcher = ModuleType(f"{extension_package}.dispatcher")
    extension_dispatcher.COMMANDS = {}
    extension_server = ModuleType(f"{extension_package}.server")
    extension_server.runtime = type("Runtime", (), {"running": False})()
    direct_dispatcher = ModuleType(f"{direct_package}.dispatcher")
    direct_dispatcher.COMMANDS = {}
    direct_server = ModuleType(f"{direct_package}.server")
    direct_server.runtime = type("Runtime", (), {"running": True})()
    injected = {
        extension_dispatcher.__name__: extension_dispatcher,
        extension_server.__name__: extension_server,
        direct_dispatcher.__name__: direct_dispatcher,
        direct_server.__name__: direct_server,
    }
    sys.modules.update(injected)
    try:
        assert bridge._find_dispatcher() is direct_dispatcher
        direct_server.runtime.running = False
        assert bridge._find_dispatcher() is extension_dispatcher
    finally:
        for name in injected:
            sys.modules.pop(name, None)
        sys.modules.update(previous_candidates)


def run_scheduled_animation_finish() -> None:
    timer = bridge._animation_finish_timer
    if timer is None:
        return
    if bpy.app.timers.is_registered(timer):
        bpy.app.timers.unregister(timer)
    timer()


def verify_payload(payload: dict) -> None:
    png = base64.b64decode(payload["image_base64"], validate=True)
    assert png.startswith(b"\x89PNG\r\n\x1a\n")
    assert payload["width"] == 128
    assert payload["height"] == 128
    assert payload["disk_cache"] is False
    offset = 8
    compressed = bytearray()
    while offset < len(png):
        chunk_size = struct.unpack(">I", png[offset : offset + 4])[0]
        chunk_type = png[offset + 4 : offset + 8]
        chunk_data = png[offset + 8 : offset + 8 + chunk_size]
        if chunk_type == b"IDAT":
            compressed.extend(chunk_data)
        offset += 12 + chunk_size
        if chunk_type == b"IEND":
            break
    scanlines = zlib.decompress(bytes(compressed))
    stride = 1 + payload["width"] * 4
    assert len(scanlines) == stride * payload["height"]
    assert all(scanlines[row * stride] == 0 for row in range(payload["height"]))
    rgba = np.frombuffer(scanlines, dtype=np.uint8).reshape(
        (payload["height"], stride)
    )[:, 1:].reshape((payload["height"], payload["width"], 4))
    white_rgb = np.all(rgba[..., :3] >= 250, axis=2)
    white_rows = int(np.count_nonzero(np.all(white_rgb, axis=1)))
    white_columns = int(np.count_nonzero(np.all(white_rgb, axis=0)))
    assert white_rows < 4
    assert white_columns < 4
    print(
        "RAVICHARA_PREVIEW_TEST="
        + json.dumps(
            {
                "capture_mode": payload["capture_mode"],
                "bytes": payload["bytes"],
                "width": payload["width"],
                "height": payload["height"],
                "disk_cache": payload["disk_cache"],
                "white_rows": white_rows,
                "white_columns": white_columns,
            },
            sort_keys=True,
        )
    )


def verify_animation_lifecycle() -> None:
    status = bridge._preview_status({})
    assert status["version"] == "0.5.5"
    assert status["pixel_transport"] == "ephemeral-png-auto-delete"
    assert status["temporary_render_file"]["retained"] is False
    assert status["temporary_render_file"]["deleted_after_read"] is True
    assert bridge._virtual_c_expression_preset("happy") == "joy"
    assert bridge._virtual_c_expression_preset("shy") == "smile"
    assert bridge._virtual_c_expression_preset("wink") == "wink_left"
    armature = build_test_scene()
    idle = bridge._set_idle_animation({
        "armature": "RaViCharaTestArmature",
        "profile_id": "test:gentle",
        "duration_frames": 64,
        "sway_degrees": 1.2,
        "breath_degrees": 1.0,
        "head_degrees": 0.8,
        "arm_drop_degrees": 14.0,
    })
    assert idle["lifecycle"] == "persona-idle-loop"
    idle_action = armature.animation_data.action
    assert idle_action is not None
    assert bpy.context.scene.frame_end == 65
    bpy.context.scene.frame_set(1)
    bpy.context.view_layer.update()
    idle_mapped = bridge._discover_motion_bones(armature)
    idle_axes = bridge._character_axes(idle_mapped)
    right_idle_direction = idle_mapped["upper_arm_right"].vector.normalized()
    assert right_idle_direction.dot(-idle_axes["up"]) > 0.78
    assert right_idle_direction.dot(-idle_axes["side"]) > 0.05
    assert abs(right_idle_direction.dot(idle_axes["forward"])) < 0.25
    parameters = {
        "armature": "RaViCharaTestArmature",
        "preset": "nod",
        "playback_start": 1,
        "action_start": 3,
        "action_end": 7,
        "playback_end": 9,
        "transition_frames": 2,
        "intensity": 1.0,
    }
    bridge._preview_frame_cache = {"key": ("stale",), "payload": {}}
    motion = bridge._play_motion_once(parameters)
    assert bridge._preview_frame_cache is None
    assert motion["lifecycle"] == "one-shot-idle-return"
    assert motion["playback_range"] == [1, 9]
    assert motion["action_range"] == [3, 7]
    bpy.context.scene.frame_set(9)
    bridge._animation_frame_change(bpy.context.scene)
    run_scheduled_animation_finish()
    assert bridge._animation_session["active"] is False
    assert bpy.context.scene.frame_start == 1
    assert bpy.context.scene.frame_end == 65
    assert bpy.context.scene.frame_current == 1
    assert armature.animation_data.action == idle_action

    capabilities = bridge._avatar_capabilities({
        "model": "RaViCharaTestModel",
        "armature": "RaViCharaTestArmature",
    })
    assert capabilities["available"] is True
    assert capabilities["protocol_version"] == 4
    assert capabilities["bone_roles"]["head"]["bone"] == "PmxHeadInternal"
    assert "upper_arm_left" not in capabilities["bone_roles"]
    assert "forearm_left" not in capabilities["bone_roles"]
    assert capabilities["control_roles"]["hand_left"]["bone"] == "LeftArmIK"
    assert capabilities["rig_profile"]["limbs"]["arm_left"]["active_channel"] == "ik"
    assert capabilities["rig_profile"]["limbs"]["arm_left"]["mode"] == "hybrid"
    assert capabilities["rig_profile"]["version"] == 3
    assert capabilities["rig_profile"]["mmd_detected"] is True
    assert "mmd_tools" in capabilities["rig_profile"]["adapters"]
    additional_transforms = capabilities["rig_profile"]["additional_transforms"]
    assert len(additional_transforms) == 1
    additional = additional_transforms[0]
    assert additional["bone"] == "UpperBody2"
    assert additional["source"] == "UpperBody"
    assert additional["rotation"] is True
    assert additional["location"] is False
    assert abs(additional["influence"] - 0.45) < 1e-5
    assert additional["source_resolved"] is True
    assert capabilities["bone_roles"]["knee_left"]["drive_mode"] == "mmd_fixed_axis"
    assert capabilities["bone_roles"]["knee_left"]["max_degrees"][1:] == [0.0, 0.0]
    assert all(
        item["name"] != "MMDShadowHelper"
        for item in capabilities["bones"]
    )
    structural_signature = capabilities["rig_profile"]["signature"]
    armature.pose.bones["LeftArmIK"].mmd_ik_toggle = False
    fk_capabilities = bridge._avatar_capabilities({
        "model": "RaViCharaTestModel",
        "armature": "RaViCharaTestArmature",
    })
    assert fk_capabilities["rig_profile"]["signature"] == structural_signature
    assert fk_capabilities["rig_profile"]["state_signature"] != capabilities["rig_profile"]["state_signature"]
    assert fk_capabilities["rig_profile"]["limbs"]["arm_left"]["active_channel"] == "fk"
    assert "forearm_left" in fk_capabilities["bone_roles"]
    assert fk_capabilities["bone_roles"]["upper_arm_left"]["rotation_space"] == (
        "character-semantic-degrees"
    )
    assert fk_capabilities["bone_roles"]["forearm_left"]["rotation_axes"] == [
        "forward_elbow_bend", "outward_bias", "axial_twist",
    ]
    assert "hand_left" not in fk_capabilities["control_roles"]
    armature.pose.bones["LeftArmIK"].mmd_ik_toggle = True
    left_arm_constraint = armature.pose.bones["LowerArm.L"].constraints["IK"]
    left_arm_constraint.influence = 0.35
    mixed_capabilities = bridge._avatar_capabilities({
        "model": "RaViCharaTestModel",
        "armature": "RaViCharaTestArmature",
    })
    assert mixed_capabilities["rig_profile"]["limbs"]["arm_left"]["active_channel"] == "mixed"
    assert mixed_capabilities["rig_profile"]["limbs"]["arm_left"]["safe"] is False
    assert "upper_arm_left" not in mixed_capabilities["bone_roles"]
    assert "hand_left" not in mixed_capabilities["control_roles"]
    assert len(mixed_capabilities["rig_profile"]["safety"]["partially_blended_ik"]) == 1
    left_arm_constraint.influence = 1.0
    armature.pose.bones["LeftArmIK"].mmd_ik_toggle = True
    assert any(item["name"] == "SmileCustom" for item in capabilities["morphs"])
    bridge._preview_frame_cache = {"key": ("stale",), "payload": {}}
    generated = bridge._play_behavior_plan({
        "model": "RaViCharaTestModel",
        "armature": "RaViCharaTestArmature",
        "intent": "gentle attentive acknowledgement",
        "hold_seconds": 2.0,
        "playback_start": 1,
        "action_start": 3,
        "action_end": 7,
        "playback_end": 9,
        "transition_frames": 2,
        "easing": "SINE",
        "keyframes": [
            {
                "at": 0.25,
                "rotations": [
                    {"role": "head", "degrees": [5.0, 4.0, 7.0]},
                    {"bone": "UpperBody", "degrees": [2.0, 0.0, 3.0]},
                    {"role": "knee_left", "degrees": [8.0, 0.0, 0.0]},
                ],
                "translations": [
                    {"role": "hand_left", "offset": [0.25, 0.1, 0.3]},
                ],
            },
            {
                "at": 0.75,
                "rotations": [
                    {"role": "head", "degrees": [-2.0, -3.0, 2.0]},
                    {"bone": "UpperBody", "degrees": [0.0, 0.0, -2.0]},
                ],
            },
        ],
        "morphs": [{"name": "SmileCustom", "weight": 0.6}],
    })
    assert bridge._preview_frame_cache is None
    assert generated["lifecycle"] == "generated-one-shot-idle-return"
    assert generated["action"] == "RaViChara_BehaviorAction"
    assert len(generated["resolved_targets"]) == 4
    assert any(
        item.get("control_role") == "hand_left"
        and item.get("bone") == "LeftArmIK"
        for item in generated["resolved_targets"]
    )
    assert any(
        item.get("drive_mode") == "mmd_fixed_axis"
        for item in generated["resolved_targets"]
    )
    assert abs(
        bpy.data.objects["HairMesh"].data.shape_keys.key_blocks["SmileCustom"].value
        - 0.6
    ) < 1e-5
    bpy.context.scene.frame_set(9)
    bridge._animation_frame_change(bpy.context.scene)
    run_scheduled_animation_finish()
    assert bridge._animation_session["active"] is False
    assert armature.animation_data.action == idle_action
    if bpy.app.timers.is_registered(bridge._behavior_morph_timer):
        bpy.app.timers.unregister(bridge._behavior_morph_timer)
    bridge._behavior_morph_restore()
    assert abs(
        bpy.data.objects["HairMesh"].data.shape_keys.key_blocks["SmileCustom"].value
    ) < 1e-5

    # Blender can wrap from the final visible frame directly to the start
    # before a post-change handler observes playback_end. The handler must
    # treat this decreasing frame sequence as one-shot completion.
    wrapped = bridge._play_motion_once(parameters)
    bridge._animation_session["last_frame"] = 8
    bpy.context.scene.frame_set(1)
    bridge._animation_frame_change(bpy.context.scene)
    run_scheduled_animation_finish()
    assert bridge._animation_session["active"] is False
    assert wrapped["lifecycle"] == "one-shot-idle-return"

    stopped = bridge._stop_animation({})
    assert stopped["animation_active"] is False
    assert stopped["lifecycle"] == "persona-idle-loop"

    for preset in (
        "wave", "nod", "walk", "bow", "head_tilt", "shake_head", "shrug",
        "kick", "raise_hand_right",
    ):
        result = bridge._play_motion_once({**parameters, "preset": preset})
        assert result["preset"] == preset
        bridge._stop_animation({})
    try:
        bridge._play_motion_once({**parameters, "preset": "raise_hand_left"})
    except RuntimeError as error:
        assert error.args[0] == "motion_incompatible_with_active_ik"
    else:
        raise AssertionError("FK-only fallback must not drive an active IK chain")
    armature.pose.bones["LeftArmIK"].mmd_ik_toggle = False
    left_arm_constraint.influence = 0.0
    fk_left_raise = bridge._play_motion_once({
        **parameters,
        "preset": "raise_hand_left",
    })
    assert fk_left_raise["preset"] == "raise_hand_left"
    bridge._stop_animation({})

    material_result = bridge._adjust_material({
        "model": "RaViCharaTestModel",
        "target": "hair",
        "brightness": 1.18,
    })
    assert material_result["changed_material_count"] == 1
    assert material_result["original_materials_preserved"] is True
    print(
        "RAVICHARA_ANIMATION_TEST="
        + json.dumps(
            {
                "lifecycle": motion["lifecycle"],
                "playback_range": motion["playback_range"],
                "action_range": motion["action_range"],
                "active_after_end": bridge._animation_session["active"],
                "wrap_detection": True,
                "explicit_stop": stopped,
                "idle": idle,
                "motion_presets": [
                    "wave", "nod", "walk", "bow", "head_tilt", "shake_head",
                    "shrug", "kick", "raise_hand_left", "raise_hand_right",
                ],
                "generated_behavior": {
                    "capabilities": capabilities,
                    "result": generated,
                    "morph_restored": True,
                },
                "material": material_result,
            },
            sort_keys=True,
        )
    )


def verify_live_viewport() -> None:
    try:
        verify_animation_lifecycle()
        payload = bridge._render_viewport(
            {
                "width": 128,
                "height": 128,
                "transparent": False,
            }
        )
        verify_payload(payload)
    finally:
        bpy.ops.wm.quit_blender()


if bpy.app.background:
    verify_transport_limit()
    verify_dispatcher_selection()
    verify_animation_lifecycle()
    rgba = bridge.np.zeros((128, 128, 4), dtype=bridge.np.uint8)
    rgba[..., 0] = 64
    rgba[..., 1] = 128
    rgba[..., 2] = 192
    rgba[..., 3] = 255
    image_bytes = bridge._rgba8_to_png_bytes(rgba)
    verify_payload(
        bridge._frame_payload(
            image_bytes,
            128,
            128,
            "encoder-test",
            True,
        )
    )
    bpy.ops.wm.quit_blender()
else:
    bpy.app.timers.register(verify_live_viewport, first_interval=1.0)
