"""RaViChara companion extension that adds safe preview commands to Virtual_c.

The extension opens no socket of its own. It attaches two allowlisted handlers
to the already-running Virtual_c dispatcher, so authentication and main-thread
execution remain owned by the existing bridge.
"""

from __future__ import annotations

import base64
from datetime import datetime, timezone
import hashlib
import json
from math import cos, radians, sin, tan
import os
import re
import struct
import sys
import tempfile
import time
from types import ModuleType
import unicodedata
import zlib

import bpy
import numpy as np
from mathutils import Euler, Matrix, Quaternion, Vector


bl_info = {
    "name": "RaViChara Preview Bridge",
    "author": "RaViChara contributors",
    "version": (0, 5, 5),
    "blender": (4, 2, 0),
    "location": "View3D > Sidebar > RaViChara",
    "description": "Adds bounded previews and generated avatar behavior to Virtual_c",
    "category": "Interface",
}

_COMMAND_PREVIEW = "render.preview"
_COMMAND_VIEWPORT = "render.viewport"
_COMMAND_STATUS = "render.preview.status"
_COMMAND_ANIMATION_PLAY_ONCE = "ravichara.animation.play_once"
_COMMAND_ANIMATION_STOP = "ravichara.animation.stop"
_COMMAND_ANIMATION_SET_IDLE = "ravichara.animation.set_idle"
_COMMAND_EXPRESSION_TIMED = "ravichara.expression.apply_timed"
_COMMAND_MATERIAL_ADJUST = "ravichara.material.adjust"
_COMMAND_AVATAR_CAPABILITIES = "ravichara.avatar.capabilities"
_COMMAND_BEHAVIOR_PLAY_PLAN = "ravichara.behavior.play_plan"
_COMMAND_BEHAVIOR_EXECUTE = "ravichara.behavior.execute"
_dispatcher: ModuleType | None = None
_last_error = ""
_animation_session: dict | None = None
_idle_session: dict | None = None
_expression_generation = 0
_expression_timer = None
_behavior_morph_generation = 0
_behavior_morph_timer = None
_behavior_morph_restore = None
_animation_finish_timer = None
_preview_frame_cache: dict | None = None
_preview_render_active = False
_preview_temp_path: str | None = None
_latest_behavior_generation = -1
_transport_frame_limit = None

_REQUIRED_TRANSPORT_FRAME_BYTES = 32 * 1024 * 1024

_MOTION_ROLE_ALIASES = {
    "center": ("センター", "Center", "Root", "全ての親", "Hips"),
    "upper_body": ("上半身", "UpperBody", "Spine", "Chest"),
    "upper_body2": ("上半身2", "上半身２", "UpperBody2", "Spine2", "UpperChest"),
    "neck": ("首", "Neck"),
    "head": ("頭", "Head", "head"),
    "shoulder_left": ("左肩", "肩.L", "Shoulder.L", "LeftShoulder"),
    "shoulder_right": ("右肩", "肩.R", "Shoulder.R", "RightShoulder"),
    "upper_arm_left": ("左腕", "腕.L", "UpperArm.L", "LeftArm"),
    "upper_arm_right": ("右腕", "腕.R", "UpperArm.R", "RightArm"),
    "forearm_left": ("左ひじ", "左肘", "ひじ.L", "LowerArm.L", "LeftElbow"),
    "forearm_right": ("右ひじ", "右肘", "ひじ.R", "LowerArm.R", "RightElbow"),
    "hand_left": ("左手首", "手首.L", "Hand.L", "LeftWrist"),
    "hand_right": ("右手首", "手首.R", "Hand.R", "RightWrist"),
    "upper_leg_left": ("左足", "足.L", "UpperLeg.L", "LeftLeg"),
    "upper_leg_right": ("右足", "足.R", "UpperLeg.R", "RightLeg"),
    "knee_left": ("左ひざ", "左膝", "ひざ.L", "LowerLeg.L", "LeftKnee"),
    "knee_right": ("右ひざ", "右膝", "ひざ.R", "LowerLeg.R", "RightKnee"),
    "foot_left": ("左足首", "左足先", "足首.L", "Foot.L", "LeftFoot"),
    "foot_right": ("右足首", "右足先", "足首.R", "Foot.R", "RightFoot"),
    "toe_left": ("左つま先", "つま先.L", "Toe.L", "LeftToeBase"),
    "toe_right": ("右つま先", "つま先.R", "Toe.R", "RightToeBase"),
}

_BEHAVIOR_ROLE_LIMITS = {
    "center": (12.0, 18.0, 12.0),
    "upper_body": (20.0, 25.0, 20.0),
    "upper_body2": (20.0, 25.0, 20.0),
    "neck": (25.0, 35.0, 25.0),
    "head": (30.0, 45.0, 35.0),
    "shoulder_left": (30.0, 30.0, 40.0),
    "shoulder_right": (30.0, 30.0, 40.0),
    # Limb roles use character-semantic axes instead of imported local Euler
    # channels.  The larger second upper-arm limit permits a hand to pass the
    # shoulder line without exposing the PMX bone roll to the planner.
    "upper_arm_left": (100.0, 125.0, 35.0),
    "upper_arm_right": (100.0, 125.0, 35.0),
    "forearm_left": (115.0, 30.0, 30.0),
    "forearm_right": (115.0, 30.0, 30.0),
    "hand_left": (35.0, 40.0, 25.0),
    "hand_right": (35.0, 40.0, 25.0),
    "upper_leg_left": (45.0, 30.0, 30.0),
    "upper_leg_right": (45.0, 30.0, 30.0),
    "knee_left": (70.0, 15.0, 15.0),
    "knee_right": (70.0, 15.0, 15.0),
    "foot_left": (35.0, 25.0, 30.0),
    "foot_right": (35.0, 25.0, 30.0),
    "toe_left": (25.0, 15.0, 15.0),
    "toe_right": (25.0, 15.0, 15.0),
}

_SEMANTIC_ROTATION_AXES = {
    "upper_arm_left": ["front_raise", "outward_raise", "axial_twist"],
    "upper_arm_right": ["front_raise", "outward_raise", "axial_twist"],
    "forearm_left": ["forward_elbow_bend", "outward_bias", "axial_twist"],
    "forearm_right": ["forward_elbow_bend", "outward_bias", "axial_twist"],
    "hand_left": ["forward_wrist_bend", "outward_wave", "axial_twist"],
    "hand_right": ["forward_wrist_bend", "outward_wave", "axial_twist"],
}

_UNSAFE_EXACT_BONE_TOKENS = (
    "pole", "twist", "dummy", "shadow", "physics", "helper",
    "control", "controller", "補助", "捩",
)


def _find_dispatcher() -> ModuleType | None:
    candidates = []
    for module_name, module in tuple(sys.modules.items()):
        if module_name.endswith("virtual_c_blender_addon.dispatcher"):
            commands = getattr(module, "COMMANDS", None)
            if isinstance(commands, dict):
                package_name = module_name.rpartition(".")[0]
                server = sys.modules.get(f"{package_name}.server")
                runtime = getattr(server, "runtime", None)
                candidates.append(
                    (
                        bool(getattr(runtime, "running", False)),
                        module_name.startswith("bl_ext."),
                        module_name,
                        module,
                    )
                )
    if not candidates:
        return None
    # Blender extensions normally live below ``bl_ext``.  A second top-level
    # copy can nevertheless appear after manual test imports or legacy add-on
    # migration.  Binding commands to that inactive copy makes system.ping work
    # on one dispatcher while all RaViChara commands remain unavailable on the
    # listening server.  Prefer the running runtime, then the registered
    # extension namespace, with the module name as a deterministic tie-breaker.
    return max(candidates, key=lambda item: item[:3])[3]


def _configure_transport_frame_limit(dispatcher: ModuleType) -> int | None:
    """Raise Virtual_c's wire bound to the same limit as the Rust client.

    Virtual_c 0.2.x defaults to four MiB, which can reject a valid 1024 px
    PNG after base64 expansion. The encoder and decoder consult the wire
    module's global at call time, so this remains an in-process bounded change
    and does not replace or bypass its protocol validation.
    """

    module_name = str(getattr(dispatcher, "__name__", ""))
    package_name, separator, _leaf = module_name.rpartition(".")
    if not separator:
        return None
    wire = sys.modules.get(f"{package_name}.wire")
    if wire is None:
        return None
    try:
        current = int(getattr(wire, "MAX_FRAME_BYTES", 0))
    except (TypeError, ValueError):
        current = 0
    resolved = max(current, _REQUIRED_TRANSPORT_FRAME_BYTES)
    try:
        wire.MAX_FRAME_BYTES = resolved
    except Exception:
        return None
    return resolved


def _bounded_int(params: dict, name: str, default: int) -> int:
    try:
        value = int(params.get(name, default))
    except (TypeError, ValueError):
        value = default
    return max(128, min(value, 1024))


def _raise_command_error(code: str, message: str) -> None:
    error_type = getattr(_dispatcher, "CommandError", RuntimeError)
    raise error_type(code, message)


def _invalidate_preview_frame() -> None:
    global _preview_frame_cache
    _preview_frame_cache = None


def _preview_status(_params: dict) -> dict:
    scene = bpy.context.scene
    render = scene.render if scene is not None else None
    resolution_scale = (
        max(1, int(render.resolution_percentage)) / 100.0
        if render is not None
        else 1.0
    )
    camera_resolution = (
        [
            max(1, round(render.resolution_x * resolution_scale)),
            max(1, round(render.resolution_y * resolution_scale)),
        ]
        if render is not None
        else None
    )
    return {
        "service": "ravichara_preview_bridge",
        "version": "0.5.5",
        "available": True,
        "formats": ["png"],
        "capture_modes": ["camera", "stable-camera-render", "render"],
        "stream_renderer": "managed-camera-render",
        "pixel_transport": "ephemeral-png-auto-delete",
        "unsafe_offscreen_stream_disabled": True,
        "animation_modes": [
            "generated-behavior-plan",
            "one-shot",
            "idle-return",
        ],
        "disk_cache": False,
        "runtime_frame_cache": {
            "entries": 1 if _preview_frame_cache is not None else 0,
            "limit": 1,
            "storage": "process-memory",
        },
        "temporary_render_file": {
            "retained": bool(
                _preview_temp_path and os.path.isfile(_preview_temp_path)
            ),
            "lifetime": "single-render",
            "deleted_after_read": True,
        },
        "min_size": 128,
        "max_size": 1024,
        "transport_max_frame_bytes": _transport_frame_limit,
        "last_error": _last_error,
        "viewport_areas": sum(
            1
            for window in bpy.context.window_manager.windows
            for area in window.screen.areas
            if area.type == "VIEW_3D"
        ),
        "active_camera": (
            scene.camera.name
            if scene is not None and scene.camera is not None
            else None
        ),
        "camera_resolution": camera_resolution,
        "camera_pixel_aspect": (
            [render.pixel_aspect_x, render.pixel_aspect_y]
            if render is not None
            else None
        ),
        "camera_lens_mm": (
            scene.camera.data.lens
            if scene is not None
            and scene.camera is not None
            and getattr(scene.camera, "data", None) is not None
            and getattr(scene.camera.data, "type", None) == "PERSP"
            else None
        ),
        "scene_frame_range": (
            [scene.frame_start, scene.frame_end] if scene is not None else None
        ),
        "scene_frame_current": scene.frame_current if scene is not None else None,
        "animation_active": bool(
            _animation_session and _animation_session.get("active")
        ),
        "idle_active": bool(_idle_session and _idle_session.get("active")),
        "idle_profile": (
            _idle_session.get("profile_id") if _idle_session else None
        ),
    }


def _png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    checksum = zlib.crc32(data, zlib.crc32(chunk_type)) & 0xFFFFFFFF
    return (
        struct.pack(">I", len(data))
        + chunk_type
        + data
        + struct.pack(">I", checksum)
    )


def _image_to_png_bytes(image, width: int, height: int) -> bytes:
    source_width, source_height = (int(value) for value in image.size[:])
    if source_width <= 0 or source_height <= 0:
        _raise_command_error("empty_preview", "Blender produced no image pixels")

    pixels = np.empty(source_width * source_height * 4, dtype=np.float32)
    image.pixels.foreach_get(pixels)
    rgba = pixels.reshape((source_height, source_width, 4))
    if (source_width, source_height) != (width, height):
        x_indices = np.linspace(0, source_width - 1, width).astype(np.int32)
        y_indices = np.linspace(0, source_height - 1, height).astype(np.int32)
        rgba = rgba[y_indices][:, x_indices]
    rgba = np.flipud(np.clip(rgba, 0.0, 1.0))
    # Render Result stores scene-linear RGB. Apply a small display conversion so
    # the in-memory PNG resembles Blender's viewport without writing a temp file.
    rgba[..., :3] = np.power(rgba[..., :3], 1.0 / 2.2)
    return _rgba8_to_png_bytes((rgba * 255.0 + 0.5).astype(np.uint8))


def _rgba8_to_png_bytes(rgba_bytes: np.ndarray) -> bytes:
    height, width, channels = rgba_bytes.shape
    if width <= 0 or height <= 0 or channels != 4:
        _raise_command_error("empty_preview", "Invalid RGBA viewport buffer")
    raw_scanlines = b"".join(
        b"\x00" + rgba_bytes[row].tobytes() for row in range(height)
    )
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + _png_chunk(b"IHDR", header)
        + _png_chunk(b"IDAT", zlib.compress(raw_scanlines, level=1))
        + _png_chunk(b"IEND", b"")
    )


def _frame_payload(
    image_bytes: bytes,
    width: int,
    height: int,
    capture_mode: str,
    refreshed: bool,
) -> dict:
    if len(image_bytes) > 20 * 1024 * 1024:
        _raise_command_error(
            "preview_too_large",
            "Preview PNG exceeded 20 MiB; lower the requested dimensions",
        )
    return {
        "mime_type": "image/png",
        "image_base64": base64.b64encode(image_bytes).decode("ascii"),
        "width": width,
        "height": height,
        "bytes": len(image_bytes),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "capture_mode": capture_mode,
        "refreshed": refreshed,
        "disk_cache": False,
    }


def _remove_preview_temp_file() -> bool:
    global _preview_temp_path
    path = _preview_temp_path
    if not path:
        return True
    try:
        if os.path.isfile(path):
            os.remove(path)
    except OSError:
        # Keep the path visible in status and refuse to create another file on
        # the next capture, preventing an unbounded leak if the OS holds it.
        return False
    _preview_temp_path = None
    return True


def _render_with_settings(params: dict, capture_mode: str) -> dict:
    global _last_error, _preview_frame_cache, _preview_render_active
    global _preview_temp_path
    scene = bpy.context.scene
    if scene is None:
        _raise_command_error("scene_unavailable", "No active Blender scene")
    if scene.camera is None:
        _raise_command_error(
            "camera_unavailable",
            "A stable preview requires an active scene camera",
        )

    width = _bounded_int(params, "width", 512)
    height = _bounded_int(params, "height", 512)
    refresh = bool(params.get("refresh", True))
    transparent = bool(params.get("transparent", False))
    render = scene.render
    compositor_file_outputs = [
        node
        for node in (
            tuple(scene.node_tree.nodes)
            if scene.use_nodes and scene.node_tree is not None
            else ()
        )
        if node.bl_idname == "CompositorNodeOutputFile"
    ]
    original_file_output_mutes = [
        (node, bool(node.mute)) for node in compositor_file_outputs
    ]
    original = {
        "resolution_x": render.resolution_x,
        "resolution_y": render.resolution_y,
        "resolution_percentage": render.resolution_percentage,
        "film_transparent": render.film_transparent,
        "filepath": render.filepath,
        "use_file_extension": render.use_file_extension,
        "file_format": render.image_settings.file_format,
        "color_mode": render.image_settings.color_mode,
        "color_depth": render.image_settings.color_depth,
        "compression": render.image_settings.compression,
        "use_multiview": render.use_multiview,
    }
    cache_key = (
        scene.as_pointer(),
        scene.camera.as_pointer(),
        int(scene.frame_current),
        width,
        height,
        transparent,
    )
    if (
        not refresh
        and _preview_frame_cache is not None
        and _preview_frame_cache.get("key") == cache_key
    ):
        payload = dict(_preview_frame_cache["payload"])
        payload["refreshed"] = False
        payload["capture_mode"] = "stable-camera-render-cache"
        return payload
    if _preview_render_active:
        cached = _preview_frame_cache
        if cached is not None and cached.get("key", ())[3:6] == cache_key[3:6]:
            payload = dict(cached["payload"])
            payload["refreshed"] = False
            payload["capture_mode"] = "stable-camera-render-coalesced"
            return payload
        _raise_command_error(
            "preview_busy",
            "A camera preview is already being rendered",
        )
    try:
        render.resolution_x = width
        render.resolution_y = height
        render.resolution_percentage = 100
        render.film_transparent = transparent
        render.use_file_extension = True
        render.image_settings.file_format = "PNG"
        render.image_settings.color_mode = "RGBA"
        render.image_settings.color_depth = "8"
        render.image_settings.compression = 15
        render.use_multiview = False
        # File Output compositor nodes are unrelated to the UI preview and can
        # otherwise write user-configured files on every streamed frame.
        for node in compositor_file_outputs:
            node.mute = True

        # Repeated GPUOffScreen.draw_view3d calls from an application timer can
        # terminate Blender inside its OpenGL state manager. The render engine
        # therefore owns framebuffer lifetime and writes one short-lived PNG.
        # Blender 4.5 may expose an empty Render Result even after FINISHED.
        _preview_render_active = True
        stable_mode = (
            "stable-camera-render" if capture_mode == "viewport" else capture_mode
        )
        # A unique closed path avoids collisions with other Blender instances.
        if not _remove_preview_temp_file():
            _raise_command_error(
                "preview_cleanup_failed",
                "The previous temporary preview file could not be deleted",
            )
        temporary = tempfile.NamedTemporaryFile(
            prefix="ravichara-preview-",
            suffix=".png",
            delete=False,
        )
        _preview_temp_path = temporary.name
        temporary.close()
        render.filepath = _preview_temp_path
        result = bpy.ops.render.render(write_still=True)
        if "FINISHED" not in result:
            _raise_command_error(
                "render_cancelled",
                f"Blender {stable_mode} capture did not finish",
            )
        try:
            with open(_preview_temp_path, "rb") as rendered:
                image_bytes = rendered.read()
        except OSError as error:
            _raise_command_error(
                "empty_preview",
                f"Blender produced no readable preview: {error}",
            )
        if not image_bytes.startswith(b"\x89PNG\r\n\x1a\n"):
            _raise_command_error(
                "empty_preview",
                "Blender produced an invalid preview PNG",
            )
        payload = _frame_payload(
            image_bytes,
            width,
            height,
            stable_mode,
            True,
        )
        _preview_frame_cache = {
            "key": cache_key,
            "payload": dict(payload),
            "created_monotonic": time.monotonic(),
        }
        _last_error = ""
        return payload
    except Exception as error:
        _last_error = str(error)[:500]
        cached = _preview_frame_cache
        if (
            cached is not None
            and cached.get("key", ())[3:6] == cache_key[3:6]
            and time.monotonic() - float(cached.get("created_monotonic", 0.0)) <= 2.0
        ):
            payload = dict(cached["payload"])
            payload["refreshed"] = False
            payload["capture_mode"] = "stable-camera-render-stale"
            payload["render_error"] = _last_error
            return payload
        raise
    finally:
        _preview_render_active = False
        _remove_preview_temp_file()
        render.resolution_x = original["resolution_x"]
        render.resolution_y = original["resolution_y"]
        render.resolution_percentage = original["resolution_percentage"]
        render.film_transparent = original["film_transparent"]
        render.filepath = original["filepath"]
        render.use_file_extension = original["use_file_extension"]
        render.image_settings.file_format = original["file_format"]
        render.image_settings.color_mode = original["color_mode"]
        render.image_settings.color_depth = original["color_depth"]
        render.image_settings.compression = original["compression"]
        render.use_multiview = original["use_multiview"]
        for node, mute in original_file_output_mutes:
            node.mute = mute


def _render_preview(params: dict) -> dict:
    return _render_with_settings(params, "render")


def _render_viewport(params: dict) -> dict:
    viewport_params = dict(params)
    viewport_params.setdefault("refresh", False)
    return _render_with_settings(viewport_params, "viewport")


def _find_armature(name: str):
    armature = bpy.data.objects.get(str(name))
    if armature is None or armature.type != "ARMATURE":
        _raise_command_error(
            "armature_not_found",
            f"Armature '{name}' is unavailable",
        )
    return armature


def _safe_attribute(owner, name: str, default=None):
    if owner is None:
        return default
    try:
        return getattr(owner, name, default)
    except Exception:
        return default


def _finite_vector3(value) -> list[float] | None:
    try:
        values = [float(item) for item in value]
    except (TypeError, ValueError):
        return None
    if len(values) != 3 or any(not np.isfinite(item) for item in values):
        return None
    return values


def _mmd_bone_metadata(bone) -> dict:
    """Return retained PMX semantics without requiring mmd_tools at runtime.

    mmd_tools registers the property group on every pose bone, including bones
    that do not belong to an MMD model.  ``present`` therefore relies on the
    imported names/id or an actual PMX feature instead of mere attribute
    existence.
    """

    mmd_bone = _safe_attribute(bone, "mmd_bone")
    name_j = str(_safe_attribute(mmd_bone, "name_j", "") or "")
    name_e = str(_safe_attribute(mmd_bone, "name_e", "") or "")
    try:
        bone_id = int(_safe_attribute(mmd_bone, "bone_id", -1))
    except (TypeError, ValueError):
        bone_id = -1
    has_additional_rotation = bool(
        _safe_attribute(mmd_bone, "has_additional_rotation", False)
    )
    has_additional_location = bool(
        _safe_attribute(mmd_bone, "has_additional_location", False)
    )
    enabled_fixed_axis = bool(
        _safe_attribute(mmd_bone, "enabled_fixed_axis", False)
    )
    enabled_local_axes = bool(
        _safe_attribute(mmd_bone, "enabled_local_axes", False)
    )
    is_shadow = bool(_safe_attribute(bone, "is_mmd_shadow_bone", False))
    present = bool(
        name_j
        or name_e
        or bone_id >= 0
        or is_shadow
        or has_additional_rotation
        or has_additional_location
        or enabled_fixed_axis
        or enabled_local_axes
    )
    additional_target = str(
        _safe_attribute(mmd_bone, "additional_transform_bone", "") or ""
    )
    try:
        additional_influence = float(
            _safe_attribute(mmd_bone, "additional_transform_influence", 0.0)
        )
    except (TypeError, ValueError):
        additional_influence = 0.0
    if not np.isfinite(additional_influence):
        additional_influence = 0.0
    try:
        transform_order = int(_safe_attribute(mmd_bone, "transform_order", 0))
    except (TypeError, ValueError):
        transform_order = 0
    try:
        ik_rotation_constraint = float(
            _safe_attribute(mmd_bone, "ik_rotation_constraint", 0.0)
        )
    except (TypeError, ValueError):
        ik_rotation_constraint = 0.0
    return {
        "present": present,
        "name_j": name_j or None,
        "name_e": name_e or None,
        "bone_id": bone_id,
        "is_controllable": bool(
            _safe_attribute(mmd_bone, "is_controllable", True)
        ),
        "transform_order": transform_order,
        "transform_after_dynamics": bool(
            _safe_attribute(mmd_bone, "transform_after_dynamics", False)
        ),
        "is_tip": bool(_safe_attribute(mmd_bone, "is_tip", False)),
        "is_shadow": is_shadow,
        "shadow_type": (
            str(_safe_attribute(bone, "mmd_shadow_bone_type", "") or "")
            or None
        ),
        "ik_toggle": (
            bool(_safe_attribute(bone, "mmd_ik_toggle", True))
            if present
            else None
        ),
        "ik_rotation_constraint": (
            ik_rotation_constraint if np.isfinite(ik_rotation_constraint) else 0.0
        ),
        "has_additional_rotation": has_additional_rotation,
        "has_additional_location": has_additional_location,
        "additional_transform_bone": additional_target or None,
        "additional_transform_influence": additional_influence,
        "enabled_fixed_axis": enabled_fixed_axis,
        "fixed_axis": _finite_vector3(
            _safe_attribute(mmd_bone, "fixed_axis", ())
        ),
        "enabled_local_axes": enabled_local_axes,
        "local_axis_x": _finite_vector3(
            _safe_attribute(mmd_bone, "local_axis_x", ())
        ),
        "local_axis_z": _finite_vector3(
            _safe_attribute(mmd_bone, "local_axis_z", ())
        ),
    }


def _pose_bone_aliases(bone) -> set[str]:
    aliases = {
        unicodedata.normalize("NFKC", str(bone.name)).casefold()
    }
    mmd_bone = _safe_attribute(bone, "mmd_bone")
    for value in (
        _safe_attribute(mmd_bone, "name_j", ""),
        _safe_attribute(mmd_bone, "name_e", ""),
        bone.get("name_j", ""),
        bone.get("name_e", ""),
    ):
        if value:
            aliases.add(unicodedata.normalize("NFKC", str(value)).casefold())
    return aliases


def _mmd_fixed_axis_local(bone, metadata: dict | None = None):
    metadata = metadata or _mmd_bone_metadata(bone)
    values = metadata.get("fixed_axis")
    if not metadata.get("enabled_fixed_axis") or not values:
        return None
    # PMX axes are retained in MMD XYZ.  mmd_tools converts them to Blender
    # object space using XZY before applying a bone roll.
    object_axis = Vector((values[0], values[2], values[1]))
    if object_axis.length < 1e-8:
        return None
    try:
        local_axis = (
            bone.bone.matrix_local.to_3x3().inverted_safe() @ object_axis
        )
    except Exception:
        return None
    if local_axis.length < 1e-8:
        return None
    return local_axis.normalized()


def _bone_rotation_specification(bone, base_limits) -> dict:
    metadata = _mmd_bone_metadata(bone)
    limits = [max(0.0, float(value)) for value in base_limits]
    locks = tuple(bool(value) for value in _safe_attribute(
        bone, "lock_rotation", (False, False, False)
    ))
    for axis in range(min(3, len(locks))):
        if locks[axis]:
            limits[axis] = 0.0
    fixed_axis = _mmd_fixed_axis_local(bone, metadata)
    drive_mode = "fk_rotation"
    fixed_axis_values = None
    if metadata.get("enabled_fixed_axis"):
        drive_mode = "mmd_fixed_axis"
        if fixed_axis is None:
            limits = [0.0, 0.0, 0.0]
        else:
            # The first degree component becomes the signed angle around the
            # retained PMX fixed axis.  This avoids pretending that a one-axis
            # PMX bone accepts three independent Blender Euler channels.
            limits = [max(limits), 0.0, 0.0]
            fixed_axis_values = [float(value) for value in fixed_axis]
    if metadata.get("is_shadow") or (
        metadata.get("present") and not metadata.get("is_controllable")
    ):
        limits = [0.0, 0.0, 0.0]
    return {
        "max_degrees": limits,
        "drive_mode": drive_mode,
        "fixed_axis_local": fixed_axis_values,
        "rotation_mode": str(bone.rotation_mode),
        "safe": any(value > 0.0 for value in limits),
    }


def _control_offset_limits(bone, base_limits=(1.0, 1.0, 1.0)) -> list[float]:
    limits = [max(0.0, float(value)) for value in base_limits]
    locks = tuple(bool(value) for value in _safe_attribute(
        bone, "lock_location", (False, False, False)
    ))
    for axis in range(min(3, len(locks))):
        if locks[axis]:
            limits[axis] = 0.0
    metadata = _mmd_bone_metadata(bone)
    if metadata.get("is_shadow") or (
        metadata.get("present") and not metadata.get("is_controllable")
    ):
        return [0.0, 0.0, 0.0]
    return limits


def _average_vector(values, fallback: Vector) -> Vector:
    values = [Vector(value) for value in values]
    if not values:
        return fallback.copy()
    result = Vector((0.0, 0.0, 0.0))
    for value in values:
        result += value
    result /= len(values)
    return result


def _normalized_or(value: Vector, fallback: Vector) -> Vector:
    value = Vector(value)
    if value.length <= 1e-6:
        return fallback.normalized()
    return value.normalized()


def _character_axes(mapped_bones: dict) -> dict[str, Vector]:
    """Infer character-left, forward and up axes in armature object space.

    PMX imports normally face Blender -Y, but imported armatures can be rotated
    or mirrored.  Left/right anatomical pairs establish the side axis and the
    pelvis-to-shoulder span establishes up.  Their cross product then gives a
    deterministic forward axis without relying on a camera or object transform.
    """

    side_vectors = []
    for left_role, right_role in (
        ("shoulder_left", "shoulder_right"),
        ("upper_arm_left", "upper_arm_right"),
        ("upper_leg_left", "upper_leg_right"),
        ("foot_left", "foot_right"),
    ):
        left = mapped_bones.get(left_role)
        right = mapped_bones.get(right_role)
        if left is not None and right is not None:
            side_vectors.append(left.head - right.head)
    side = _normalized_or(
        _average_vector(side_vectors, Vector((1.0, 0.0, 0.0))),
        Vector((1.0, 0.0, 0.0)),
    )

    lower_points = [
        mapped_bones[role].head
        for role in ("upper_leg_left", "upper_leg_right")
        if role in mapped_bones
    ]
    upper_points = [
        mapped_bones[role].head
        for role in ("shoulder_left", "shoulder_right")
        if role in mapped_bones
    ]
    if not upper_points:
        upper_points = [
            mapped_bones[role].head
            for role in ("neck", "head", "upper_body2")
            if role in mapped_bones
        ]
    if lower_points and upper_points:
        up_hint = _average_vector(upper_points, Vector((0.0, 0.0, 1.0))) - \
            _average_vector(lower_points, Vector((0.0, 0.0, 0.0)))
    else:
        up_hint = _average_vector(
            [
                mapped_bones[role].vector
                for role in ("upper_body", "upper_body2", "neck")
                if role in mapped_bones
            ],
            Vector((0.0, 0.0, 1.0)),
        )
    up = _normalized_or(up_hint, Vector((0.0, 0.0, 1.0)))
    side = side - up * side.dot(up)
    if side.length <= 1e-6:
        reference = Vector((1.0, 0.0, 0.0))
        if abs(up.dot(reference)) > 0.9:
            reference = Vector((0.0, 1.0, 0.0))
        side = reference - up * reference.dot(up)
    side.normalize()
    forward = side.cross(up)
    if forward.length <= 1e-6:
        forward = Vector((0.0, -1.0, 0.0))
    forward.normalize()
    up = forward.cross(side).normalized()
    return {"side": side, "forward": forward, "up": up}


def _pose_channel_to_armature_matrix(bone) -> Matrix | None:
    """Return the linear mapping from PoseBone.location to armature space."""

    parent_matrix = bone.parent.matrix if bone.parent is not None else Matrix.Identity(4)
    parent_rest = (
        bone.parent.bone.matrix_local
        if bone.parent is not None
        else Matrix.Identity(4)
    )
    try:
        baseline = bone.bone.convert_local_to_pose(
            bone.matrix_basis,
            bone.bone.matrix_local,
            parent_matrix=parent_matrix,
            parent_matrix_local=parent_rest,
        )
        columns = []
        for axis in range(3):
            basis = bone.matrix_basis.copy()
            basis.translation[axis] += 1.0
            pose = bone.bone.convert_local_to_pose(
                basis,
                bone.bone.matrix_local,
                parent_matrix=parent_matrix,
                parent_matrix_local=parent_rest,
            )
            columns.append(pose.translation - baseline.translation)
        mapping = Matrix(tuple(columns)).transposed()
        if abs(mapping.determinant()) <= 1e-8:
            return None
        return mapping
    except (ReferenceError, RuntimeError, ValueError):
        return None


def _semantic_to_pose_channel_matrix(
    bone,
    character_axes: dict[str, Vector],
) -> Matrix | None:
    channel_to_armature = _pose_channel_to_armature_matrix(bone)
    if channel_to_armature is None:
        return None
    semantic_to_armature = Matrix((
        character_axes["side"],
        character_axes["forward"],
        character_axes["up"],
    )).transposed()
    try:
        return channel_to_armature.inverted() @ semantic_to_armature
    except ValueError:
        return None


def _semantic_control_limits(
    bone,
    character_axes: dict[str, Vector],
    channel_limits=(1.0, 1.0, 1.0),
) -> list[float]:
    mapping = _semantic_to_pose_channel_matrix(bone, character_axes)
    if mapping is None:
        return [0.0, 0.0, 0.0]
    limits = []
    for semantic_axis in range(3):
        candidates = []
        blocked = False
        for channel_axis in range(3):
            coefficient = abs(float(mapping[channel_axis][semantic_axis]))
            if coefficient <= 1e-6:
                continue
            channel_limit = max(0.0, float(channel_limits[channel_axis]))
            if channel_limit <= 0.0:
                blocked = True
                break
            candidates.append(channel_limit / coefficient)
        limits.append(0.0 if blocked or not candidates else min(2.0, min(candidates)))
    return limits


def _semantic_control_offset(
    bone,
    normalized_offset,
    scale: float,
    character_axes: dict[str, Vector],
) -> tuple[float, float, float]:
    mapping = _semantic_to_pose_channel_matrix(bone, character_axes)
    if mapping is None:
        _raise_command_error(
            "control_space_unavailable",
            f"Cannot map semantic translation onto control bone '{bone.name}'",
        )
    channel_offset = mapping @ Vector(normalized_offset)
    channel_offset *= float(scale)
    return tuple(float(value) for value in channel_offset)


def _normalized_bone_name(value: str) -> str:
    normalized = unicodedata.normalize("NFKC", str(value)).casefold()
    return re.sub(r"[\s_.:\-]+", " ", normalized).strip()


def _is_ik_name(value: str) -> bool:
    raw = unicodedata.normalize("NFKC", str(value)).strip()
    normalized = _normalized_bone_name(raw)
    if re.search(r"(^|[^a-z])ik($|[^a-z])", normalized):
        return True
    compact = re.sub(r"[\s_.:\-]+", "", raw)
    folded = compact.casefold()
    anatomical = (
        "arm", "hand", "leg", "foot", "ankle", "knee", "toe",
        "wrist", "elbow", "shoulder", "hip",
    )
    # PMX imports commonly use names such as LeftLegIK or 左足ＩＫ. NFKC
    # normalization plus an anatomical/non-ASCII prefix catches these without
    # classifying unrelated words that merely contain the letters "ik".
    if folded.endswith("ik") and len(compact) > 2:
        prefix = compact[:-2]
        return any(token in prefix.casefold() for token in anatomical) or any(
            not character.isascii() for character in prefix
        )
    if folded.startswith("ik") and len(compact) > 2:
        suffix = compact[2:]
        return any(token in suffix.casefold() for token in anatomical) or any(
            not character.isascii() for character in suffix
        )
    return False


def _unsafe_exact_bone(bone) -> bool:
    metadata = _mmd_bone_metadata(bone)
    if metadata.get("is_shadow") or (
        metadata.get("present") and not metadata.get("is_controllable")
    ):
        return True
    aliases = _pose_bone_aliases(bone)
    for alias in aliases:
        normalized = _normalized_bone_name(alias)
        if _is_ik_name(normalized):
            return True
        if any(token in normalized for token in _UNSAFE_EXACT_BONE_TOKENS):
            return True
    return False


def _constraint_target(constraint, prefix: str = "") -> dict | None:
    target = getattr(constraint, f"{prefix}target", None)
    subtarget = str(getattr(constraint, f"{prefix}subtarget", "") or "")
    if target is None and not subtarget:
        return None
    return {
        "object": getattr(target, "name", None),
        "object_type": getattr(target, "type", None),
        "bone": subtarget or None,
    }


def _constraint_descriptor(constraint) -> dict:
    descriptor = {
        "name": str(constraint.name),
        "type": str(constraint.type),
        "influence": float(getattr(constraint, "influence", 1.0)),
        "mute": bool(getattr(constraint, "mute", False)),
        "valid": bool(getattr(constraint, "is_valid", True)),
        "target": _constraint_target(constraint),
    }
    if constraint.type == "IK":
        descriptor.update({
            "pole": _constraint_target(constraint, "pole_"),
            "chain_count": int(getattr(constraint, "chain_count", 0)),
            "use_tail": bool(getattr(constraint, "use_tail", True)),
            "use_stretch": bool(getattr(constraint, "use_stretch", False)),
        })
    for name in ("owner_space", "target_space", "mix_mode"):
        value = getattr(constraint, name, None)
        if value is not None:
            descriptor[name] = str(value)
    return descriptor


def _ik_chain(owner, chain_count: int, use_tail: bool = True) -> list:
    chain = []
    current = owner if use_tail else owner.parent
    remaining = max(0, int(chain_count))
    while current is not None and (remaining == 0 or len(chain) < remaining):
        chain.append(current)
        current = current.parent
    return chain


def _driver_catalog(armature) -> list[dict]:
    animation_data = getattr(armature, "animation_data", None)
    drivers = getattr(animation_data, "drivers", None)
    if drivers is None:
        return []
    catalog = []
    for curve in drivers:
        driver = getattr(curve, "driver", None)
        variables = []
        for variable in getattr(driver, "variables", []) if driver is not None else []:
            targets = []
            for target in variable.targets:
                target_id = getattr(target, "id", None)
                targets.append({
                    "id": getattr(target_id, "name", None),
                    "data_path": str(getattr(target, "data_path", "")),
                    "bone": str(getattr(target, "bone_target", "")) or None,
                })
            variables.append({
                "name": str(variable.name),
                "type": str(variable.type),
                "targets": targets,
            })
        catalog.append({
            "data_path": str(curve.data_path),
            "array_index": int(curve.array_index),
            "expression": str(getattr(driver, "expression", "")),
            "variables": variables,
        })
    return catalog[:128]


def _scalar_custom_properties(owner) -> dict:
    properties = {}
    for key in owner.keys():
        if str(key).startswith("_"):
            continue
        value = owner.get(key)
        if isinstance(value, (bool, int, float, str)):
            properties[str(key)] = value
    return properties


def _rig_profile(armature, mapped_bones: dict) -> dict:
    character_axes = _character_axes(mapped_bones)
    mmd_metadata = {
        bone.name: _mmd_bone_metadata(bone)
        for bone in armature.pose.bones
    }
    mmd_detected = any(value.get("present") for value in mmd_metadata.values())
    drivers = _driver_catalog(armature)
    constraints = []
    ik_entries = []
    ik_members = set()
    active_ik_members = set()
    ik_targets = set()
    pole_targets = set()
    for bone in armature.pose.bones:
        for constraint in bone.constraints:
            descriptor = _constraint_descriptor(constraint)
            descriptor["owner"] = bone.name
            constraints.append(descriptor)
            if constraint.type != "IK":
                continue
            chain = _ik_chain(
                bone,
                descriptor.get("chain_count", 0),
                descriptor.get("use_tail", True),
            )
            members = [item.name for item in chain]
            ik_members.update(members)
            target = descriptor.get("target") or {}
            pole = descriptor.get("pole") or {}
            target_bone = (
                armature.pose.bones.get(target.get("bone", ""))
                if target.get("object") == armature.name
                else None
            )
            target_metadata = (
                mmd_metadata.get(target_bone.name, {})
                if target_bone is not None
                else {}
            )
            mmd_ik_toggle = (
                bool(_safe_attribute(target_bone, "mmd_ik_toggle", True))
                if target_bone is not None and target_metadata.get("present")
                else None
            )
            ik_enabled = bool(
                descriptor["valid"]
                and not descriptor["mute"]
                and mmd_ik_toggle is not False
            )
            influence = float(descriptor["influence"])
            if not ik_enabled or influence <= 1e-4:
                active_channel = "fk"
            elif influence >= 1.0 - 1e-4:
                active_channel = "ik"
            else:
                # A partially blended IK constraint is not equivalent to either
                # channel. Driving the FK links or the effector independently can
                # double-transform the limb, so advertise neither until the rig
                # reaches a discrete state.
                active_channel = "mixed"
            active = active_channel != "fk"
            if active:
                active_ik_members.update(members)
            if target.get("object") == armature.name and target.get("bone"):
                ik_targets.add(target["bone"])
            if pole.get("object") == armature.name and pole.get("bone"):
                pole_targets.add(pole["bone"])
            driven_paths = []
            for property_name in ("influence", "mute"):
                try:
                    driven_paths.append(constraint.path_from_id(property_name))
                except Exception:
                    driven_paths.append(
                        f'pose.bones["{bone.name}"].constraints'
                        f'["{constraint.name}"].{property_name}'
                    )
            entry_drivers = [
                driver
                for driver in drivers
                if driver["data_path"] in driven_paths
            ]
            ik_entries.append({
                **descriptor,
                "members": members,
                "drivers": entry_drivers,
                "mmd_ik_toggle": mmd_ik_toggle,
                "active_channel": active_channel,
                "active": active,
            })

    bone_descriptors = []
    for bone in armature.pose.bones:
        aliases = sorted(_pose_bone_aliases(bone))
        mmd = mmd_metadata[bone.name]
        bone_descriptors.append({
            "name": bone.name,
            "parent": bone.parent.name if bone.parent is not None else None,
            "aliases": aliases,
            "deform": bool(bone.bone.use_deform),
            "rotation_mode": str(bone.rotation_mode),
            "length": float(bone.bone.length),
            "custom_shape": getattr(getattr(bone, "custom_shape", None), "name", None),
            "lock_rotation": [bool(value) for value in bone.lock_rotation],
            "lock_location": [bool(value) for value in bone.lock_location],
            "constraints": [
                item for item in constraints if item["owner"] == bone.name
            ],
            "custom_properties": _scalar_custom_properties(bone),
            "mmd": mmd,
            "classification": (
                "mmd_shadow" if mmd.get("is_shadow")
                else "ik_effector" if bone.name in ik_targets
                else "ik_pole" if bone.name in pole_targets
                else "ik_chain_active" if bone.name in active_ik_members
                else "ik_chain_inactive" if bone.name in ik_members
                else "mmd_non_controllable" if (
                    mmd.get("present") and not mmd.get("is_controllable")
                )
                else "mmd_additional_transform" if (
                    mmd.get("has_additional_rotation")
                    or mmd.get("has_additional_location")
                )
                else "deform" if bone.bone.use_deform
                else "control"
            ),
        })

    limb_specs = {
        "arm_left": (["upper_arm_left", "forearm_left", "hand_left"], "hand_left"),
        "arm_right": (["upper_arm_right", "forearm_right", "hand_right"], "hand_right"),
        "leg_left": (["upper_leg_left", "knee_left", "foot_left"], "foot_left"),
        "leg_right": (["upper_leg_right", "knee_right", "foot_right"], "foot_right"),
    }
    limbs = {}
    control_roles = {}
    for limb_name, (roles, effector_role) in limb_specs.items():
        role_bones = [mapped_bones[role].name for role in roles if role in mapped_bones]
        candidates = [
            entry
            for entry in ik_entries
            if set(entry["members"]) & set(role_bones)
        ]
        selected = max(
            candidates,
            key=lambda item: (
                bool(item.get("active")),
                len(set(item["members"]) & set(role_bones)),
                float(item.get("influence", 0.0)),
            ),
            default=None,
        )
        selected_overlap = (
            len(set(selected["members"]) & set(role_bones))
            if selected is not None
            else 0
        )
        tied_candidates = [
            entry
            for entry in candidates
            if bool(entry.get("active")) == bool(selected and selected.get("active"))
            and len(set(entry["members"]) & set(role_bones)) == selected_overlap
        ]
        selected_members = set(selected.get("members", [])) if selected else set()
        competing_candidates = [
            entry
            for entry in candidates
            if entry.get("active")
            and selected is not None
            and selected.get("active")
            and set(entry.get("members", [])) & selected_members & set(role_bones)
        ]
        ambiguity_candidates = competing_candidates or tied_candidates
        selection_targets = {
            (
                entry.get("owner"),
                (entry.get("target") or {}).get("bone"),
            )
            for entry in ambiguity_candidates
        }
        ambiguous = len(selection_targets) > 1
        mode = "fk"
        effector = None
        pole = None
        switch_drivers = []
        mmd_ik_toggle = None
        if selected is not None:
            target = selected.get("target") or {}
            pole_target = selected.get("pole") or {}
            if target.get("object") == armature.name:
                effector = target.get("bone")
            if pole_target.get("object") == armature.name:
                pole = pole_target.get("bone")
            switch_drivers = selected.get("drivers", [])
            mmd_ik_toggle = selected.get("mmd_ik_toggle")
            mode = "hybrid" if (
                switch_drivers or mmd_ik_toggle is not None
            ) else (
                "ik" if selected.get("active") else "fk"
            )
        active_channel = (
            selected.get("active_channel", "fk") if selected is not None else "fk"
        )
        limb_length = sum(
            float(mapped_bones[role].bone.length)
            for role in roles
            if role in mapped_bones
        )
        fk_channel_safe = all(
            _bone_rotation_specification(
                mapped_bones[role],
                _BEHAVIOR_ROLE_LIMITS[role],
            )["safe"]
            for role in roles
            if role in mapped_bones
        )
        effector_bone = (
            armature.pose.bones.get(effector) if effector else None
        )
        effector_channel_limits = (
            _control_offset_limits(effector_bone)
            if effector_bone is not None
            else [0.0, 0.0, 0.0]
        )
        effector_limits = (
            _semantic_control_limits(
                effector_bone,
                character_axes,
                effector_channel_limits,
            )
            if effector_bone is not None
            else [0.0, 0.0, 0.0]
        )
        complete_mapping = len(role_bones) == len(roles)
        safe = bool(
            complete_mapping
            and (
                (active_channel == "fk" and fk_channel_safe)
                or (
                    active_channel == "ik"
                    and effector_bone is not None
                    and any(value > 0.0 for value in effector_limits)
                    and not ambiguous
                )
            )
        )
        limbs[limb_name] = {
            "mode": mode if role_bones else "unknown",
            "active_channel": active_channel if role_bones else "unavailable",
            "roles": roles,
            "bones": role_bones,
            "ik_owner": selected.get("owner") if selected else None,
            "effector": effector,
            "pole": pole,
            "ik_chain": selected.get("members", []) if selected else [],
            "switch_drivers": switch_drivers,
            "mmd_ik_toggle": mmd_ik_toggle,
            "candidate_count": len(candidates),
            "selection_ambiguous": ambiguous,
            "selection_conflicts": [
                {
                    "owner": entry.get("owner"),
                    "effector": (entry.get("target") or {}).get("bone"),
                    "members": entry.get("members", []),
                    "active": bool(entry.get("active")),
                }
                for entry in ambiguity_candidates
            ] if ambiguous else [],
            "length": limb_length,
            "safe": safe,
        }
        if (
            active_channel == "ik"
            and effector_bone is not None
            and safe
        ):
            control_roles[effector_role] = {
                "bone": effector,
                "mode": mode,
                "active_channel": active_channel,
                "channels": ["location"],
                "offset_scale": max(limb_length * 0.35, 0.05),
                "normalized_offset_limit": effector_limits,
                "normalized_axes": [
                    "character_left", "character_forward", "character_up"
                ],
                "pole": pole,
                "safe": True,
            }

    center = mapped_bones.get("center")
    if center is not None and center.name not in ik_members:
        torso_scale = max(float(center.bone.length) * 2.0, 0.05)
        center_limits = _semantic_control_limits(
            center,
            character_axes,
            _control_offset_limits(center, (0.35, 0.35, 0.35)),
        )
        if any(value > 0.0 for value in center_limits):
            control_roles["center"] = {
                "bone": center.name,
                "mode": "fk_root",
                "channels": ["location"],
                "offset_scale": torso_scale,
                "normalized_offset_limit": center_limits,
                "normalized_axes": [
                    "character_left", "character_forward", "character_up"
                ],
                "pole": None,
                "safe": True,
            }

    additional_transforms = [
        {
            "bone": name,
            "source": metadata.get("additional_transform_bone"),
            "rotation": bool(metadata.get("has_additional_rotation")),
            "location": bool(metadata.get("has_additional_location")),
            "influence": metadata.get("additional_transform_influence", 0.0),
            "source_resolved": bool(
                metadata.get("additional_transform_bone")
                and armature.pose.bones.get(
                    metadata.get("additional_transform_bone")
                ) is not None
            ),
        }
        for name, metadata in mmd_metadata.items()
        if metadata.get("has_additional_rotation")
        or metadata.get("has_additional_location")
    ]

    structural = {
        "armature": armature.name,
        "bones": [
            (
                item["name"],
                item["parent"],
                (
                    "ik_chain"
                    if item["classification"] in {
                        "ik_chain_active", "ik_chain_inactive"
                    }
                    else item["classification"]
                ),
                {
                    key: value
                    for key, value in item["mmd"].items()
                    if key != "ik_toggle"
                },
            )
            for item in bone_descriptors
        ],
        "constraints": [
            {
                key: value
                for key, value in item.items()
                if key not in {"influence", "mute"}
            }
            for item in constraints
        ],
        "additional_transforms": additional_transforms,
        "limbs": {
            name: {
                key: value
                for key, value in limb.items()
                if key not in {
                    "active_channel", "mmd_ik_toggle", "safe",
                    "selection_ambiguous", "selection_conflicts",
                }
            }
            for name, limb in limbs.items()
        },
    }
    signature = hashlib.sha256(
        json.dumps(structural, ensure_ascii=False, sort_keys=True).encode("utf-8")
    ).hexdigest()
    state_signature = hashlib.sha256(
        json.dumps(
            {
                "limbs": {
                    name: {
                        "active_channel": value["active_channel"],
                        "ik_owner": value.get("ik_owner"),
                        "effector": value.get("effector"),
                        "mmd_ik_toggle": value.get("mmd_ik_toggle"),
                        "selection_ambiguous": value.get("selection_ambiguous"),
                        "safe": value["safe"],
                    }
                    for name, value in limbs.items()
                },
                "ik_constraints": [
                    {
                        "owner": value.get("owner"),
                        "effector": (value.get("target") or {}).get("bone"),
                        "influence": value.get("influence"),
                        "mute": value.get("mute"),
                        "mmd_ik_toggle": value.get("mmd_ik_toggle"),
                        "active_channel": value.get("active_channel"),
                    }
                    for value in ik_entries
                ],
                "active_ik_members": sorted(active_ik_members),
            },
            ensure_ascii=False,
            sort_keys=True,
        ).encode("utf-8")
    ).hexdigest()
    return {
        "version": 3,
        "signature": signature,
        "state_signature": state_signature,
        "adapters": [
            "blender_constraints",
            *(["mmd_tools"] if mmd_detected else []),
        ],
        "mmd_detected": mmd_detected,
        "armature": armature.name,
        "bone_count": len(bone_descriptors),
        "bones": bone_descriptors[:256],
        "bones_truncated": len(bone_descriptors) > 256,
        "constraints": constraints[:256],
        "constraints_truncated": len(constraints) > 256,
        "drivers": drivers,
        "additional_transforms": additional_transforms,
        "armature_custom_properties": _scalar_custom_properties(armature),
        "limbs": limbs,
        "control_roles": control_roles,
        "character_axes": {
            name: [float(component) for component in axis]
            for name, axis in character_axes.items()
        },
        "active_ik_members": sorted(active_ik_members),
        "all_ik_members": sorted(ik_members),
        "safety": {
            "full_body_ready": all(
                limbs[name]["safe"]
                for name in ("arm_left", "arm_right", "leg_left", "leg_right")
            ),
            "unknown_limbs": [
                name for name, value in limbs.items() if value["mode"] == "unknown"
            ],
            "unsafe_limbs": [
                name for name, value in limbs.items() if not value["safe"]
            ],
            "unresolved_additional_transforms": [
                value["bone"]
                for value in additional_transforms
                if not value["source_resolved"]
            ],
            "partially_blended_ik": [
                {
                    "owner": value.get("owner"),
                    "effector": (value.get("target") or {}).get("bone"),
                    "influence": value.get("influence"),
                }
                for value in ik_entries
                if value.get("active_channel") == "mixed"
            ],
            "policy": (
                "fail-closed;partial-ik-blends-are-withheld;"
                "active-ik-uses-effector-translation;"
                "fk-uses-bounded-pose-rotation;mmd-fixed-axis-is-axis-angle"
            ),
        },
    }


def _discover_motion_bones(armature, roles: set[str] | None = None) -> dict:
    requested = set(_MOTION_ROLE_ALIASES) if roles is None else set(roles)
    bones = {}
    for role in sorted(requested):
        if role not in _MOTION_ROLE_ALIASES:
            continue
        expected = {
            unicodedata.normalize("NFKC", str(alias)).casefold()
            for alias in _MOTION_ROLE_ALIASES[role]
        }
        bone = next(
            (
                candidate
                for candidate in armature.pose.bones
                if _pose_bone_aliases(candidate) & expected
                and not _unsafe_exact_bone(candidate)
            ),
            None,
        )
        if bone is not None:
            bones[role] = bone
    return bones


def _resolve_motion_bones(armature, roles: set[str]) -> dict:
    bones = _discover_motion_bones(armature, roles)
    if not bones:
        _raise_command_error(
            "motion_bones_missing",
            "Armature contains none of the bones required by this motion",
        )
    return bones


def _exact_bone_catalog(
    armature,
    mapped_bones: dict,
    blocked_names: set[str] | None = None,
) -> tuple[list[dict], bool]:
    mapped_names = {bone.name for bone in mapped_bones.values()}
    blocked_names = set(blocked_names or ())
    catalog = []
    for bone in sorted(armature.pose.bones, key=lambda item: item.name.casefold()):
        name = str(bone.name)
        if name in blocked_names or _unsafe_exact_bone(bone):
            continue
        is_mapped = name in mapped_names
        use_deform = bool(getattr(getattr(bone, "bone", None), "use_deform", False))
        if not is_mapped and not use_deform:
            continue
        finger_like = any(
            token in name.casefold()
            for token in ("finger", "thumb", "index", "middle", "ring", "pinky", "指")
        )
        limit = 15.0 if finger_like else 20.0
        specification = _bone_rotation_specification(
            bone,
            (limit, limit, limit),
        )
        if not specification["safe"]:
            continue
        mmd = _mmd_bone_metadata(bone)
        catalog.append({
            "name": name,
            **specification,
            "deform": use_deform,
            "mapped_role": next(
                (role for role, candidate in mapped_bones.items() if candidate == bone),
                None,
            ),
            "mmd": {
                "present": mmd["present"],
                "name_j": mmd["name_j"],
                "name_e": mmd["name_e"],
                "has_additional_rotation": mmd["has_additional_rotation"],
                "has_additional_location": mmd["has_additional_location"],
                "transform_after_dynamics": mmd["transform_after_dynamics"],
            },
        })
    truncated = len(catalog) > 96
    return catalog[:96], truncated


def _exact_morph_catalog(model_name: str) -> tuple[list[dict], bool]:
    morphs = []
    names = set()
    for obj in _model_meshes(model_name):
        shape_keys = getattr(obj.data, "shape_keys", None)
        key_blocks = getattr(shape_keys, "key_blocks", None)
        if key_blocks is None:
            continue
        for block in key_blocks:
            name = str(block.name)
            if name.casefold() in {"basis", "base", "基準", "基本"}:
                continue
            if name in names or len(name) > 128:
                continue
            names.add(name)
            morphs.append({"name": name, "mesh": obj.name})
    morphs.sort(key=lambda item: item["name"].casefold())
    truncated = len(morphs) > 64
    return morphs[:64], truncated


def _avatar_capabilities(params: dict) -> dict:
    armature = _find_armature(str(params.get("armature", "")))
    model_name = str(params.get("model", ""))
    mapped = _discover_motion_bones(armature)
    character_axes = _character_axes(mapped)
    rig_profile = _rig_profile(armature, mapped)
    active_ik_members = set(rig_profile.get("active_ik_members", []))
    rotation_roles = {}
    for role, bone in mapped.items():
        if bone.name in active_ik_members:
            continue
        specification = _bone_rotation_specification(
            bone,
            _BEHAVIOR_ROLE_LIMITS[role],
        )
        if not specification["safe"]:
            continue
        rotation_roles[role] = bone
    rotation_role_specs = {}
    for role, bone in rotation_roles.items():
        specification = _bone_rotation_specification(
            bone,
            _BEHAVIOR_ROLE_LIMITS[role],
        )
        if specification["safe"]:
            semantic_axes = (
                _SEMANTIC_ROTATION_AXES.get(role)
                if specification["drive_mode"] == "fk_rotation"
                else None
            )
            rotation_role_specs[role] = {
                "bone": bone.name,
                **specification,
                "rotation_space": (
                    "character-semantic-degrees"
                    if semantic_axes is not None
                    else "relative-idle-pose-basis-degrees"
                ),
                "rotation_axes": semantic_axes or ["local_x", "local_y", "local_z"],
                "zero_pose": "persona-idle",
            }
    exact_bones, bones_truncated = _exact_bone_catalog(
        armature,
        rotation_roles,
        active_ik_members,
    )
    exact_morphs, morphs_truncated = _exact_morph_catalog(model_name)
    expression_presets = {}
    if _dispatcher is not None:
        handler = _dispatcher.COMMANDS.get("expression.inspect")
        if handler is not None:
            try:
                inspection = handler({"model": model_name, "limit": 1})
                expression_presets = inspection.get("preset_support", {})
            except Exception:
                expression_presets = {}
    return {
        "protocol_version": 4,
        "available": True,
        "model": model_name,
        "armature": armature.name,
        "active_action": (
            armature.animation_data.action.name
            if armature.animation_data is not None
            and armature.animation_data.action is not None
            else None
        ),
        "expression_presets": expression_presets,
        "coordinate_space": {
            "rotation": "per-role-see-bone_roles.rotation_space",
            "semantic_rotation": "character-semantic-degrees-relative-persona-idle",
            "fixed_axis_rotation": "degrees[0]-as-signed-axis-angle",
            "translation": "relative-control-character-semantic-normalized-offset",
            "translation_axes": [
                "character_left", "character_forward", "character_up"
            ],
        },
        "bone_roles": rotation_role_specs,
        "control_roles": rig_profile["control_roles"],
        "rig_profile": rig_profile,
        "rotation_exclusions": {
            role: bone.name
            for role, bone in mapped.items()
            if bone.name in active_ik_members or role not in rotation_role_specs
        },
        "bones": exact_bones,
        "bones_truncated": bones_truncated,
        "morphs": exact_morphs,
        "morphs_truncated": morphs_truncated,
        "limits": {
            "keyframes": 8,
            "rotations_per_keyframe": 12,
            "translations_per_keyframe": 6,
            "morphs": 6,
            "duration_scale": [0.5, 2.0],
        },
        "lifecycle": "temporary-action-idle-return",
        "disk_cache": False,
    }


def _motion_keys(
    preset: str,
    action_start: int,
    action_end: int,
    intensity: float,
) -> dict[str, list[tuple[int, tuple[float, float, float]]]]:
    duration = action_end - action_start
    quarter = action_start + duration // 4
    half = action_start + duration // 2
    three_quarter = action_start + (duration * 3) // 4
    if preset == "nod":
        return {
            "head": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (18.0 * intensity, 0.0, 0.0)),
                (half, (-8.0 * intensity, 0.0, 0.0)),
                (three_quarter, (14.0 * intensity, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ]
        }
    if preset == "wave":
        return {
            "upper_arm_right": [
                (action_start, (16.0 * intensity, 112.0 * intensity, 0.0)),
                (action_end, (16.0 * intensity, 112.0 * intensity, 0.0)),
            ],
            "forearm_right": [
                (action_start, (48.0 * intensity, 0.0, 0.0)),
                (action_end, (48.0 * intensity, 0.0, 0.0)),
            ],
            "hand_right": [
                (action_start, (5.0 * intensity, -18.0 * intensity, 0.0)),
                (quarter, (5.0 * intensity, 28.0 * intensity, 0.0)),
                (half, (5.0 * intensity, -30.0 * intensity, 0.0)),
                (
                    three_quarter,
                    (5.0 * intensity, 28.0 * intensity, 0.0),
                ),
                (action_end, (5.0 * intensity, -18.0 * intensity, 0.0)),
            ],
        }
    if preset == "walk":
        amplitude = 28.0 * intensity
        keys = {
            "upper_leg_left": [],
            "upper_leg_right": [],
            "upper_arm_left": [],
            "upper_arm_right": [],
            "knee_left": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (amplitude * 0.8, 0.0, 0.0)),
                (half, (0.0, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            "knee_right": [
                (action_start, (0.0, 0.0, 0.0)),
                (half, (0.0, 0.0, 0.0)),
                (three_quarter, (amplitude * 0.8, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
        }
        for frame, phase in (
            (action_start, 1.0),
            (half, -1.0),
            (action_end, 1.0),
        ):
            keys["upper_leg_left"].append((frame, (phase * amplitude, 0.0, 0.0)))
            keys["upper_leg_right"].append((frame, (-phase * amplitude, 0.0, 0.0)))
            keys["upper_arm_left"].append(
                (frame, (-phase * amplitude * 0.65, 0.0, 0.0))
            )
            keys["upper_arm_right"].append(
                (frame, (phase * amplitude * 0.65, 0.0, 0.0))
            )
        return keys
    if preset == "bow":
        return {
            "upper_body": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (20.0 * intensity, 0.0, 0.0)),
                (three_quarter, (20.0 * intensity, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            "upper_body2": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (10.0 * intensity, 0.0, 0.0)),
                (three_quarter, (10.0 * intensity, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            "head": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (8.0 * intensity, 0.0, 0.0)),
                (three_quarter, (8.0 * intensity, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
        }
    if preset == "head_tilt":
        return {
            "head": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (0.0, 0.0, 13.0 * intensity)),
                (three_quarter, (0.0, 0.0, 13.0 * intensity)),
                (action_end, (0.0, 0.0, 0.0)),
            ]
        }
    if preset == "shake_head":
        return {
            "head": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (0.0, 18.0 * intensity, 0.0)),
                (half, (0.0, -18.0 * intensity, 0.0)),
                (three_quarter, (0.0, 14.0 * intensity, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ]
        }
    if preset == "shrug":
        return {
            "shoulder_left": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (0.0, 0.0, -10.0 * intensity)),
                (three_quarter, (0.0, 0.0, -10.0 * intensity)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            "shoulder_right": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (0.0, 0.0, 10.0 * intensity)),
                (three_quarter, (0.0, 0.0, 10.0 * intensity)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            "head": [
                (action_start, (0.0, 0.0, 0.0)),
                (half, (0.0, 0.0, 5.0 * intensity)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
        }
    if preset == "kick":
        return {
            "upper_leg_right": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (-18.0 * intensity, 0.0, 0.0)),
                (half, (48.0 * intensity, 0.0, 0.0)),
                (three_quarter, (-10.0 * intensity, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            "knee_right": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (55.0 * intensity, 0.0, 0.0)),
                (half, (8.0 * intensity, 0.0, 0.0)),
                (three_quarter, (35.0 * intensity, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            "upper_body": [
                (action_start, (0.0, 0.0, 0.0)),
                (half, (-7.0 * intensity, 0.0, -4.0 * intensity)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
        }
    if preset in {"raise_hand_left", "raise_hand_right"}:
        side = "left" if preset.endswith("left") else "right"
        return {
            f"upper_arm_{side}": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (14.0 * intensity, 122.0 * intensity, 0.0)),
                (three_quarter, (14.0 * intensity, 122.0 * intensity, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
            f"forearm_{side}": [
                (action_start, (0.0, 0.0, 0.0)),
                (quarter, (34.0 * intensity, 0.0, 0.0)),
                (three_quarter, (34.0 * intensity, 0.0, 0.0)),
                (action_end, (0.0, 0.0, 0.0)),
            ],
        }
    _raise_command_error(
        "unsupported_motion",
        "Unsupported bounded motion preset",
    )


def _rotation_snapshot(bone):
    return bone.matrix_basis.to_quaternion().normalized()


def _rotation_with_bone_offset(bone, base_rotation, degrees):
    metadata = _mmd_bone_metadata(bone)
    if metadata.get("enabled_fixed_axis"):
        axis = _mmd_fixed_axis_local(bone, metadata)
        if axis is None:
            return base_rotation.copy()
        delta = Quaternion(axis, radians(float(degrees[0])))
    else:
        delta = Euler(
            tuple(radians(float(value)) for value in degrees),
            "XYZ",
        ).to_quaternion()
    return (base_rotation @ delta).normalized()


def _basis_rotation_for_pose_direction(
    bone,
    base_rotation,
    base_pose_matrix: Matrix,
    target_direction: Vector,
    maximum_swing_degrees: float = 82.0,
):
    """Aim a pose bone in armature space while preserving its imported roll."""

    target_direction = _normalized_or(target_direction, bone.vector)
    current_direction = _normalized_or(
        base_pose_matrix.to_3x3() @ Vector((0.0, 1.0, 0.0)),
        bone.vector,
    )
    swing = current_direction.rotation_difference(target_direction)
    axis, angle = swing.to_axis_angle()
    maximum = radians(max(0.0, float(maximum_swing_degrees)))
    if angle > maximum > 0.0:
        swing = Quaternion(axis, maximum)
    desired_rotation = (swing @ base_pose_matrix.to_quaternion()).normalized()
    desired_pose = Matrix.LocRotScale(
        base_pose_matrix.translation,
        desired_rotation,
        base_pose_matrix.to_scale(),
    )
    parent_matrix = bone.parent.matrix if bone.parent is not None else Matrix.Identity(4)
    parent_rest = (
        bone.parent.bone.matrix_local
        if bone.parent is not None
        else Matrix.Identity(4)
    )
    try:
        basis = bone.bone.convert_local_to_pose(
            desired_pose,
            bone.bone.matrix_local,
            parent_matrix=parent_matrix,
            parent_matrix_local=parent_rest,
            invert=True,
        )
        return basis.to_quaternion().normalized()
    except (ReferenceError, RuntimeError, ValueError):
        return base_rotation.copy()


def _relaxed_arm_rotation(
    role: str,
    bone,
    base_rotation,
    base_pose_matrix: Matrix,
    character_axes: dict[str, Vector],
    rest_angle_degrees: float,
):
    down = -character_axes["up"]
    outward = character_axes["side"]
    if role.endswith("right"):
        outward = -outward
    rest_angle = radians(max(8.0, min(float(rest_angle_degrees), 35.0)))
    target = (
        down * cos(rest_angle)
        + outward * sin(rest_angle)
    ).normalized()
    current = _normalized_or(
        base_pose_matrix.to_3x3() @ Vector((0.0, 1.0, 0.0)),
        bone.vector,
    )
    # Do not force an authored pose farther away from the body.  A/T rest
    # poses, however, are lowered to a small character-specific abduction.
    if current.angle(down, 0.0) <= rest_angle + radians(3.0):
        return base_rotation.copy()
    return _basis_rotation_for_pose_direction(
        bone,
        base_rotation,
        base_pose_matrix,
        target,
    )


def _raised_arm_rotation(
    role: str,
    bone,
    base_rotation,
    base_pose_matrix: Matrix,
    character_axes: dict[str, Vector],
):
    outward = character_axes["side"]
    if role.endswith("right"):
        outward = -outward
    target = (
        character_axes["up"] * 0.80
        + outward * 0.55
        + character_axes["forward"] * 0.12
    ).normalized()
    return _basis_rotation_for_pose_direction(
        bone,
        base_rotation,
        base_pose_matrix,
        target,
        maximum_swing_degrees=135.0,
    )


def _projected_direction(
    direction: Vector,
    normal: Vector,
    fallback: Vector,
) -> Vector:
    projected = Vector(direction) - normal * Vector(direction).dot(normal)
    if projected.length <= 1e-6:
        projected = Vector(fallback) - normal * Vector(fallback).dot(normal)
    return _normalized_or(projected, fallback)


def _semantic_twist(rotation, degrees: float):
    amount = max(-35.0, min(float(degrees), 35.0))
    if abs(amount) <= 1e-6:
        return rotation
    # Pose bones point along local +Y. Post-multiplication keeps the twist on
    # that anatomical long axis after the direction solver has removed roll.
    return (rotation @ Quaternion((0.0, 1.0, 0.0), radians(amount))).normalized()


def _semantic_limb_rotation(
    role: str,
    bone,
    base_rotation,
    base_pose_matrix: Matrix,
    character_axes: dict[str, Vector],
    degrees,
    base_pose_by_bone: dict[str, Matrix],
):
    """Resolve arm roles in character space, independent of PMX bone roll.

    Exact bone targets deliberately retain local-channel semantics.  Only the
    advertised anatomical roles enter this solver, so an imported T/A pose,
    Japanese PMX names, and arbitrary per-bone rolls share the same contract.
    """

    if role not in _SEMANTIC_ROTATION_AXES:
        return _rotation_with_bone_offset(bone, base_rotation, degrees)
    metadata = _mmd_bone_metadata(bone)
    if metadata.get("enabled_fixed_axis"):
        return _rotation_with_bone_offset(bone, base_rotation, degrees)
    values = tuple(float(value) for value in degrees)
    if not any(abs(value) > 1e-6 for value in values):
        return base_rotation.copy()

    base_direction = _normalized_or(
        base_pose_matrix.to_3x3() @ Vector((0.0, 1.0, 0.0)),
        bone.vector,
    )
    outward = character_axes["side"]
    if role.endswith("right"):
        outward = -outward
    forward = character_axes["forward"]

    if role.startswith("upper_arm_"):
        # Positive front/outward values mean anatomical directions on both
        # sides.  Backward/inward requests are retained but tightly bounded so
        # a noisy small planner cannot place an arm behind the torso.
        front = max(-22.0, min(values[0], 100.0))
        side = max(-22.0, min(values[1], 125.0))
        amount = min((front * front + side * side) ** 0.5, 125.0)
        front_tangent = _projected_direction(
            forward if front >= 0.0 else -forward,
            base_direction,
            character_axes["up"],
        )
        side_tangent = _projected_direction(
            outward if side >= 0.0 else -outward,
            base_direction,
            forward,
        )
        tangent = _normalized_or(
            front_tangent * abs(front) + side_tangent * abs(side),
            front_tangent,
        )
        target = (
            base_direction * cos(radians(amount))
            + tangent * sin(radians(amount))
        ).normalized()
        aimed = _basis_rotation_for_pose_direction(
            bone,
            base_rotation,
            base_pose_matrix,
            target,
            maximum_swing_degrees=125.0,
        )
        return _semantic_twist(aimed, values[2])

    if role.startswith("forearm_"):
        # Elbows have one principal flexion direction.  Treat either sign as a
        # bend magnitude and always bend toward the character front; this is the
        # fail-safe that prevents the backwards-over-head pose seen on PMX rigs.
        bend = min(abs(values[0]), 115.0)
        parent = bone.parent
        parent_pose = (
            base_pose_by_bone.get(parent.name)
            if parent is not None
            else None
        )
        parent_direction = (
            _normalized_or(
                parent_pose.to_3x3() @ Vector((0.0, 1.0, 0.0)),
                parent.vector,
            )
            if parent_pose is not None and parent is not None
            else base_direction
        )
        forward_tangent = _projected_direction(
            forward,
            parent_direction,
            character_axes["up"],
        )
        outward_tangent = _projected_direction(
            outward,
            parent_direction,
            forward_tangent,
        )
        lateral = max(-30.0, min(values[1], 30.0))
        bend_tangent = _normalized_or(
            forward_tangent + outward_tangent * tan(radians(lateral)),
            forward_tangent,
        )
        target = (
            parent_direction * cos(radians(bend))
            + bend_tangent * sin(radians(bend))
        ).normalized()
        aimed = _basis_rotation_for_pose_direction(
            bone,
            base_rotation,
            base_pose_matrix,
            target,
            maximum_swing_degrees=120.0,
        )
        return _semantic_twist(aimed, values[2])

    # Wrist channels remain deliberately small.  Their direction is solved
    # around the imported idle hand, avoiding assumptions about hand-bone roll.
    front = max(-35.0, min(values[0], 35.0))
    wave = max(-40.0, min(values[1], 40.0))
    amount = min((front * front + wave * wave) ** 0.5, 45.0)
    front_tangent = _projected_direction(
        forward if front >= 0.0 else -forward,
        base_direction,
        character_axes["up"],
    )
    wave_tangent = _projected_direction(
        outward if wave >= 0.0 else -outward,
        base_direction,
        front_tangent,
    )
    tangent = _normalized_or(
        front_tangent * abs(front) + wave_tangent * abs(wave),
        wave_tangent,
    )
    target = (
        base_direction * cos(radians(amount))
        + tangent * sin(radians(amount))
    ).normalized()
    aimed = _basis_rotation_for_pose_direction(
        bone,
        base_rotation,
        base_pose_matrix,
        target,
        maximum_swing_degrees=48.0,
    )
    return _semantic_twist(aimed, values[2])


def _assign_bone_rotation(bone, rotation) -> str:
    mode = str(bone.rotation_mode)
    if mode == "QUATERNION":
        bone.rotation_quaternion = rotation
        return "rotation_quaternion"
    if mode == "AXIS_ANGLE":
        axis, angle = rotation.to_axis_angle()
        bone.rotation_axis_angle = (angle, axis.x, axis.y, axis.z)
        return "rotation_axis_angle"
    # Preserve the rig's Euler order. Converting the desired quaternion into
    # that order avoids silently changing imported PMX rotation semantics.
    bone.rotation_euler = rotation.to_euler(mode, bone.rotation_euler)
    return "rotation_euler"


def _key_bone_rotation(bone, rotation, frame: int) -> None:
    data_path = _assign_bone_rotation(bone, rotation)
    bone.keyframe_insert(data_path=data_path, frame=frame, group=bone.name)


def _action_fcurves(action):
    legacy = getattr(action, "fcurves", None)
    if legacy is not None and len(legacy):
        yield from legacy
        return
    for layer in getattr(action, "layers", []):
        for strip in getattr(layer, "strips", []):
            for channelbag in getattr(strip, "channelbags", []):
                yield from channelbag.fcurves


def _stop_playback() -> bool:
    stopped = False
    for window in tuple(bpy.context.window_manager.windows):
        screen = window.screen
        if not screen.is_animation_playing:
            continue
        area = next(
            (
                candidate
                for candidate in screen.areas
                if candidate.type in {"VIEW_3D", "DOPESHEET_EDITOR", "TIMELINE"}
            ),
            None,
        )
        try:
            with bpy.context.temp_override(window=window, screen=screen, area=area):
                bpy.ops.screen.animation_cancel(restore_frame=False)
            stopped = True
        except Exception:
            try:
                with bpy.context.temp_override(window=window, screen=screen, area=area):
                    bpy.ops.screen.animation_play()
                stopped = True
            except Exception:
                pass
    return stopped


def _start_playback() -> bool:
    for window in tuple(bpy.context.window_manager.windows):
        screen = window.screen
        if screen.is_animation_playing:
            return True
        area = next(
            (
                candidate
                for candidate in screen.areas
                if candidate.type in {"VIEW_3D", "DOPESHEET_EDITOR", "TIMELINE"}
            ),
            None,
        )
        try:
            with bpy.context.temp_override(window=window, screen=screen, area=area):
                bpy.ops.screen.animation_play()
            return True
        except Exception:
            continue
    return False


def _set_idle_animation(params: dict) -> dict:
    global _idle_session, _animation_session
    armature = _find_armature(str(params.get("armature", "")))
    profile_id = str(params.get("profile_id", "balanced"))[:128]
    duration = max(48, min(int(params.get("duration_frames", 104)), 240))
    sway = max(0.0, min(float(params.get("sway_degrees", 1.8)), 6.0))
    breathe = max(0.0, min(float(params.get("breath_degrees", 1.6)), 5.0))
    head_amount = max(0.0, min(float(params.get("head_degrees", 1.2)), 5.0))
    # Kept under the original wire name for compatibility.  It now describes
    # the final arm angle away from straight down, not a blind local-Z offset.
    arm_rest_angle = max(
        8.0,
        min(float(params.get("arm_drop_degrees", 16.0)), 35.0),
    )
    signature = (
        profile_id, duration, sway, breathe, head_amount, arm_rest_angle,
    )

    existing = _idle_session
    if (
        existing
        and existing.get("active")
        and existing.get("armature") == armature
        and existing.get("signature") == signature
        and existing.get("action") in bpy.data.actions[:]
    ):
        if not (_animation_session and _animation_session.get("active")):
            armature.animation_data_create()
            armature.animation_data.action = existing["action"]
            scene = bpy.context.scene
            scene.frame_start, scene.frame_end = existing["range"]
            _start_playback()
        return {
            "configured": True,
            "reused": True,
            "profile_id": profile_id,
            "action": existing["action"].name,
            "range": list(existing["range"]),
            "lifecycle": "persona-idle-loop",
        }

    _invalidate_preview_frame()
    if _animation_session and _animation_session.get("active"):
        _finish_animation(_animation_session["scene"])
    _stop_playback()
    scene = bpy.context.scene
    armature.animation_data_create()

    if existing and existing.get("armature") == armature:
        original_action = existing.get("original_action")
        original_range = existing.get("original_range", (scene.frame_start, scene.frame_end))
        original_frame = existing.get("original_frame", scene.frame_current)
        base_rotations = existing.get("base_rotations", {})
        old_action = existing.get("action")
    else:
        original_action = armature.animation_data.action
        original_range = (scene.frame_start, scene.frame_end)
        original_frame = scene.frame_current
        base_rotations = {}
        old_action = None

        # A Blender extension reload clears this module's session state but can
        # leave the previously generated idle Action assigned.  Treat that
        # Action as transient; otherwise its old arm offsets become the next
        # idle baseline and accumulate on every reload.
        if (
            original_action is not None
            and original_action.name.startswith((
                "RaViChara_IdleAction", "EverChara_IdleAction",
            ))
        ):
            stale_action = original_action
            armature.animation_data.action = None
            bpy.context.view_layer.update()
            if stale_action.users == 0:
                bpy.data.actions.remove(stale_action)
            original_action = None

    roles = {
        "upper_body", "upper_body2", "neck", "head",
        "upper_arm_left", "upper_arm_right",
    }
    mapped = _discover_motion_bones(armature)
    bones = {role: mapped[role] for role in roles if role in mapped}
    if not bones:
        _raise_command_error(
            "motion_bones_missing",
            "Armature contains none of the bones required by the idle profile",
        )
    rig_profile = _rig_profile(armature, mapped)
    character_axes = _character_axes(mapped)
    active_ik_members = set(rig_profile.get("active_ik_members", []))
    bones = {
        role: bone
        for role, bone in bones.items()
        if bone.name not in active_ik_members
    }
    if not bones:
        _raise_command_error(
            "idle_rig_unsupported",
            "No FK-safe torso or head bones are available for the idle profile",
        )
    bpy.context.view_layer.update()
    arm_pose_matrices = {
        role: bone.matrix.copy()
        for role, bone in bones.items()
        if role in {"upper_arm_left", "upper_arm_right"}
    }
    arm_current_rotations = {
        role: _rotation_snapshot(bone)
        for role, bone in bones.items()
        if role in arm_pose_matrices
    }
    for role, bone in bones.items():
        base_rotations.setdefault(role, _rotation_snapshot(bone))

    idle_rotations = dict(base_rotations)
    for role in ("upper_arm_left", "upper_arm_right"):
        bone = bones.get(role)
        if bone is None:
            continue
        idle_rotations[role] = _relaxed_arm_rotation(
            role,
            bone,
            arm_current_rotations[role],
            arm_pose_matrices[role],
            character_axes,
            arm_rest_angle,
        )

    if old_action is not None and armature.animation_data.action == old_action:
        armature.animation_data.action = None
    if old_action is not None and old_action.users == 0:
        bpy.data.actions.remove(old_action)
    for orphan_name in ("RaViChara_IdleAction", "EverChara_IdleAction"):
        orphan = bpy.data.actions.get(orphan_name)
        if orphan is not None and orphan.users == 0:
            bpy.data.actions.remove(orphan)
    action = bpy.data.actions.new("RaViChara_IdleAction")
    armature.animation_data.action = action

    start = 1
    quarter = start + duration // 4
    half = start + duration // 2
    three_quarter = start + (duration * 3) // 4
    end = start + duration
    phase_frames = [start, quarter, half, three_quarter, end]
    offsets = {
        "upper_body": [
            (0.0, 0.0, 0.0),
            (breathe, 0.0, sway),
            (0.0, 0.0, 0.0),
            (breathe * 0.7, 0.0, -sway),
            (0.0, 0.0, 0.0),
        ],
        "upper_body2": [
            (0.0, 0.0, 0.0),
            (breathe * 0.55, 0.0, sway * 0.45),
            (0.0, 0.0, 0.0),
            (breathe * 0.4, 0.0, -sway * 0.45),
            (0.0, 0.0, 0.0),
        ],
        "neck": [
            (0.0, 0.0, 0.0),
            (0.0, head_amount * 0.25, head_amount * 0.25),
            (0.0, 0.0, 0.0),
            (0.0, -head_amount * 0.25, -head_amount * 0.25),
            (0.0, 0.0, 0.0),
        ],
        "head": [
            (0.0, 0.0, 0.0),
            (-head_amount * 0.25, head_amount * 0.4, head_amount),
            (head_amount * 0.2, 0.0, 0.0),
            (-head_amount * 0.2, -head_amount * 0.4, -head_amount),
            (0.0, 0.0, 0.0),
        ],
        "upper_arm_left": [(0.0, 0.0, 0.0)] * 5,
        "upper_arm_right": [(0.0, 0.0, 0.0)] * 5,
    }
    for role, bone in bones.items():
        base = idle_rotations[role]
        for frame, degrees in zip(phase_frames, offsets[role]):
            _key_bone_rotation(
                bone,
                _rotation_with_bone_offset(bone, base, degrees),
                frame,
            )
    for curve in _action_fcurves(action):
        for point in curve.keyframe_points:
            point.interpolation = "SINE"

    scene.frame_start, scene.frame_end = start, end
    scene.frame_set(start)
    _idle_session = {
        "active": True,
        "profile_id": profile_id,
        "signature": signature,
        "armature": armature,
        "action": action,
        "range": (start, end),
        "base_rotations": base_rotations,
        "idle_rotations": {
            **idle_rotations,
            **{
                bones[role].name: rotation
                for role, rotation in idle_rotations.items()
                if role in bones
            },
        },
        "original_action": original_action,
        "original_range": original_range,
        "original_frame": original_frame,
    }
    started = _start_playback()
    return {
        "configured": True,
        "reused": False,
        "profile_id": profile_id,
        "action": action.name,
        "range": [start, end],
        "resolved_roles": sorted(bones),
        "playback_started": started,
        "lifecycle": "persona-idle-loop",
    }


def _finish_animation(scene) -> None:
    global _animation_session, _animation_finish_timer
    session = _animation_session
    if not session or not session.get("active") or session.get("finishing"):
        return
    session["finishing"] = True
    _stop_playback()
    playback_end = session["playback_end"]
    if scene.frame_current != playback_end:
        scene.frame_set(playback_end)
    armature = session["armature"]
    previous_action = session.get("previous_action")
    live_action = session.get("live_action")
    if armature.animation_data is not None:
        armature.animation_data.action = previous_action
    scene.frame_start, scene.frame_end = session["previous_range"]
    scene.frame_set(session["previous_frame"])
    if live_action is not None and live_action.users == 0:
        bpy.data.actions.remove(live_action)
    session["active"] = False
    session["finishing"] = False
    returns_to_idle = bool(
        _idle_session
        and _idle_session.get("active")
        and _idle_session.get("armature") == armature
        and _idle_session.get("action") == previous_action
    )
    if returns_to_idle:
        _start_playback()
    _invalidate_preview_frame()


def _schedule_animation_finish(scene) -> None:
    global _animation_finish_timer
    session = _animation_session
    if not session or not session.get("active"):
        return
    session["finish_requested"] = True
    if (
        _animation_finish_timer is not None
        and bpy.app.timers.is_registered(_animation_finish_timer)
    ):
        return

    def finish_on_timer():
        global _animation_finish_timer
        _animation_finish_timer = None
        current = _animation_session
        if current and current.get("active") and current.get("finish_requested"):
            _finish_animation(current.get("scene", scene))
        return None

    _animation_finish_timer = finish_on_timer
    bpy.app.timers.register(finish_on_timer, first_interval=0.01)


def _animation_frame_change(scene, *_args) -> None:
    session = _animation_session
    if not (
        session
        and session.get("active")
        and not session.get("finishing")
        and scene == session.get("scene")
    ):
        return
    current_frame = scene.frame_current
    last_frame = session.get("last_frame", current_frame)
    if current_frame >= session["playback_end"] or current_frame < last_frame:
        # frame_change_post executes during dependency-graph evaluation. It must
        # never stop playback, jump frames, or remove Actions directly.
        _schedule_animation_finish(scene)
        return
    session["last_frame"] = current_frame


def _stop_animation(_params: dict) -> dict:
    session = _animation_session
    was_active = bool(session and session.get("active"))
    if was_active:
        _finish_animation(session["scene"])
    elif _idle_session and _idle_session.get("active"):
        armature = _idle_session["armature"]
        armature.animation_data_create()
        armature.animation_data.action = _idle_session["action"]
        scene = bpy.context.scene
        scene.frame_start, scene.frame_end = _idle_session["range"]
        _start_playback()
    else:
        _stop_playback()
    return {
        "stopped": was_active,
        "animation_active": bool(
            _animation_session and _animation_session.get("active")
        ),
        "lifecycle": (
            "persona-idle-loop"
            if _idle_session and _idle_session.get("active")
            else "idle"
        ),
    }


def _apply_behavior_morphs(
    model_name: str,
    morphs: list[dict],
    hold_seconds: float,
) -> list[dict]:
    global _behavior_morph_generation, _behavior_morph_timer
    global _behavior_morph_restore
    if not morphs:
        return []
    if len(morphs) > 6:
        _raise_command_error("invalid_morph_plan", "At most 6 morphs are allowed")
    if _behavior_morph_timer is not None:
        if bpy.app.timers.is_registered(_behavior_morph_timer):
            bpy.app.timers.unregister(_behavior_morph_timer)
        if _behavior_morph_restore is not None:
            _behavior_morph_restore()

    available = {}
    for obj in _model_meshes(model_name):
        shape_keys = getattr(obj.data, "shape_keys", None)
        key_blocks = getattr(shape_keys, "key_blocks", None)
        if key_blocks is None:
            continue
        for block in key_blocks:
            available.setdefault(str(block.name), []).append((obj, block))

    originals = []
    applied = []
    seen = set()
    for item in morphs:
        name = str(item.get("name", ""))
        if name in seen:
            _raise_command_error(
                "duplicate_morph",
                f"Morph '{name}' was specified more than once",
            )
        seen.add(name)
        blocks = available.get(name)
        if not blocks:
            _raise_command_error(
                "morph_not_found",
                f"Morph '{name}' is unavailable on model '{model_name}'",
            )
        try:
            weight = float(item.get("weight", 0.0))
        except (TypeError, ValueError):
            _raise_command_error("invalid_morph_weight", "Morph weight must be numeric")
        if not 0.0 <= weight <= 1.0:
            _raise_command_error(
                "invalid_morph_weight",
                "Morph weight must be between 0 and 1",
            )
        for obj, block in blocks:
            originals.append((block, float(block.value)))
            block.value = weight
            applied.append({"name": name, "mesh": obj.name, "weight": weight})
    _invalidate_preview_frame()

    _behavior_morph_generation += 1
    generation = _behavior_morph_generation

    def restore_morphs():
        global _behavior_morph_timer, _behavior_morph_restore
        if generation != _behavior_morph_generation:
            return None
        for block, value in originals:
            try:
                block.value = value
            except ReferenceError:
                pass
        _invalidate_preview_frame()
        _behavior_morph_timer = None
        _behavior_morph_restore = None
        return None

    _behavior_morph_restore = restore_morphs
    _behavior_morph_timer = restore_morphs
    bpy.app.timers.register(
        restore_morphs,
        first_interval=max(0.25, min(float(hold_seconds), 12.0)),
    )
    return applied


def _play_behavior_plan(params: dict) -> dict:
    global _animation_session
    _invalidate_preview_frame()
    armature = _find_armature(str(params.get("armature", "")))
    model_name = str(params.get("model", ""))
    playback_start = int(params.get("playback_start", 1))
    action_start = int(params.get("action_start", 9))
    action_end = int(params.get("action_end", 41))
    playback_end = int(params.get("playback_end", 49))
    transition_frames = int(params.get("transition_frames", 8))
    if not (
        1 <= playback_start <= action_start < action_end <= playback_end <= 1_000_000
    ):
        _raise_command_error(
            "invalid_frame_range",
            "Expected playback_start <= action_start < action_end <= playback_end",
        )
    if not 0 <= transition_frames <= 120:
        _raise_command_error(
            "invalid_transition",
            "transition_frames must be between 0 and 120",
        )
    if (
        action_start - playback_start < transition_frames
        or playback_end - action_end < transition_frames
    ):
        _raise_command_error(
            "invalid_transition_range",
            "Playback range must leave transition_frames around the action range",
        )
    easing = str(params.get("easing", "SINE")).upper()
    if easing not in {"SINE", "BEZIER", "LINEAR"}:
        _raise_command_error(
            "invalid_easing",
            "easing must be SINE, BEZIER, or LINEAR",
        )
    raw_keyframes = params.get("keyframes", [])
    if not isinstance(raw_keyframes, list) or len(raw_keyframes) > 8:
        _raise_command_error(
            "invalid_behavior_plan",
            "keyframes must be an array with at most 8 entries",
        )
    raw_morphs = params.get("morphs", [])
    if not isinstance(raw_morphs, list):
        _raise_command_error("invalid_morph_plan", "morphs must be an array")

    mapped = _discover_motion_bones(armature)
    character_axes = _character_axes(mapped)
    rig_profile = _rig_profile(armature, mapped)
    active_ik_members = set(rig_profile.get("active_ik_members", []))
    rotation_roles = {}
    rotation_role_limits = {}
    rotation_role_modes = {}
    for role, bone in mapped.items():
        if bone.name in active_ik_members:
            continue
        specification = _bone_rotation_specification(
            bone,
            _BEHAVIOR_ROLE_LIMITS[role],
        )
        if not specification["safe"]:
            continue
        rotation_roles[role] = bone
        rotation_role_limits[role] = tuple(specification["max_degrees"])
        rotation_role_modes[role] = specification["drive_mode"]
    control_roles = rig_profile.get("control_roles", {})
    control_bones = {
        item["name"]: item
        for item in rig_profile.get("bones", [])
        if item.get("classification") in {
            "ik_effector", "ik_pole", "control"
        }
    }
    exact_catalog, _truncated = _exact_bone_catalog(
        armature,
        rotation_roles,
        active_ik_members,
    )
    exact_limits = {
        item["name"]: tuple(item["max_degrees"])
        for item in exact_catalog
    }
    exact_modes = {
        item["name"]: item.get("drive_mode", "fk_rotation")
        for item in exact_catalog
    }
    target_bones = {}
    target_limits = {}
    target_labels = {}
    translation_bones = {}
    translation_labels = {}
    normalized_keyframes = []
    previous_at = -1.0
    for keyframe in raw_keyframes:
        if not isinstance(keyframe, dict):
            _raise_command_error(
                "invalid_behavior_plan",
                "each keyframe must be an object",
            )
        try:
            at = float(keyframe.get("at"))
        except (TypeError, ValueError):
            _raise_command_error(
                "invalid_behavior_plan",
                "keyframe.at must be numeric",
            )
        if not 0.0 <= at <= 1.0 or at <= previous_at:
            _raise_command_error(
                "invalid_behavior_plan",
                "keyframe.at values must be strictly increasing between 0 and 1",
            )
        previous_at = at
        rotations = keyframe.get("rotations", [])
        translations = keyframe.get("translations", [])
        if not isinstance(rotations, list) or len(rotations) > 12:
            _raise_command_error(
                "invalid_behavior_plan",
                "each keyframe supports at most 12 rotations",
            )
        if not isinstance(translations, list) or len(translations) > 6:
            _raise_command_error(
                "invalid_behavior_plan",
                "each keyframe supports at most 6 translations",
            )
        if not rotations and not translations:
            _raise_command_error(
                "invalid_behavior_plan",
                "each keyframe needs at least one rotation or translation",
            )
        normalized_rotations = []
        actual_names = set()
        for rotation in rotations:
            if not isinstance(rotation, dict):
                _raise_command_error(
                    "invalid_behavior_plan",
                    "each rotation must be an object",
                )
            role = str(rotation.get("role", "")).strip()
            exact_name = str(rotation.get("bone", "")).strip()
            if bool(role) == bool(exact_name):
                _raise_command_error(
                    "invalid_behavior_target",
                    "each rotation must use exactly one role or exact bone",
                )
            if role:
                bone = rotation_roles.get(role)
                if bone is None or role not in _BEHAVIOR_ROLE_LIMITS:
                    _raise_command_error(
                        "behavior_role_unavailable",
                        f"Role '{role}' is unavailable for FK rotation; it may be in an active IK chain",
                    )
                identity = f"role:{role}"
                limits = rotation_role_limits[role]
                label = {
                    "role": role,
                    "bone": bone.name,
                    "drive_mode": rotation_role_modes[role],
                }
            else:
                bone = armature.pose.bones.get(exact_name)
                limits = exact_limits.get(exact_name)
                if bone is None or limits is None:
                    _raise_command_error(
                        "behavior_bone_unavailable",
                        f"Bone '{exact_name}' is not in the controllable catalog",
                    )
                identity = f"bone:{exact_name}"
                label = {
                    "bone": exact_name,
                    "drive_mode": exact_modes.get(exact_name, "fk_rotation"),
                }
            if bone.name in actual_names:
                _raise_command_error(
                    "duplicate_behavior_target",
                    f"Bone '{bone.name}' is targeted twice in one keyframe",
                )
            actual_names.add(bone.name)
            degrees = rotation.get("degrees")
            if not isinstance(degrees, list) or len(degrees) != 3:
                _raise_command_error(
                    "invalid_behavior_rotation",
                    "rotation.degrees must contain exactly 3 numbers",
                )
            bounded = []
            for axis, value in enumerate(degrees):
                try:
                    number = float(value)
                except (TypeError, ValueError):
                    _raise_command_error(
                        "invalid_behavior_rotation",
                        "rotation degrees must be numeric",
                    )
                if abs(number) > float(limits[axis]) + 1e-6:
                    _raise_command_error(
                        "unsafe_behavior_rotation",
                        f"Rotation for '{bone.name}' exceeds advertised axis limits",
                    )
                bounded.append(number)
            target_bones[identity] = bone
            target_limits[identity] = limits
            target_labels[identity] = label
            normalized_rotations.append((identity, tuple(bounded)))
        normalized_translations = []
        translated_names = set()
        for translation in translations:
            if not isinstance(translation, dict):
                _raise_command_error(
                    "invalid_behavior_translation",
                    "each translation must be an object",
                )
            role = str(translation.get("role", "")).strip()
            exact_name = str(translation.get("bone", "")).strip()
            if bool(role) == bool(exact_name):
                _raise_command_error(
                    "invalid_behavior_target",
                    "each translation must use exactly one role or exact control bone",
                )
            if role:
                specification = control_roles.get(role)
                if not specification:
                    _raise_command_error(
                        "behavior_control_role_unavailable",
                        f"Control role '{role}' is unavailable on armature '{armature.name}'",
                    )
                bone = armature.pose.bones.get(specification.get("bone", ""))
                if bone is None:
                    _raise_command_error(
                        "behavior_control_bone_unavailable",
                        f"Control role '{role}' has no usable bone",
                    )
                identity = f"control:{role}"
                scale = float(specification.get("offset_scale", 0.05))
                limits = tuple(specification.get("normalized_offset_limit", [1.0, 1.0, 1.0]))
                label = {"control_role": role, "bone": bone.name}
            else:
                descriptor = control_bones.get(exact_name)
                bone = armature.pose.bones.get(exact_name)
                if descriptor is None or bone is None:
                    _raise_command_error(
                        "behavior_control_bone_unavailable",
                        f"Bone '{exact_name}' is not an advertised control bone",
                    )
                identity = f"control_bone:{exact_name}"
                scale = max(float(bone.bone.length) * 2.0, 0.05)
                limits = tuple(_semantic_control_limits(
                    bone,
                    character_axes,
                    _control_offset_limits(bone),
                ))
                if not any(value > 0.0 for value in limits):
                    _raise_command_error(
                        "behavior_control_bone_locked",
                        f"Control bone '{exact_name}' has no writable location channel",
                    )
                label = {"control_bone": exact_name}
            if bone.name in translated_names:
                _raise_command_error(
                    "duplicate_behavior_target",
                    f"Control bone '{bone.name}' is translated twice in one keyframe",
                )
            translated_names.add(bone.name)
            offset = translation.get("offset")
            if not isinstance(offset, list) or len(offset) != 3:
                _raise_command_error(
                    "invalid_behavior_translation",
                    "translation.offset must contain exactly 3 normalized numbers",
                )
            bounded = []
            for axis, value in enumerate(offset):
                try:
                    number = float(value)
                except (TypeError, ValueError):
                    _raise_command_error(
                        "invalid_behavior_translation",
                        "translation offsets must be numeric",
                    )
                if abs(number) > float(limits[axis]) + 1e-6:
                    _raise_command_error(
                        "unsafe_behavior_translation",
                        f"Translation for '{bone.name}' exceeds the advertised normalized limit",
                    )
                bounded.append(number)
            translation_bones[identity] = bone
            translation_labels[identity] = label
            normalized_translations.append((
                identity,
                _semantic_control_offset(
                    bone,
                    tuple(bounded),
                    scale,
                    character_axes,
                ),
            ))
        normalized_keyframes.append((at, normalized_rotations, normalized_translations))

    applied_morphs = _apply_behavior_morphs(
        model_name,
        raw_morphs,
        float(params.get("hold_seconds", 4.0)),
    )
    if not normalized_keyframes:
        return {
            "armature": armature.name,
            "action": None,
            "intent": str(params.get("intent", ""))[:160],
            "resolved_targets": [],
            "applied_morphs": applied_morphs,
            "playback_started": False,
            "lifecycle": "timed-morph-idle-return" if applied_morphs else "idle",
        }

    scene = bpy.context.scene
    current_session = _animation_session
    chained_session = bool(
        current_session
        and current_session.get("active")
        and current_session.get("armature") == armature
    )
    if chained_session:
        previous_frame = current_session["previous_frame"]
        previous_range = current_session["previous_range"]
        previous_action = current_session.get("previous_action")
        baseline_idle = current_session.get("idle_rotations", {})
        current_session["active"] = False
    else:
        previous_frame = scene.frame_current
        previous_range = (scene.frame_start, scene.frame_end)
        previous_action = None
        baseline_idle = {}
    _stop_playback()
    bpy.context.view_layer.update()

    armature.animation_data_create()
    active_action = armature.animation_data.action
    if not chained_session:
        previous_action = active_action
        if (
            _idle_session
            and _idle_session.get("active")
            and _idle_session.get("armature") == armature
            and _idle_session.get("action") == active_action
        ):
            baseline_idle = dict(_idle_session.get("idle_rotations", {}))
    if active_action is not None and (
        active_action.name.startswith("VCB_")
        or active_action.name.startswith("RaViChara_LiveAction")
        or active_action.name.startswith("RaViChara_BehaviorAction")
        or active_action.name.startswith("EverChara_LiveAction")
        or active_action.name.startswith("EverChara_BehaviorAction")
    ):
        if not chained_session:
            previous_action = None
        armature.animation_data.action = None
        bpy.context.view_layer.update()

    start_rotations = {}
    idle_rotations = {}
    for identity, bone in target_bones.items():
        current = _rotation_snapshot(bone)
        start_rotations[identity] = current
        role = identity.removeprefix("role:") if identity.startswith("role:") else None
        idle_rotations[identity] = baseline_idle.get(
            identity,
            baseline_idle.get(role, baseline_idle.get(bone.name, current)),
        )
    pose_restore = {
        identity: _rotation_snapshot(bone)
        for identity, bone in target_bones.items()
    }
    for identity, bone in target_bones.items():
        _assign_bone_rotation(bone, idle_rotations[identity])
    bpy.context.view_layer.update()
    base_pose_matrices = {
        identity: bone.matrix.copy()
        for identity, bone in target_bones.items()
    }
    base_pose_by_bone = {
        bone.name: bone.matrix.copy()
        for bone in armature.pose.bones
    }
    for identity, bone in target_bones.items():
        _assign_bone_rotation(bone, pose_restore[identity])
    bpy.context.view_layer.update()
    start_locations = {
        identity: tuple(bone.location)
        for identity, bone in translation_bones.items()
    }

    for orphan_name in (
        "RaViChara_BehaviorAction", "RaViChara_LiveAction",
        "EverChara_BehaviorAction", "EverChara_LiveAction",
    ):
        orphan = bpy.data.actions.get(orphan_name)
        if orphan is not None and orphan.users == 0:
            bpy.data.actions.remove(orphan)
    action = bpy.data.actions.new("RaViChara_BehaviorAction")
    armature.animation_data.action = action
    span = action_end - action_start

    keyed_targets = set()
    for identity, bone in target_bones.items():
        _key_bone_rotation(bone, start_rotations[identity], playback_start)
    for identity, bone in translation_bones.items():
        bone.location = start_locations[identity]
        bone.keyframe_insert(
            data_path="location",
            frame=playback_start,
            group=bone.name,
        )
    for at, rotations, translations in normalized_keyframes:
        frame = action_start + round(span * at)
        for identity, degrees in rotations:
            bone = target_bones[identity]
            base = idle_rotations[identity]
            role = (
                identity.removeprefix("role:")
                if identity.startswith("role:")
                else ""
            )
            rotation = (
                _semantic_limb_rotation(
                    role,
                    bone,
                    base,
                    base_pose_matrices[identity],
                    character_axes,
                    degrees,
                    base_pose_by_bone,
                )
                if role in _SEMANTIC_ROTATION_AXES
                else _rotation_with_bone_offset(bone, base, degrees)
            )
            _key_bone_rotation(
                bone,
                rotation,
                frame,
            )
            keyed_targets.add(identity)
        for identity, offset in translations:
            bone = translation_bones[identity]
            base = start_locations[identity]
            bone.location = tuple(base[index] + offset[index] for index in range(3))
            bone.keyframe_insert(
                data_path="location",
                frame=frame,
                group=bone.name,
            )
            keyed_targets.add(identity)
    for identity, bone in target_bones.items():
        _key_bone_rotation(bone, idle_rotations[identity], playback_end)
    for identity, bone in translation_bones.items():
        bone.location = start_locations[identity]
        bone.keyframe_insert(
            data_path="location",
            frame=playback_end,
            group=bone.name,
        )
    for curve in _action_fcurves(action):
        for point in curve.keyframe_points:
            point.interpolation = easing

    scene.frame_start = playback_start
    scene.frame_end = playback_end
    scene.frame_set(playback_start)
    session_idle = dict(baseline_idle)
    for identity, value in idle_rotations.items():
        session_idle[identity] = value
        session_idle[target_bones[identity].name] = value
        if identity.startswith("role:"):
            session_idle[identity.removeprefix("role:")] = value
    _animation_session = {
        "active": True,
        "finishing": False,
        "scene": scene,
        "armature": armature,
        "live_action": action,
        "previous_action": previous_action,
        "previous_frame": previous_frame,
        "previous_range": previous_range,
        "playback_end": playback_end,
        "last_frame": playback_start,
        "idle_rotations": session_idle,
    }
    started = _start_playback()
    return {
        "armature": armature.name,
        "action": action.name,
        "intent": str(params.get("intent", ""))[:160],
        "playback_range": [playback_start, playback_end],
        "action_range": [action_start, action_end],
        "resolved_targets": [
            target_labels.get(key, translation_labels.get(key))
            for key in sorted(keyed_targets)
        ],
        "applied_morphs": applied_morphs,
        "easing": easing,
        "playback_started": started,
        "lifecycle": "generated-one-shot-idle-return",
    }


def _play_motion_once(params: dict) -> dict:
    global _animation_session
    _invalidate_preview_frame()
    armature = _find_armature(str(params.get("armature", "")))
    preset = str(params.get("preset", "")).strip().lower()
    playback_start = int(params.get("playback_start", 1))
    action_start = int(params.get("action_start", 9))
    action_end = int(params.get("action_end", 41))
    playback_end = int(params.get("playback_end", 49))
    transition_frames = int(params.get("transition_frames", 8))
    intensity = float(params.get("intensity", 1.0))
    if not (
        1 <= playback_start <= action_start < action_end <= playback_end <= 1_000_000
    ):
        _raise_command_error(
            "invalid_frame_range",
            "Expected playback_start <= action_start < action_end <= playback_end",
        )
    if not 0 <= transition_frames <= 120:
        _raise_command_error(
            "invalid_transition",
            "transition_frames must be between 0 and 120",
        )
    if (
        action_start - playback_start < transition_frames
        or playback_end - action_end < transition_frames
    ):
        _raise_command_error(
            "invalid_transition_range",
            "Playback range must leave transition_frames around the action range",
        )
    if not 0.1 <= intensity <= 2.0:
        _raise_command_error(
            "invalid_intensity",
            "intensity must be between 0.1 and 2.0",
        )
    plan = _motion_keys(preset, action_start, action_end, intensity)
    mapped = _discover_motion_bones(armature)
    bones = {role: mapped[role] for role in plan if role in mapped}
    if not bones:
        _raise_command_error(
            "motion_bones_missing",
            "Armature contains none of the bones required by this motion",
        )
    rig_profile = _rig_profile(armature, mapped)
    character_axes = _character_axes(mapped)
    active_ik_members = set(rig_profile.get("active_ik_members", []))
    bones = {
        role: bone
        for role, bone in bones.items()
        if bone.name not in active_ik_members
    }
    plan = {role: keys for role, keys in plan.items() if role in bones}
    if not bones:
        _raise_command_error(
            "motion_incompatible_with_active_ik",
            f"Preset '{preset}' only targets bones currently driven by IK",
        )
    scene = bpy.context.scene
    current_session = _animation_session
    chained_session = bool(
        current_session
        and current_session.get("active")
        and current_session.get("armature") == armature
    )
    if chained_session:
        previous_frame = current_session["previous_frame"]
        previous_range = current_session["previous_range"]
        previous_action = current_session.get("previous_action")
        baseline_idle = current_session.get("idle_rotations", {})
        current_session["active"] = False
    else:
        previous_frame = scene.frame_current
        previous_range = (scene.frame_start, scene.frame_end)
        previous_action = None
        baseline_idle = {}
    _stop_playback()
    bpy.context.view_layer.update()
    start_rotations = {
        role: _rotation_snapshot(bone)
        for role, bone in bones.items()
    }
    armature.animation_data_create()
    active_action = armature.animation_data.action
    if not chained_session:
        previous_action = active_action
        if (
            _idle_session
            and _idle_session.get("active")
            and _idle_session.get("armature") == armature
            and _idle_session.get("action") == active_action
        ):
            baseline_idle = dict(_idle_session.get("idle_rotations", {}))
    if active_action is not None and (
        active_action.name.startswith("VCB_")
        or active_action.name.startswith("RaViChara_LiveAction")
        or active_action.name.startswith("RaViChara_BehaviorAction")
        or active_action.name.startswith("EverChara_LiveAction")
        or active_action.name.startswith("EverChara_BehaviorAction")
    ):
        if not chained_session:
            previous_action = None
        armature.animation_data.action = None
        bpy.context.view_layer.update()
    idle_rotations = {}
    for role, bone in bones.items():
        idle_rotations[role] = baseline_idle.get(
            role,
            _rotation_snapshot(bone),
        )
    pose_restore = {
        role: _rotation_snapshot(bone)
        for role, bone in bones.items()
    }
    for role, bone in bones.items():
        _assign_bone_rotation(bone, idle_rotations[role])
    bpy.context.view_layer.update()
    base_pose_matrices = {
        role: bone.matrix.copy()
        for role, bone in bones.items()
    }
    base_pose_by_bone = {
        bone.name: bone.matrix.copy()
        for bone in armature.pose.bones
    }
    for role, bone in bones.items():
        _assign_bone_rotation(bone, pose_restore[role])
    bpy.context.view_layer.update()

    for orphan_name in (
        "RaViChara_LiveAction", "RaViChara_BehaviorAction",
        "EverChara_LiveAction", "EverChara_BehaviorAction",
    ):
        old_live_action = bpy.data.actions.get(orphan_name)
        if old_live_action is not None and old_live_action.users == 0:
            bpy.data.actions.remove(old_live_action)
    action = bpy.data.actions.new("RaViChara_LiveAction")
    armature.animation_data.action = action

    for role, bone in bones.items():
        _key_bone_rotation(bone, start_rotations[role], playback_start)
        base = idle_rotations[role]
        for frame, degrees in plan[role]:
            if role in _SEMANTIC_ROTATION_AXES:
                rotation = _semantic_limb_rotation(
                    role,
                    bone,
                    base,
                    base_pose_matrices[role],
                    character_axes,
                    degrees,
                    base_pose_by_bone,
                )
            else:
                rotation = _rotation_with_bone_offset(bone, base, degrees)
            _key_bone_rotation(
                bone,
                rotation,
                frame,
            )
        _key_bone_rotation(bone, base, playback_end)
    for curve in _action_fcurves(action):
        for point in curve.keyframe_points:
            point.interpolation = "BEZIER"

    scene.frame_start = playback_start
    scene.frame_end = playback_end
    scene.frame_set(playback_start)
    _animation_session = {
        "active": True,
        "finishing": False,
        "scene": scene,
        "armature": armature,
        "live_action": action,
        "previous_action": previous_action,
        "previous_frame": previous_frame,
        "previous_range": previous_range,
        "playback_end": playback_end,
        "last_frame": playback_start,
        "idle_rotations": idle_rotations,
    }
    started = _start_playback()
    return {
        "armature": armature.name,
        "action": action.name,
        "preset": preset,
        "playback_range": [playback_start, playback_end],
        "action_range": [action_start, action_end],
        "transition_in_frames": action_start - playback_start,
        "transition_out_frames": playback_end - action_end,
        "configured_transition_frames": transition_frames,
        "playback_started": started,
        "lifecycle": "one-shot-idle-return",
    }


def _virtual_c_expression_preset(value: str) -> str:
    normalized = str(value or "neutral").strip().lower()
    return {
        "happy": "joy",
        "excited": "joy",
        "shy": "smile",
        "default": "neutral",
        "wink": "wink_left",
    }.get(normalized, normalized)


def _apply_expression_timed(params: dict) -> dict:
    global _expression_generation, _expression_timer, _last_error
    if _dispatcher is None:
        _raise_command_error("dispatcher_unavailable", "Virtual_c is unavailable")
    handler = _dispatcher.COMMANDS.get("expression.apply")
    if handler is None:
        _raise_command_error(
            "expression_unavailable",
            "Virtual_c expression.apply is unavailable",
        )
    hold_seconds = max(0.25, min(float(params.get("hold_seconds", 4.0)), 60.0))
    expression = _virtual_c_expression_preset(params.get("expression", "neutral"))
    reset_expression_name = _virtual_c_expression_preset(
        params.get("reset_expression", "neutral")
    )
    apply_params = {
        "model": params.get("model"),
        "expression": expression,
        "intensity": params.get("intensity", 1.0),
        "replace": True,
    }
    result = handler(apply_params)
    _invalidate_preview_frame()
    _expression_generation += 1
    generation = _expression_generation
    if (
        _expression_timer is not None
        and bpy.app.timers.is_registered(_expression_timer)
    ):
        bpy.app.timers.unregister(_expression_timer)

    def reset_expression():
        global _expression_timer, _last_error
        if generation != _expression_generation or _dispatcher is None:
            return None
        try:
            reset_handler = _dispatcher.COMMANDS.get("expression.apply")
            if reset_handler is not None:
                reset_handler(
                    {
                        "model": params.get("model"),
                        "expression": reset_expression_name,
                        "intensity": 1.0,
                        "replace": True,
                    }
                )
                _invalidate_preview_frame()
            _last_error = ""
        except Exception as error:
            _last_error = f"Expression idle reset failed: {error}"[:500]
        _expression_timer = None
        return None

    _expression_timer = reset_expression
    bpy.app.timers.register(reset_expression, first_interval=hold_seconds)
    return {
        **result,
        "hold_seconds": hold_seconds,
        "requested_expression": str(params.get("expression", "neutral")),
        "resolved_expression": expression,
        "reset_expression": reset_expression_name,
        "lifecycle": "timed-idle-return",
    }


def _execute_behavior_bundle(params: dict) -> dict:
    global _latest_behavior_generation
    try:
        generation = int(params.get("generation", 0))
    except (TypeError, ValueError):
        _raise_command_error("invalid_generation", "generation must be an integer")
    if generation < 0:
        _raise_command_error("invalid_generation", "generation must not be negative")
    if generation < _latest_behavior_generation:
        return {
            "ignored": True,
            "reason": "stale_generation",
            "generation": generation,
            "latest_generation": _latest_behavior_generation,
        }
    if generation > _latest_behavior_generation:
        _latest_behavior_generation = generation
        if _animation_session and _animation_session.get("active"):
            _finish_animation(_animation_session["scene"])

    behavior = dict(params.get("behavior") or {})
    for name in (
        "model", "armature", "playback_start", "action_start",
        "action_end", "playback_end", "transition_frames",
    ):
        if name not in behavior and name in params:
            behavior[name] = params[name]
    errors = []
    idle_result = None
    expression_result = None
    behavior_result = None
    fallback_result = None
    idle = params.get("idle")
    if isinstance(idle, dict):
        idle_params = dict(idle)
        idle_params.setdefault("armature", behavior.get("armature"))
        try:
            idle_result = _set_idle_animation(idle_params)
        except Exception as error:
            errors.append(f"idle: {error}")
    expression = str(behavior.get("expression", "neutral"))
    try:
        expression_result = _apply_expression_timed({
            "model": behavior.get("model"),
            "expression": expression,
            "intensity": behavior.get("expression_intensity", 0.75),
            "hold_seconds": behavior.get("hold_seconds", 4.0),
            "reset_expression": "neutral",
        })
    except Exception as error:
        errors.append(f"expression: {error}")
    try:
        behavior_result = _play_behavior_plan(behavior)
    except Exception as error:
        errors.append(f"generated behavior: {error}")
    fallback = behavior.get("fallback_motion")
    if (
        fallback
        and (behavior_result is None or behavior_result.get("action") is None)
    ):
        try:
            fallback_result = _play_motion_once({
                "armature": behavior.get("armature"),
                "preset": fallback,
                "playback_start": behavior.get("playback_start", 1),
                "action_start": behavior.get("action_start", 9),
                "action_end": behavior.get("action_end", 41),
                "playback_end": behavior.get("playback_end", 49),
                "transition_frames": behavior.get("transition_frames", 8),
                "intensity": 1.0,
            })
        except Exception as error:
            errors.append(f"fallback: {error}")
    return {
        "ignored": False,
        "generation": generation,
        "idle": idle_result,
        "expression": expression_result,
        "behavior": behavior_result,
        "fallback": fallback_result,
        "errors": errors,
        "dispatched": any(
            value is not None
            for value in (expression_result, behavior_result, fallback_result)
        ),
        "lifecycle": "atomic-capability-aware-behavior",
    }


_MATERIAL_TARGET_TOKENS = {
    "hair": ("hair", "髪", "头发", "頭髮", "发"),
    "skin": ("skin", "face", "body", "肌", "皮肤", "皮膚", "顔"),
    "clothes": (
        "cloth", "dress", "shirt", "skirt", "coat", "sock", "shoe",
        "衣", "服", "裙", "袖", "靴", "鞋",
    ),
    "eyes": ("eye", "iris", "pupil", "目", "瞳", "眼"),
}


def _model_meshes(model_name: str) -> list:
    root = bpy.data.objects.get(model_name)
    if root is None:
        _raise_command_error("model_not_found", f"Model '{model_name}' is unavailable")
    objects = [root, *list(root.children_recursive)]
    meshes = [obj for obj in objects if obj.type == "MESH"]
    if not meshes:
        _raise_command_error("mesh_not_found", f"Model '{model_name}' has no mesh objects")
    return meshes


def _material_matches(target: str, obj, material) -> bool:
    if target == "all":
        return True
    tokens = _MATERIAL_TARGET_TOKENS[target]
    searchable = f"{obj.name} {material.name}".casefold()
    return any(token.casefold() in searchable for token in tokens)


def _adjust_material_brightness(material, brightness: float) -> int:
    if not material.use_nodes or material.node_tree is None:
        material.diffuse_color = tuple(
            min(1.0, max(0.0, channel * brightness))
            for channel in material.diffuse_color[:3]
        ) + (material.diffuse_color[3],)
        return 1
    tree = material.node_tree
    changed = 0
    for node in tuple(tree.nodes):
        if node.type != "BSDF_PRINCIPLED":
            continue
        base_color = node.inputs.get("Base Color")
        if base_color is None:
            continue
        links = list(base_color.links)
        if links:
            source = links[0].from_socket
            tree.links.remove(links[0])
            value_node = tree.nodes.new("ShaderNodeHueSaturation")
            value_node.name = "RaViChara_Brightness"
            value_node.label = f"RaViChara brightness {brightness:.2f}x"
            value_node.inputs["Value"].default_value = brightness
            value_node.location = (node.location.x - 220, node.location.y)
            tree.links.new(source, value_node.inputs["Color"])
            tree.links.new(value_node.outputs["Color"], base_color)
        else:
            rgba = tuple(base_color.default_value)
            base_color.default_value = (
                min(1.0, max(0.0, rgba[0] * brightness)),
                min(1.0, max(0.0, rgba[1] * brightness)),
                min(1.0, max(0.0, rgba[2] * brightness)),
                rgba[3],
            )
        changed += 1
    return changed


def _adjust_material(params: dict) -> dict:
    model_name = str(params.get("model", ""))
    target = str(params.get("target", "")).strip().lower()
    if target not in {*_MATERIAL_TARGET_TOKENS, "all"}:
        _raise_command_error(
            "invalid_material_target",
            "target must be hair, skin, clothes, eyes, or all",
        )
    try:
        brightness = float(params.get("brightness", 1.0))
    except (TypeError, ValueError):
        brightness = 1.0
    if not 0.25 <= brightness <= 2.0:
        _raise_command_error(
            "invalid_brightness",
            "brightness must be between 0.25 and 2.0",
        )

    changed = []
    copies = {}
    for obj in _model_meshes(model_name):
        for slot_index, slot in enumerate(obj.material_slots):
            source = slot.material
            if source is None or not _material_matches(target, obj, source):
                continue
            key = source.as_pointer()
            duplicate = copies.get(key)
            if duplicate is None:
                duplicate = source.copy()
                duplicate.name = f"{source.name}.RaViCharaBrightness"
                node_count = _adjust_material_brightness(duplicate, brightness)
                copies[key] = duplicate
            else:
                node_count = sum(
                    1 for node in duplicate.node_tree.nodes
                    if node.type == "BSDF_PRINCIPLED"
                ) if duplicate.use_nodes and duplicate.node_tree else 1
            slot.material = duplicate
            changed.append({
                "object": obj.name,
                "slot": slot_index,
                "source_material": source.name,
                "material": duplicate.name,
                "principled_nodes": node_count,
            })
    if not changed:
        _raise_command_error(
            "material_target_not_found",
            f"No material names matched semantic target '{target}'",
        )
    _invalidate_preview_frame()
    return {
        "model": model_name,
        "target": target,
        "brightness": brightness,
        "changed_slots": changed,
        "changed_material_count": len(copies),
        "original_materials_preserved": True,
        "lifecycle": "persistent-confirmed-change",
    }


def _command_bindings() -> dict:
    bindings = {
        _COMMAND_PREVIEW: _render_preview,
        _COMMAND_VIEWPORT: _render_viewport,
        _COMMAND_STATUS: _preview_status,
        _COMMAND_ANIMATION_PLAY_ONCE: _play_motion_once,
        _COMMAND_ANIMATION_STOP: _stop_animation,
        _COMMAND_ANIMATION_SET_IDLE: _set_idle_animation,
        _COMMAND_EXPRESSION_TIMED: _apply_expression_timed,
        _COMMAND_MATERIAL_ADJUST: _adjust_material,
        _COMMAND_AVATAR_CAPABILITIES: _avatar_capabilities,
        _COMMAND_BEHAVIOR_PLAY_PLAN: _play_behavior_plan,
        _COMMAND_BEHAVIOR_EXECUTE: _execute_behavior_bundle,
    }
    # One migration cycle of aliases keeps existing packaged clients and LM
    # Studio configurations functional while all newly advertised identifiers
    # use RaViChara.
    for command, handler in tuple(bindings.items()):
        if command.startswith("ravichara."):
            bindings[command.replace("ravichara.", "everchara.", 1)] = handler
    return bindings


def _attach() -> float | None:
    global _dispatcher, _last_error, _transport_frame_limit
    dispatcher = _find_dispatcher()
    if dispatcher is None:
        _last_error = "Virtual_c dispatcher is not loaded"
        return 1.0
    for command, handler in _command_bindings().items():
        dispatcher.COMMANDS[command] = handler
    _dispatcher = dispatcher
    _transport_frame_limit = _configure_transport_frame_limit(dispatcher)
    _last_error = (
        ""
        if _transport_frame_limit is not None
        else "Virtual_c transport limit could not be verified"
    )
    return None


class RAVICHARA_PT_preview_bridge(bpy.types.Panel):
    bl_label = "RaViChara Preview"
    bl_idname = "RAVICHARA_PT_preview_bridge"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "RaViChara"

    def draw(self, context) -> None:
        del context
        layout = self.layout
        attached = (
            _dispatcher is not None
            and _dispatcher.COMMANDS.get(_COMMAND_PREVIEW) is _render_preview
            and _dispatcher.COMMANDS.get(_COMMAND_VIEWPORT) is _render_viewport
            and _dispatcher.COMMANDS.get(_COMMAND_ANIMATION_PLAY_ONCE)
            is _play_motion_once
            and _dispatcher.COMMANDS.get(_COMMAND_ANIMATION_STOP)
            is _stop_animation
            and _dispatcher.COMMANDS.get(_COMMAND_ANIMATION_SET_IDLE)
            is _set_idle_animation
            and _dispatcher.COMMANDS.get(_COMMAND_MATERIAL_ADJUST)
            is _adjust_material
            and _dispatcher.COMMANDS.get(_COMMAND_AVATAR_CAPABILITIES)
            is _avatar_capabilities
            and _dispatcher.COMMANDS.get(_COMMAND_BEHAVIOR_PLAY_PLAN)
            is _play_behavior_plan
            and _dispatcher.COMMANDS.get(_COMMAND_BEHAVIOR_EXECUTE)
            is _execute_behavior_bundle
        )
        layout.label(
            text="Attached to Virtual_c" if attached else "Waiting for Virtual_c",
            icon="CHECKMARK" if attached else "TIME",
        )
        layout.label(text="No persistent frame cache: 128–1024 px")
        layout.label(text="Temporary PNG deleted after each render")
        layout.label(text="Animation: generated plan + idle return")
        if _last_error:
            layout.label(text=_last_error[:80], icon="ERROR")


_CLASSES = (RAVICHARA_PT_preview_bridge,)


def register() -> None:
    for cls in _CLASSES:
        bpy.utils.register_class(cls)
    if _animation_frame_change not in bpy.app.handlers.frame_change_post:
        bpy.app.handlers.frame_change_post.append(_animation_frame_change)
    if not bpy.app.timers.is_registered(_attach):
        bpy.app.timers.register(_attach, first_interval=0.2, persistent=True)


def unregister() -> None:
    global _expression_timer, _idle_session, _animation_session
    global _behavior_morph_timer, _behavior_morph_restore
    global _animation_finish_timer, _preview_frame_cache, _preview_render_active
    if bpy.app.timers.is_registered(_attach):
        bpy.app.timers.unregister(_attach)
    if (
        _expression_timer is not None
        and bpy.app.timers.is_registered(_expression_timer)
    ):
        bpy.app.timers.unregister(_expression_timer)
    _expression_timer = None
    if (
        _behavior_morph_timer is not None
        and bpy.app.timers.is_registered(_behavior_morph_timer)
    ):
        bpy.app.timers.unregister(_behavior_morph_timer)
    if _behavior_morph_restore is not None:
        _behavior_morph_restore()
    _behavior_morph_timer = None
    _behavior_morph_restore = None
    if (
        _animation_finish_timer is not None
        and bpy.app.timers.is_registered(_animation_finish_timer)
    ):
        bpy.app.timers.unregister(_animation_finish_timer)
    _animation_finish_timer = None
    if _animation_session and _animation_session.get("active"):
        _finish_animation(_animation_session["scene"])
    _stop_playback()
    if _idle_session and _idle_session.get("active"):
        idle = _idle_session
        armature = idle.get("armature")
        if armature is not None:
            armature.animation_data_create()
            armature.animation_data.action = idle.get("original_action")
        scene = bpy.context.scene
        scene.frame_start, scene.frame_end = idle.get(
            "original_range", (scene.frame_start, scene.frame_end)
        )
        scene.frame_set(idle.get("original_frame", scene.frame_current))
        action = idle.get("action")
        if action is not None and action.users == 0:
            bpy.data.actions.remove(action)
    _idle_session = None
    _animation_session = None
    _preview_frame_cache = None
    _preview_render_active = False
    _remove_preview_temp_file()
    if _animation_frame_change in bpy.app.handlers.frame_change_post:
        bpy.app.handlers.frame_change_post.remove(_animation_frame_change)
    if _dispatcher is not None:
        for command, handler in _command_bindings().items():
            if _dispatcher.COMMANDS.get(command) is handler:
                _dispatcher.COMMANDS.pop(command, None)
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
