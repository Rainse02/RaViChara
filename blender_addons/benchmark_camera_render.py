"""Measure stable camera-render latency without modifying or saving the scene."""

from __future__ import annotations

import time

import bpy


scene = bpy.context.scene
if scene is None or scene.camera is None:
    raise RuntimeError("The scene needs an active camera")

render = scene.render
original = (
    render.resolution_x,
    render.resolution_y,
    render.resolution_percentage,
    render.film_transparent,
    scene.eevee.taa_render_samples if hasattr(scene, "eevee") else None,
)
try:
    render.resolution_x = 288
    render.resolution_y = 512
    render.resolution_percentage = 100
    render.film_transparent = False
    if hasattr(scene, "eevee"):
        scene.eevee.taa_render_samples = min(
            scene.eevee.taa_render_samples,
            16,
        )
    for index in range(3):
        started = time.perf_counter()
        result = bpy.ops.render.render(write_still=False)
        elapsed = time.perf_counter() - started
        image = bpy.data.images.get("Render Result")
        print(
            "RAVICHARA_RENDER_BENCHMARK",
            index,
            sorted(result),
            f"{elapsed:.4f}",
            tuple(image.size) if image is not None else None,
        )
finally:
    (
        render.resolution_x,
        render.resolution_y,
        render.resolution_percentage,
        render.film_transparent,
        eevee_samples,
    ) = original
    if eevee_samples is not None:
        scene.eevee.taa_render_samples = eevee_samples
