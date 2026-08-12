"""Read-only pose-space diagnostics for the primary armature in a real scene.

This utility never saves the loaded file.  It is intentionally kept beside the
bridge verification scripts because PMX imports differ substantially in bone
roll, rest pose, and IK control orientation.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import bpy
from mathutils import Matrix


def load_bridge():
    source = Path(__file__).with_name("ravichara_preview_bridge") / "__init__.py"
    spec = importlib.util.spec_from_file_location(
        "ravichara_preview_bridge_pose_inspection",
        source,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load preview bridge from {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def primary_armature():
    armatures = [obj for obj in bpy.data.objects if obj.type == "ARMATURE"]
    if not armatures:
        raise RuntimeError("The scene contains no armature")
    return max(
        armatures,
        key=lambda item: (
            sum(1 for bone in item.data.bones if bone.use_deform),
            len(item.data.bones),
        ),
    )


def vec(value):
    return [round(float(component), 6) for component in value]


def main() -> None:
    bridge = load_bridge()
    armature = primary_armature()
    mapped = bridge._discover_motion_bones(armature)
    profile = bridge._rig_profile(armature, mapped)
    roles = {}
    for role in (
        "upper_body",
        "upper_body2",
        "shoulder_left",
        "shoulder_right",
        "upper_arm_left",
        "upper_arm_right",
        "forearm_left",
        "forearm_right",
        "hand_left",
        "hand_right",
        "upper_leg_left",
        "upper_leg_right",
        "knee_left",
        "knee_right",
        "foot_left",
        "foot_right",
    ):
        bone = mapped.get(role)
        if bone is None:
            continue
        roles[role] = {
            "name": bone.name,
            "head": vec(bone.head),
            "tail": vec(bone.tail),
            "vector": vec(bone.vector.normalized()),
            "rest_head": vec(bone.bone.head_local),
            "rest_tail": vec(bone.bone.tail_local),
            "rest_vector": vec(bone.bone.vector.normalized()),
            "rotation_mode": str(bone.rotation_mode),
            "matrix_basis_quaternion": vec(
                bone.matrix_basis.to_quaternion().normalized()
            ),
        }

    controls = {}
    for role, specification in profile.get("control_roles", {}).items():
        bone = armature.pose.bones.get(specification.get("bone", ""))
        if bone is None:
            continue
        original = bone.location.copy()
        baseline = bone.head.copy()
        scale = max(float(specification.get("offset_scale", 0.05)), 0.05)
        columns = []
        for axis in range(3):
            bone.location = original.copy()
            bone.location[axis] += scale * 0.05
            bpy.context.view_layer.update()
            columns.append(vec((bone.head - baseline) / (scale * 0.05)))
        bone.location = original
        bpy.context.view_layer.update()
        parent_matrix = bone.parent.matrix if bone.parent else Matrix.Identity(4)
        parent_rest = (
            bone.parent.bone.matrix_local if bone.parent else Matrix.Identity(4)
        )
        computed = bone.bone.convert_local_to_pose(
            bone.matrix_basis,
            bone.bone.matrix_local,
            parent_matrix=parent_matrix,
            parent_matrix_local=parent_rest,
        )
        computed_columns = []
        for axis in range(3):
            basis = bone.matrix_basis.copy()
            basis.translation[axis] += scale * 0.05
            perturbed = bone.bone.convert_local_to_pose(
                basis,
                bone.bone.matrix_local,
                parent_matrix=parent_matrix,
                parent_matrix_local=parent_rest,
            )
            computed_columns.append(
                vec((perturbed.translation - computed.translation) / (scale * 0.05))
            )
        controls[role] = {
            "name": bone.name,
            "parent": bone.parent.name if bone.parent else None,
            "location": vec(original),
            "head": vec(bone.head),
            "pose_channel_jacobian": columns,
            "computed_jacobian": computed_columns,
            "computed_pose_error": vec(computed.translation - bone.matrix.translation),
            "specification": specification,
        }

    print(
        "RAVICHARA_REAL_POSE="
        + json.dumps(
            {
                "blend": bpy.data.filepath,
                "armature": armature.name,
                "roles": roles,
                "controls": controls,
                "limbs": profile.get("limbs", {}),
            },
            ensure_ascii=True,
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
