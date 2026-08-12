"""Read-only verification for the managed ephemeral-PNG preview transport."""

from __future__ import annotations

import base64
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import time

import bpy


def load_bridge():
    source = Path(__file__).with_name("ravichara_preview_bridge") / "__init__.py"
    spec = importlib.util.spec_from_file_location(
        "ravichara_preview_bridge_render_verification",
        source,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load preview bridge from {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def preview_temp_files() -> set[Path]:
    return set(Path(tempfile.gettempdir()).glob("ravichara-preview-*.png"))


def render_settings(scene) -> tuple:
    render = scene.render
    file_output_mutes = tuple(
        (node.name, bool(node.mute))
        for node in (
            tuple(scene.node_tree.nodes)
            if scene.use_nodes and scene.node_tree is not None
            else ()
        )
        if node.bl_idname == "CompositorNodeOutputFile"
    )
    return (
        render.resolution_x,
        render.resolution_y,
        render.resolution_percentage,
        render.film_transparent,
        render.filepath,
        render.use_file_extension,
        render.image_settings.file_format,
        render.image_settings.color_mode,
        render.image_settings.color_depth,
        render.image_settings.compression,
        render.use_multiview,
        file_output_mutes,
    )


def main() -> None:
    bridge = load_bridge()
    scene = bpy.context.scene
    if scene is None or scene.camera is None:
        raise AssertionError("A real scene camera is required")

    before_files = preview_temp_files()
    before_settings = render_settings(scene)
    started = time.monotonic()
    first = bridge._render_viewport(
        {"width": 128, "height": 192, "transparent": False}
    )
    first_seconds = time.monotonic() - started
    after_first_files = preview_temp_files()
    after_first_settings = render_settings(scene)

    started = time.monotonic()
    second = bridge._render_viewport(
        {"width": 128, "height": 192, "transparent": False}
    )
    cached_seconds = time.monotonic() - started
    after_cached_files = preview_temp_files()

    image_bytes = base64.b64decode(first["image_base64"], validate=True)
    if not image_bytes.startswith(b"\x89PNG\r\n\x1a\n"):
        raise AssertionError("Preview is not a PNG")
    png_width, png_height = struct.unpack(">II", image_bytes[16:24])
    if (png_width, png_height) != (128, 192):
        raise AssertionError(
            f"Unexpected PNG dimensions {(png_width, png_height)}"
        )
    if before_settings != after_first_settings:
        raise AssertionError("Scene render settings were not restored")
    if after_first_files != before_files or after_cached_files != before_files:
        raise AssertionError("Temporary preview files were retained")
    if second["capture_mode"] != "stable-camera-render-cache":
        raise AssertionError("Static second frame did not use the memory cache")
    if second["image_base64"] != first["image_base64"]:
        raise AssertionError("Cached frame payload changed")

    status = bridge._preview_status({})
    temporary_status = status["temporary_render_file"]
    if temporary_status["retained"]:
        raise AssertionError("Status reports a retained temporary file")

    print(
        "RAVICHARA_RENDER_TRANSPORT_TEST="
        + json.dumps(
            {
                "plugin_version": list(bridge.bl_info["version"]),
                "blend": bpy.data.filepath,
                "capture_mode": first["capture_mode"],
                "cached_mode": second["capture_mode"],
                "png_dimensions": [png_width, png_height],
                "png_bytes": len(image_bytes),
                "first_render_seconds": round(first_seconds, 3),
                "cached_seconds": round(cached_seconds, 6),
                "settings_restored": before_settings == after_first_settings,
                "temporary_files_retained": len(after_cached_files - before_files),
                "disk_cache": status["disk_cache"],
                "pixel_transport": status["pixel_transport"],
                "file_saved": False,
            },
            ensure_ascii=True,
        )
    )


if __name__ == "__main__":
    main()
