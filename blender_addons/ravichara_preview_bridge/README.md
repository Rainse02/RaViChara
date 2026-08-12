# RaViChara Preview Bridge

This Blender 4.2+ companion extension adds preview and bounded animation
commands to the loaded
Virtual_c bridge:

- `render.preview.status`
- `render.preview`
- `render.viewport`
- `ravichara.animation.play_once`
- `ravichara.animation.set_idle`
- `ravichara.expression.apply_timed`
- `ravichara.material.adjust`
- `ravichara.avatar.capabilities`
- `ravichara.behavior.play_plan`
- `ravichara.behavior.execute`

It does not open another network port. Virtual_c remains responsible for the
loopback socket, token validation, frame protocol, and main-thread dispatch.

Install the directory or a ZIP containing it through Blender's extension/add-on
installer, enable Virtual_c first, then enable **RaViChara Preview Bridge**.
The 3D View sidebar shows whether the handler has attached successfully.

Preview capture is bounded to 128–1024 pixels. Blender 4.5 can complete a
managed render while its special `Render Result` image still exposes a 0 x 0
pixel buffer to Python. Version 0.5.5 therefore renders one PNG to a uniquely
named system-temporary file, reads it, and deletes it before returning the
frame. This file is transport, not a cache: static frames are retained only as
one bounded in-process payload, the `.blend` is never saved, and the scene's
configured output path and image settings are restored after every capture.
Compositor File Output nodes are temporarily muted so a preview cannot trigger
unrelated persistent frame outputs configured inside the scene.
`render.viewport` and `render.preview` both use the active scene camera and the
managed render engine; unsafe repeated `GPUOffScreen.draw_view3d` capture stays
disabled.

The status response reports the active camera render resolution, pixel aspect,
current scene frame range, and whether a one-shot or persona idle animation is
active. Version 0.5.5 also reports a bounded PMX-aware RigProfile v3 inventory,
semantic character-space IK axes, and A/T-pose-aware idle calibration:
semantic roles mapped to the current armature, a filtered exact-bone catalog,
IK constraints, active IK/FK channels, safe effectors, exact shape-key names,
and per-axis rotation limits.

Conversation motion is no longer limited to stored presets. The application can
send a short keyframe plan whose rotations are relative to the current persona
idle pose. Every target must have appeared in the capability inventory, and the
extension independently enforces keyframe, channel, timing, morph-weight, and
rotation limits. The generated Action is temporary, is removed after playback,
and blends back to the character-card-derived idle Action. Exact morph values are
restored by a bounded timer. The earlier wave/nod/walk and other presets remain
only as a compatibility fallback for 2D mode or invalid/old-plugin operation.

Confirmed semantic material changes still duplicate matching materials before
adjusting brightness, so original materials remain available. Persistent pose,
shader, material, and authored animation changes remain outside the automatic
conversation path and require application-side user confirmation.
