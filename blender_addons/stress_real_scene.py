"""Read-only PMX action/render endurance test for the RaViChara bridge."""

from __future__ import annotations

import ctypes
import importlib.util
import json
import math
from pathlib import Path
import tempfile

import bpy


def load_bridge():
    source = Path(__file__).with_name("ravichara_preview_bridge") / "__init__.py"
    spec = importlib.util.spec_from_file_location(
        "ravichara_preview_bridge_stress_verification",
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
        raise AssertionError("The scene contains no armature")
    return max(armatures, key=lambda obj: len(obj.pose.bones))


def top_level_root(obj):
    while obj.parent is not None:
        obj = obj.parent
    return obj


class PROCESS_MEMORY_COUNTERS(ctypes.Structure):
    _fields_ = [
        ("cb", ctypes.c_ulong),
        ("PageFaultCount", ctypes.c_ulong),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def working_set_bytes() -> int:
    counters = PROCESS_MEMORY_COUNTERS()
    counters.cb = ctypes.sizeof(counters)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    kernel32.GetCurrentProcess.restype = ctypes.c_void_p
    process = kernel32.GetCurrentProcess()
    psapi.GetProcessMemoryInfo.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(PROCESS_MEMORY_COUNTERS),
        ctypes.c_ulong,
    ]
    psapi.GetProcessMemoryInfo.restype = ctypes.c_int
    if not psapi.GetProcessMemoryInfo(
        process,
        ctypes.byref(counters),
        counters.cb,
    ):
        raise ctypes.WinError()
    return int(counters.WorkingSetSize)


def assert_finite_arm_chain(bridge, armature) -> None:
    mapped = bridge._discover_motion_bones(armature)
    for role in (
        "upper_arm_left", "forearm_left", "hand_left",
        "upper_arm_right", "forearm_right", "hand_right",
    ):
        bone = mapped.get(role)
        if bone is None:
            continue
        if not all(math.isfinite(float(value)) for row in bone.matrix for value in row):
            raise AssertionError(f"Non-finite transform on {role}/{bone.name}")


def main() -> None:
    bridge = load_bridge()
    armature = primary_armature()
    root = top_level_root(armature)
    scene = bpy.context.scene
    original_frame = scene.frame_current
    original_range = (scene.frame_start, scene.frame_end)
    armature.animation_data_create()
    original_action = armature.animation_data.action
    original_action_name = original_action.name if original_action is not None else None
    original_render = (
        scene.render.resolution_x,
        scene.render.resolution_y,
        scene.render.resolution_percentage,
        scene.render.film_transparent,
    )
    initial_action_count = len(bpy.data.actions)

    bridge._set_idle_animation({
        "armature": armature.name,
        "profile_id": "stress:pmx-semantic-arms",
        "duration_frames": 72,
        "sway_degrees": 1.0,
        "breath_degrees": 1.0,
        "head_degrees": 0.8,
        "arm_drop_degrees": 16.0,
    })
    memory_before_actions = working_set_bytes()
    peak_action_count = len(bpy.data.actions)
    action_cycles = 80
    for index in range(action_cycles):
        side = "left" if index % 2 else "right"
        if index % 4 == 0:
            bridge._play_motion_once({
                "armature": armature.name,
                "preset": f"raise_hand_{side}",
                "playback_start": 1,
                "action_start": 3,
                "action_end": 9,
                "playback_end": 11,
                "transition_frames": 2,
                "intensity": 1.0,
            })
        else:
            bridge._play_behavior_plan({
                "model": root.name,
                "armature": armature.name,
                "intent": f"stress semantic arm {index}",
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
                            {"role": f"upper_arm_{side}", "degrees": [12, 92, 0]},
                            {"role": f"forearm_{side}", "degrees": [38, 0, 0]},
                            {"role": f"hand_{side}", "degrees": [4, -18, 0]},
                        ],
                    },
                    {
                        "at": 0.5,
                        "rotations": [
                            {"role": f"upper_arm_{side}", "degrees": [16, 112, 0]},
                            {"role": f"forearm_{side}", "degrees": [50, 0, 0]},
                            {"role": f"hand_{side}", "degrees": [4, 26, 0]},
                        ],
                    },
                    {
                        "at": 0.8,
                        "rotations": [
                            {"role": f"upper_arm_{side}", "degrees": [12, 92, 0]},
                            {"role": f"forearm_{side}", "degrees": [38, 0, 0]},
                            {"role": f"hand_{side}", "degrees": [4, -18, 0]},
                        ],
                    },
                ],
                "morphs": [],
            })
        scene.frame_set(6)
        bpy.context.view_layer.update()
        assert_finite_arm_chain(bridge, armature)
        bridge._finish_animation(scene)
        peak_action_count = max(peak_action_count, len(bpy.data.actions))
        transient = [
            action.name
            for action in bpy.data.actions
            if action.name.startswith((
                "RaViChara_LiveAction", "RaViChara_BehaviorAction",
                "EverChara_LiveAction", "EverChara_BehaviorAction",
            ))
        ]
        if transient:
            raise AssertionError(f"Transient Action leak after cycle {index}: {transient}")
    memory_after_actions = working_set_bytes()

    render_cycles = 16
    preview_files_before = set(
        Path(tempfile.gettempdir()).glob("ravichara-preview-*.png")
    )
    scene.render.resolution_x = 288
    scene.render.resolution_y = 512
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = False
    for index in range(render_cycles + 1):
        scene.frame_set(1 + (index * 4) % 72)
        result = bridge._render_viewport({
            "width": 288,
            "height": 512,
            "transparent": False,
        })
        if result.get("capture_mode") != "stable-camera-render":
            raise AssertionError(
                f"Managed render failed at cycle {index}: {result}"
            )
        if index == 0:
            memory_after_render_warmup = working_set_bytes()
    memory_after_renders = working_set_bytes()
    preview_files_after = set(
        Path(tempfile.gettempdir()).glob("ravichara-preview-*.png")
    )
    if preview_files_after != preview_files_before:
        raise AssertionError(
            f"Temporary preview leak: {preview_files_after - preview_files_before}"
        )

    idle = bridge._idle_session
    bridge._stop_playback()
    restored_original = (
        bpy.data.actions.get(original_action_name)
        if original_action_name is not None
        else None
    )
    armature.animation_data.action = restored_original
    scene.frame_start, scene.frame_end = original_range
    scene.frame_set(original_frame)
    idle_action = idle.get("action") if idle else None
    if idle_action is not None and idle_action.users == 0:
        bpy.data.actions.remove(idle_action)
    bridge._idle_session = None
    (
        scene.render.resolution_x,
        scene.render.resolution_y,
        scene.render.resolution_percentage,
        scene.render.film_transparent,
    ) = original_render

    final_action_count = len(bpy.data.actions)
    if final_action_count > initial_action_count:
        raise AssertionError(
            f"Action count grew from {initial_action_count} to {final_action_count}"
        )
    render_growth = memory_after_renders - memory_after_render_warmup
    if render_growth > 256 * 1024 * 1024:
        raise AssertionError(
            f"Managed-render working set grew by {render_growth} bytes after warm-up"
        )
    output = {
        "blend": bpy.data.filepath,
        "action_cycles": action_cycles,
        "render_cycles_after_warmup": render_cycles,
        "initial_action_count": initial_action_count,
        "peak_action_count": peak_action_count,
        "final_action_count": final_action_count,
        "action_memory_delta_bytes": memory_after_actions - memory_before_actions,
        "render_memory_delta_after_warmup_bytes": render_growth,
        "temporary_preview_files_retained": len(
            preview_files_after - preview_files_before
        ),
        "file_saved": False,
    }
    print("RAVICHARA_STRESS_TEST=" + json.dumps(output, ensure_ascii=True))


if __name__ == "__main__":
    main()
