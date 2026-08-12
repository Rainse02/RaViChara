"""Run the RaViChara/Virtual_c bridge against an opened .blend without saving it.

This is an integration-test helper, not an installed add-on.  Blender loads the
scene before executing this script.  The helper attaches the workspace bridge
implementation to Virtual_c, listens on localhost, pumps Virtual_c's main-thread
queue, and exits after a bounded interval.
"""

from __future__ import annotations

import argparse
import importlib
import importlib.util
import json
from pathlib import Path
import sys
import time

import bpy


def _arguments() -> argparse.Namespace:
    values = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--virtual-c-root", type=Path, required=True)
    parser.add_argument("--port", type=int, default=9876)
    parser.add_argument("--seconds", type=float, default=180.0)
    return parser.parse_args(values)


def _loaded_module(suffix: str):
    return next(
        (module for name, module in tuple(sys.modules.items()) if name.endswith(suffix)),
        None,
    )


def _load_virtual_c(root: Path):
    server = _loaded_module("virtual_c_blender_addon.server")
    if server is None:
        sys.path.insert(0, str(root.parent))
        importlib.import_module("virtual_c_blender_addon")
        server = importlib.import_module("virtual_c_blender_addon.server")
    dispatcher_name = f"{server.__package__}.dispatcher"
    dispatcher = sys.modules.get(dispatcher_name)
    if dispatcher is None:
        dispatcher = importlib.import_module(dispatcher_name)
    if getattr(server, "dispatch", None) is not getattr(dispatcher, "dispatch", None):
        raise RuntimeError("Virtual_c server and dispatcher modules do not match")
    return dispatcher, server.runtime


def _load_workspace_bridge(workspace: Path):
    source = workspace / "blender_addons" / "ravichara_preview_bridge" / "__init__.py"
    module_name = "ravichara_preview_bridge_isolated_test"
    spec = importlib.util.spec_from_file_location(module_name, source)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load bridge from {source}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


def _top_level_root(obj):
    current = obj
    while current.parent is not None:
        current = current.parent
    return current


def _descendant_mesh_count(root) -> int:
    return sum(
        1
        for obj in (root, *tuple(root.children_recursive))
        if obj.type == "MESH"
    )


def _primary_model_target():
    armatures = [obj for obj in bpy.data.objects if obj.type == "ARMATURE"]
    if not armatures:
        raise RuntimeError("the scene contains no armature")
    armature = max(
        armatures,
        key=lambda value: (
            _descendant_mesh_count(_top_level_root(value)),
            sum(1 for bone in value.data.bones if bone.use_deform),
            len(value.data.bones),
        ),
    )
    root = _top_level_root(armature)
    if _descendant_mesh_count(root) == 0:
        raise RuntimeError(f"model root '{root.name}' has no mesh descendants")
    return root, armature


def _pump_animation(bridge, frame_interval: float, last_frame_at: float) -> float:
    session = bridge._animation_session
    if not session or not session.get("active"):
        return last_frame_at
    now = time.monotonic()
    if now - last_frame_at < frame_interval:
        return last_frame_at
    scene = session["scene"]
    next_frame = min(scene.frame_current + 1, session["playback_end"])
    scene.frame_set(next_frame)
    bridge._animation_frame_change(scene)
    if next_frame >= session["playback_end"]:
        bridge._finish_animation(scene)
    return now


def main() -> None:
    args = _arguments()
    if not 1024 <= args.port <= 65535:
        raise ValueError("port must be between 1024 and 65535")
    dispatcher, runtime = _load_virtual_c(args.virtual_c_root.resolve())
    bridge = _load_workspace_bridge(args.workspace.resolve())
    runtime.start(args.port, "", 30.0)
    bridge._attach()
    if bridge._dispatcher is not dispatcher:
        raise RuntimeError("RaViChara attached to a different Virtual_c dispatcher")
    if dispatcher.COMMANDS.get("ravichara.avatar.capabilities") is not bridge._avatar_capabilities:
        raise RuntimeError("RaViChara capability command was not attached")

    model, armature = _primary_model_target()
    capability = bridge._avatar_capabilities(
        {"model": model.name, "armature": armature.name}
    )
    print(
        "RAVICHARA_ISOLATED_BRIDGE_READY="
        + json.dumps(
            {
                "port": args.port,
                "plugin_version": list(bridge.bl_info["version"]),
                "protocol_version": capability.get("protocol_version"),
                "rig_profile_version": capability.get("rig_profile", {}).get("version"),
                "model": capability.get("model"),
                "armature": capability.get("armature"),
                "mmd_detected": capability.get("rig_profile", {}).get("mmd_detected"),
                "disk_cache": False,
                "file_saved": False,
            },
            ensure_ascii=False,
        ),
        flush=True,
    )

    deadline = time.monotonic() + max(5.0, min(args.seconds, 600.0))
    fps = max(1.0, float(bpy.context.scene.render.fps or 24))
    last_frame_at = time.monotonic()
    try:
        while time.monotonic() < deadline:
            runtime.process_pending()
            last_frame_at = _pump_animation(bridge, 1.0 / fps, last_frame_at)
            time.sleep(0.01)
    finally:
        session = bridge._animation_session
        if session and session.get("active"):
            bridge._finish_animation(session["scene"])
        runtime.stop()
        print("RAVICHARA_ISOLATED_BRIDGE_STOPPED", flush=True)


if __name__ == "__main__":
    main()
