"""Read-only integration test for a real Blender scene and RigProfile v3.

Run Blender with an existing .blend followed by this script. The test creates a
small temporary in-memory Action only through channels advertised by the loaded
rig, finishes it immediately, verifies the original animation state is restored,
and never saves the file.
"""

from __future__ import annotations

import importlib.util
import json
from math import degrees
from pathlib import Path

import bpy


def load_bridge():
    source = Path(__file__).with_name("ravichara_preview_bridge") / "__init__.py"
    spec = importlib.util.spec_from_file_location(
        "ravichara_preview_bridge_real_scene_verification",
        source,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load preview bridge from {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def top_level_root(obj):
    current = obj
    while current.parent is not None:
        current = current.parent
    return current


def descendant_mesh_count(root) -> int:
    return sum(
        1
        for obj in (root, *tuple(root.children_recursive))
        if obj.type == "MESH"
    )


def primary_armature():
    armatures = [obj for obj in bpy.data.objects if obj.type == "ARMATURE"]
    if not armatures:
        raise AssertionError("The scene contains no armature")
    return max(
        armatures,
        key=lambda armature: (
            descendant_mesh_count(top_level_root(armature)),
            sum(1 for bone in armature.data.bones if bone.use_deform),
            len(armature.data.bones),
        ),
    )


def bounded_rotation(capabilities: dict) -> tuple[dict | None, str | None]:
    preferred = (
        "head",
        "upper_body",
        "upper_body2",
        "neck",
        "shoulder_left",
        "shoulder_right",
    )
    roles = capabilities.get("bone_roles", {})
    role = next((name for name in preferred if name in roles), None)
    if role is None and roles:
        role = sorted(roles)[0]
    if role is None:
        return None, None
    limits = roles[role].get("max_degrees", [0.0, 0.0, 0.0])
    degrees = [0.0, 0.0, 0.0]
    for axis, limit in enumerate(limits):
        if float(limit) > 0.0:
            degrees[axis] = min(2.0, float(limit) * 0.05)
            break
    if not any(degrees):
        return None, None
    return {"role": role, "degrees": degrees}, role


def bounded_ik_translation(capabilities: dict) -> tuple[dict | None, str | None]:
    roles = capabilities.get("control_roles", {})
    role = next((name for name in sorted(roles) if name != "center"), None)
    if role is None:
        return None, None
    limits = roles[role].get("normalized_offset_limit", [0.0, 0.0, 0.0])
    offset = [0.0, 0.0, 0.0]
    axes = roles[role].get("normalized_axes", [])
    if axes == ["character_left", "character_forward", "character_up"]:
        offset[1] = min(0.42, float(limits[1]) * 0.55)
        offset[2] = min(0.28, float(limits[2]) * 0.55)
    else:
        for axis, limit in enumerate(limits):
            if float(limit) > 0.0:
                offset[axis] = min(0.02, float(limit) * 0.05)
                break
    if not any(offset):
        return None, None
    return {"role": role, "offset": offset}, role


def main() -> None:
    bridge = load_bridge()
    armature = primary_armature()
    root = top_level_root(armature)
    if descendant_mesh_count(root) == 0:
        raise AssertionError(f"Model root '{root.name}' has no mesh descendants")

    capabilities = bridge._avatar_capabilities({
        "model": root.name,
        "armature": armature.name,
    })
    profile = capabilities["rig_profile"]
    assert capabilities["protocol_version"] == 4
    assert profile["version"] == 3

    rotation, rotation_role = bounded_rotation(capabilities)
    translation, control_role = bounded_ik_translation(capabilities)
    if rotation is None and translation is None:
        raise AssertionError("The real rig advertises no safe behavior channel")

    keyframe_a = {"at": 0.30, "rotations": [], "translations": []}
    keyframe_b = {"at": 0.70, "rotations": [], "translations": []}
    if rotation is not None:
        keyframe_a["rotations"].append(rotation)
        keyframe_b["rotations"].append({
            **rotation,
            "degrees": [-value for value in rotation["degrees"]],
        })
    if translation is not None:
        keyframe_a["translations"].append(translation)
        keyframe_b["translations"].append({
            **translation,
            "offset": [-value for value in translation["offset"]],
        })

    scene = bpy.context.scene
    original_frame = scene.frame_current
    original_range = (scene.frame_start, scene.frame_end)
    original_action = (
        armature.animation_data.action
        if armature.animation_data is not None
        else None
    )
    original_action_name = original_action.name if original_action else None
    control_bone = None
    control_baseline = None
    if control_role is not None:
        control_spec = capabilities["control_roles"][control_role]
        control_bone = armature.pose.bones.get(control_spec.get("bone", ""))
        if control_bone is not None:
            control_baseline = control_bone.head.copy()

    result = bridge._play_behavior_plan({
        "model": root.name,
        "armature": armature.name,
        "intent": "real-scene bounded channel verification",
        "playback_start": 1,
        "action_start": 3,
        "action_end": 7,
        "playback_end": 9,
        "transition_frames": 2,
        "easing": "SINE",
        "keyframes": [keyframe_a, keyframe_b],
        "morphs": [],
    })
    temporary_action = bridge._animation_session.get("live_action")
    temporary_action_name = temporary_action.name if temporary_action else None
    semantic_displacement = None
    if control_bone is not None and control_baseline is not None:
        scene.frame_set(4)
        bpy.context.view_layer.update()
        axes = bridge._character_axes(bridge._discover_motion_bones(armature))
        displacement = control_bone.head - control_baseline
        semantic_displacement = {
            "left": float(displacement.dot(axes["side"])),
            "forward": float(displacement.dot(axes["forward"])),
            "up": float(displacement.dot(axes["up"])),
            "length": float(displacement.length),
        }
        assert semantic_displacement["forward"] > 0.05
        assert semantic_displacement["up"] > 0.03
    bridge._finish_animation(scene)

    restored_action = (
        armature.animation_data.action
        if armature.animation_data is not None
        else None
    )
    action_restored = restored_action is original_action
    range_restored = (scene.frame_start, scene.frame_end) == original_range
    frame_restored = scene.frame_current == original_frame
    temporary_action_removed = (
        temporary_action_name is None or temporary_action_name not in bpy.data.actions
    )
    assert action_restored
    assert range_restored
    assert frame_restored
    assert temporary_action_removed

    idle = bridge._set_idle_animation({
        "armature": armature.name,
        "profile_id": "real-scene:a-t-rest-test",
        "duration_frames": 64,
        "sway_degrees": 1.0,
        "breath_degrees": 1.0,
        "head_degrees": 0.8,
        "arm_drop_degrees": 16.0,
    })
    scene.frame_set(1)
    bpy.context.view_layer.update()
    mapped = bridge._discover_motion_bones(armature)
    character_axes = bridge._character_axes(mapped)
    arm_alignment = {}
    for role, side_sign in (("upper_arm_left", 1.0), ("upper_arm_right", -1.0)):
        bone = mapped.get(role)
        if bone is None:
            continue
        direction = bone.vector.normalized()
        arm_alignment[role] = {
            "down": float(direction.dot(-character_axes["up"])),
            "outward": float(direction.dot(character_axes["side"]) * side_sign),
            "forward": float(direction.dot(character_axes["forward"])),
        }
        assert arm_alignment[role]["down"] > 0.78
        assert arm_alignment[role]["outward"] > 0.05
        assert abs(arm_alignment[role]["forward"]) < 0.25
    if len(arm_alignment) == 2:
        assert abs(
            arm_alignment["upper_arm_left"]["down"]
            - arm_alignment["upper_arm_right"]["down"]
        ) < 0.08

    raised = bridge._play_motion_once({
        "armature": armature.name,
        "preset": "raise_hand_right",
        "playback_start": 1,
        "action_start": 3,
        "action_end": 7,
        "playback_end": 9,
        "transition_frames": 2,
        "intensity": 1.0,
    })
    scene.frame_set(4)
    bpy.context.view_layer.update()
    right_arm = bridge._discover_motion_bones(armature).get("upper_arm_right")
    right_forearm = bridge._discover_motion_bones(armature).get("forearm_right")
    right_hand = bridge._discover_motion_bones(armature).get("hand_right")
    right_direction = right_arm.vector.normalized()
    forearm_direction = right_forearm.vector.normalized()
    hand_direction = right_hand.vector.normalized()
    raised_right_alignment = {
        "up": float(right_direction.dot(character_axes["up"])),
        "outward": float(right_direction.dot(-character_axes["side"])),
        "forward": float(right_direction.dot(character_axes["forward"])),
        "forearm_forward": float(
            forearm_direction.dot(character_axes["forward"])
        ),
        "hand_forward": float(hand_direction.dot(character_axes["forward"])),
        "elbow_bend_degrees": float(
            degrees(right_direction.angle(forearm_direction))
        ),
    }
    assert raised_right_alignment["up"] > 0.65
    assert raised_right_alignment["outward"] > 0.35
    assert raised_right_alignment["forward"] > 0.0
    assert raised_right_alignment["forearm_forward"] > 0.08
    assert raised_right_alignment["hand_forward"] > -0.20
    assert 8.0 < raised_right_alignment["elbow_bend_degrees"] < 100.0
    bridge._finish_animation(scene)

    generated_right_alignment = None
    semantic_roles = capabilities["bone_roles"]
    if all(
        semantic_roles.get(role, {}).get("rotation_space")
        == "character-semantic-degrees"
        for role in ("upper_arm_right", "forearm_right", "hand_right")
    ):
        generated = bridge._play_behavior_plan({
            "model": root.name,
            "armature": armature.name,
            "intent": "semantic right-arm chain regression",
            "playback_start": 1,
            "action_start": 3,
            "action_end": 9,
            "playback_end": 11,
            "transition_frames": 2,
            "easing": "SINE",
            "keyframes": [
                {
                    "at": 0.2,
                    "rotations": [
                        {"role": "upper_arm_right", "degrees": [12, 96, 0]},
                        {"role": "forearm_right", "degrees": [42, 0, 0]},
                        {"role": "hand_right", "degrees": [4, -22, 0]},
                    ],
                },
                {
                    "at": 0.5,
                    "rotations": [
                        {"role": "upper_arm_right", "degrees": [16, 112, 0]},
                        {"role": "forearm_right", "degrees": [50, 0, 0]},
                        {"role": "hand_right", "degrees": [4, 28, 0]},
                    ],
                },
                {
                    "at": 0.8,
                    "rotations": [
                        {"role": "upper_arm_right", "degrees": [12, 96, 0]},
                        {"role": "forearm_right", "degrees": [42, 0, 0]},
                        {"role": "hand_right", "degrees": [4, -22, 0]},
                    ],
                },
            ],
            "morphs": [],
        })
        scene.frame_set(6)
        bpy.context.view_layer.update()
        generated_mapped = bridge._discover_motion_bones(armature)
        generated_arm = generated_mapped["upper_arm_right"].vector.normalized()
        generated_forearm = generated_mapped["forearm_right"].vector.normalized()
        generated_hand = generated_mapped["hand_right"].vector.normalized()
        generated_right_alignment = {
            "result": generated,
            "arm_up": float(generated_arm.dot(character_axes["up"])),
            "arm_outward": float(generated_arm.dot(-character_axes["side"])),
            "arm_forward": float(generated_arm.dot(character_axes["forward"])),
            "forearm_forward": float(
                generated_forearm.dot(character_axes["forward"])
            ),
            "hand_forward": float(generated_hand.dot(character_axes["forward"])),
            "elbow_bend_degrees": float(
                degrees(generated_arm.angle(generated_forearm))
            ),
        }
        assert generated_right_alignment["arm_up"] > 0.40
        assert generated_right_alignment["arm_outward"] > 0.45
        assert generated_right_alignment["arm_forward"] > 0.0
        assert generated_right_alignment["forearm_forward"] > 0.08
        assert generated_right_alignment["hand_forward"] > -0.20
        assert 12.0 < generated_right_alignment["elbow_bend_degrees"] < 110.0
        bridge._finish_animation(scene)

    idle_session = bridge._idle_session
    bridge._stop_playback()
    armature.animation_data.action = idle_session.get("original_action")
    scene.frame_start, scene.frame_end = idle_session["original_range"]
    scene.frame_set(idle_session["original_frame"])
    idle_action = idle_session.get("action")
    if idle_action is not None and idle_action.users == 0:
        bpy.data.actions.remove(idle_action)
    bridge._idle_session = None

    preview = bridge._preview_status({})
    limbs = {
        name: {
            "mode": value.get("mode"),
            "active_channel": value.get("active_channel"),
            "safe": value.get("safe"),
            "effector": value.get("effector"),
            "selection_ambiguous": value.get("selection_ambiguous"),
        }
        for name, value in profile.get("limbs", {}).items()
    }
    output = {
        "blend": bpy.data.filepath,
        "plugin_version": list(bridge.bl_info["version"]),
        "model": root.name,
        "armature": armature.name,
        "armature_count": sum(
            1 for obj in bpy.data.objects if obj.type == "ARMATURE"
        ),
        "mesh_count": descendant_mesh_count(root),
        "bone_count": profile.get("bone_count"),
        "mmd_detected": profile.get("mmd_detected"),
        "adapters": profile.get("adapters"),
        "limbs": limbs,
        "unsafe_limbs": profile.get("safety", {}).get("unsafe_limbs", []),
        "partially_blended_ik": profile.get("safety", {}).get(
            "partially_blended_ik", []
        ),
        "unresolved_additional_transforms": profile.get("safety", {}).get(
            "unresolved_additional_transforms", []
        ),
        "rotation_role_tested": rotation_role,
        "ik_control_role_tested": control_role,
        "resolved_targets": result.get("resolved_targets", []),
        "semantic_ik_displacement": semantic_displacement,
        "idle": idle,
        "idle_arm_alignment": arm_alignment,
        "raised_right": raised,
        "raised_right_alignment": raised_right_alignment,
        "generated_right_alignment": generated_right_alignment,
        "original_action": original_action_name,
        "action_restored": action_restored,
        "range_restored": range_restored,
        "frame_restored": frame_restored,
        "temporary_action_removed": temporary_action_removed,
        "camera": preview.get("active_camera"),
        "camera_resolution": preview.get("camera_resolution"),
        "disk_cache": preview.get("disk_cache"),
        "file_saved": False,
    }
    print("RAVICHARA_REAL_SCENE_TEST=" + json.dumps(output, ensure_ascii=True))


if __name__ == "__main__":
    main()
