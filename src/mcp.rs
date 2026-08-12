use crate::blender::{
    build_proposal, check_health as check_blender_health,
    normalize_behavior_plan, BehaviorPlan, BlenderBridge,
    persona_idle_profile, RenderMode, BEHAVIOR_ROLES, MAX_PENDING_PROPOSALS,
    SHADER_TEMPLATES,
};
use crate::realtime;
use crate::server::AppState;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{json, Value};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
pub(crate) const EXPRESSIONS: &[&str] = &[
    "neutral",
    "happy",
    "shy",
    "sad",
    "angry",
    "surprised",
    "blink",
    "wink",
];
pub(crate) const MOTIONS: &[&str] = &[
    "wave",
    "nod",
    "walk",
    "bow",
    "head_tilt",
    "shake_head",
    "shrug",
    "kick",
    "raise_hand_left",
    "raise_hand_right",
];

pub async fn handle_get() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [("allow", "POST")],
        "This stateless MCP server accepts JSON-RPC requests over POST.",
    )
        .into_response()
}

pub async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    if !origin_is_allowed(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json_rpc_error(
                Value::Null,
                -32001,
                "MCP Origin must be localhost",
            )),
        )
            .into_response();
    }

    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json_rpc_error(id, -32600, "Invalid JSON-RPC request")),
        )
            .into_response();
    };

    if request.get("id").is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    let response = match method {
        "initialize" => json_rpc_result(
            id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "ravichara-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": "For ordinary conversation, inspect avatar capabilities and prefer ravichara_apply_behavior_plan to generate a bounded transient reaction from the current dialogue. Plans use advertised semantic roles or exact bone/morph names, are relative to the persona idle pose, and return to idle. Use ravichara_apply_reaction only as a compatibility fallback. Read-only inspection is for explicit avatar or scene questions. Persistent pose, Action, expression-sequence, material, shader, and animation changes must be queued with a ravichara_propose_* tool and are not applied until the user confirms them in the UI. Never print tool syntax, JSON, control tags, or internal action reasoning in conversational text."
            }),
        ),
        "ping" => json_rpc_result(id, json!({})),
        "tools/list" => json_rpc_result(id, json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match call_tool(&state, &params).await {
                Ok(result) => json_rpc_result(id, result),
                Err(message) => json_rpc_result(id, tool_error(&message)),
            }
        }
        _ => json_rpc_error(id, -32601, "Method not found"),
    };
    (StatusCode::OK, Json(response)).into_response()
}

fn tool_definitions() -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": "everchara_set_expression",
            "description": "Compatibility tool for one bounded facial expression. Prefer ravichara_apply_reaction when expression and motion should be coordinated. Never reproduce tool syntax in visible text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "enum": EXPRESSIONS,
                        "description": "The visible expression to apply."
                    },
                    "intensity": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 1.0,
                        "description": "Expression strength from 0 to 1."
                    }
                },
                "required": ["expression"],
                "additionalProperties": false
            },
            "outputSchema": interaction_output_schema(),
            "annotations": {
                "title": "Set RaViChara expression",
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_play_motion",
            "description": "Compatibility tool for one bounded motion. Prefer ravichara_apply_reaction. Blender plays it once, blends back to idle, and stops. Never reproduce tool syntax in visible text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "motion": {
                        "type": "string",
                        "enum": MOTIONS,
                        "description": "The short motion to play."
                    }
                },
                "required": ["motion"],
                "additionalProperties": false
            },
            "outputSchema": interaction_output_schema(),
            "annotations": {
                "title": "Play RaViChara motion",
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_get_render_status",
            "description": "Read whether the local RaViChara UI and Blender preview bridge are available. This does not render a frame or modify the scene.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "ui_available": {"type": "boolean"},
                    "blender_enabled": {"type": "boolean"},
                    "blender_reachable": {"type": "boolean"},
                    "preview_available": {"type": "boolean"},
                    "detail": {"type": "string"}
                },
                "required": [
                    "ui_available",
                    "blender_enabled",
                    "blender_reachable",
                    "preview_available",
                    "detail"
                ],
                "additionalProperties": false
            },
            "annotations": {
                "title": "Get RaViChara render status",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
    ];
    tools.extend(extended_tool_definitions());
    for tool in &mut tools {
        rebrand_tool_value(tool);
    }
    tools
}

fn rebrand_tool_value(value: &mut Value) {
    match value {
        Value::String(text) => {
            *text = text
                .replace("EverChara", "RaViChara")
                .replace("everchara_", "ravichara_");
        }
        Value::Array(items) => {
            for item in items {
                rebrand_tool_value(item);
            }
        }
        Value::Object(object) => {
            for item in object.values_mut() {
                rebrand_tool_value(item);
            }
        }
        _ => {}
    }
}

fn extended_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "everchara_apply_behavior_plan",
            "description": "Generate and play one capability-validated transient behavior plan inferred from the conversation. Call ravichara_get_avatar_capabilities first and use only advertised semantic roles, exact bone names, and exact morph names. Rotations are relative to the persona idle pose; the temporary Action is deleted after playback and returns to idle. This is preferred over fixed motion presets and must never be printed in visible text.",
            "inputSchema": behavior_plan_input_schema(),
            "outputSchema": behavior_output_schema(),
            "annotations": {
                "title": "Apply generated RaViChara behavior",
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_apply_reaction",
            "description": "Compatibility fallback for one preset transient reaction. Prefer ravichara_apply_behavior_plan when the installed Blender bridge advertises generated behavior. It is reversible, bounded, returns to neutral/idle, and must never appear in visible text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "enum": EXPRESSIONS,
                        "description": "The single facial expression for this conversational beat."
                    },
                    "motion": {
                        "type": "string",
                        "enum": MOTIONS,
                        "description": "Optional one-shot motion. Omit it when movement is unnecessary."
                    },
                    "intensity": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 1.0
                    }
                },
                "required": ["expression"],
                "additionalProperties": false
            },
            "outputSchema": reaction_output_schema(),
            "annotations": {
                "title": "Apply one RaViChara reaction",
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_get_avatar_capabilities",
            "description": "Read a compact inventory of the configured avatar: selected model and armature, expression support, safe motion presets, shader templates, and mutation policy. Use before proposing persistent changes; do not call for ordinary chat reactions.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "outputSchema": bridge_data_output_schema(),
            "annotations": {
                "title": "Get avatar capabilities",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_inspect_rig",
            "description": "Read a bounded page of exact pose-bone names and current transforms from the configured armature. Use this before proposing an exact-bone pose; this tool is read-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200,
                        "default": 100
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 1000000,
                        "default": 0
                    }
                },
                "additionalProperties": false
            },
            "outputSchema": bridge_data_output_schema(),
            "annotations": {
                "title": "Inspect Blender rig",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_inspect_expressions",
            "description": "Read a bounded page of exact facial Morph/Shape Key names, aliases, categories, and semantic preset support. Use exact returned names before proposing custom morph weights. This tool is read-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "maxLength": 128, "default": ""},
                    "category": {
                        "type": "string",
                        "enum": ["all", "eyebrow", "eye", "mouth", "other", "unknown"],
                        "default": "all"
                    },
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 50},
                    "offset": {"type": "integer", "minimum": 0, "maximum": 1000000, "default": 0}
                },
                "additionalProperties": false
            },
            "outputSchema": bridge_data_output_schema(),
            "annotations": {
                "title": "Inspect facial morphs",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_list_scene_objects",
            "description": "List a bounded page of Blender scene objects so an explicit user-requested scene change can target an exact object. This is read-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 50
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 1000000,
                        "default": 0
                    }
                },
                "additionalProperties": false
            },
            "outputSchema": bridge_data_output_schema(),
            "annotations": {
                "title": "List Blender scene objects",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_list_materials",
            "description": "List material slots for one exact mesh object returned by ravichara_list_scene_objects. This is read-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "object": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    }
                },
                "required": ["object"],
                "additionalProperties": false
            },
            "outputSchema": bridge_data_output_schema(),
            "annotations": {
                "title": "List Blender materials",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "everchara_inspect_shader",
            "description": "Inspect the node graph of one exact material without modifying it. Specify slot when the mesh has multiple materials.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "object": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "material": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "slot": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 255
                    }
                },
                "required": ["object"],
                "additionalProperties": false
            },
            "outputSchema": bridge_data_output_schema(),
            "annotations": {
                "title": "Inspect Blender shader",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        proposal_tool(
            "everchara_propose_rig_pose",
            "Propose a bounded exact-bone pose. This never executes immediately: the UI must obtain explicit user confirmation because the pose persists in the Blender scene.",
            "Propose Blender bone pose",
            json!({
                "type": "object",
                "properties": {
                    "bone": {"type": "string", "minLength": 1, "maxLength": 128},
                    "location": {
                        "type": "array",
                        "items": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                        "minItems": 3,
                        "maxItems": 3
                    },
                    "rotation_degrees": {
                        "type": "array",
                        "items": {"type": "number", "minimum": -180.0, "maximum": 180.0},
                        "minItems": 3,
                        "maxItems": 3
                    },
                    "frame": {"type": "integer", "minimum": 1, "maximum": 1000000}
                },
                "required": ["bone"],
                "additionalProperties": false
            }),
            false,
        ),
        proposal_tool(
            "everchara_propose_motion_action",
            "Propose creation of a persistent Blender Action using an allowlisted motion preset. This never executes until the user confirms it in the UI. Use ravichara_apply_reaction for ordinary transient conversation movement.",
            "Propose persistent Blender Action",
            json!({
                "type": "object",
                "properties": {
                    "preset": {"type": "string", "enum": MOTIONS},
                    "start_frame": {"type": "integer", "minimum": 1, "maximum": 1000000, "default": 1},
                    "duration": {"type": "integer", "minimum": 2, "maximum": 240, "default": 48},
                    "intensity": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 1.0},
                    "side": {"type": "string", "enum": ["left", "right"], "default": "right"},
                    "action_name": {"type": "string", "minLength": 1, "maxLength": 128}
                },
                "required": ["preset"],
                "additionalProperties": false
            }),
            false,
        ),
        proposal_tool(
            "everchara_propose_morph_weights",
            "Propose exact weights for Morph/Shape Key names returned by ravichara_inspect_expressions. This persistent facial change is queued and never executes until the user confirms it in the UI.",
            "Propose facial morph weights",
            json!({
                "type": "object",
                "properties": {
                    "weights": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 16,
                        "items": {
                            "type": "object",
                            "properties": {
                                "morph": {"type": "string", "minLength": 1, "maxLength": 128},
                                "weight": {"type": "number", "minimum": 0.0, "maximum": 1.0}
                            },
                            "required": ["morph", "weight"],
                            "additionalProperties": false
                        }
                    },
                    "reset_scope": {
                        "type": "string",
                        "enum": ["none", "target_categories", "all_facial"],
                        "default": "target_categories"
                    },
                    "frame": {"type": "integer", "minimum": 1, "maximum": 1000000},
                    "transition_frames": {"type": "integer", "minimum": 0, "maximum": 120, "default": 4},
                    "interpolation": {"type": "string", "enum": ["BEZIER", "LINEAR", "CONSTANT"], "default": "BEZIER"}
                },
                "required": ["weights"],
                "additionalProperties": false
            }),
            false,
        ),
        proposal_tool(
            "everchara_propose_expression_sequence",
            "Propose a persistent keyframed facial-expression sequence with strictly increasing frames. It is queued for explicit UI confirmation and is never applied directly.",
            "Propose expression sequence",
            json!({
                "type": "object",
                "properties": {
                    "cues": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 16,
                        "items": {
                            "type": "object",
                            "properties": {
                                "frame": {"type": "integer", "minimum": 1, "maximum": 1000000},
                                "expression": {"type": "string", "enum": EXPRESSIONS},
                                "intensity": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 1.0}
                            },
                            "required": ["frame", "expression"],
                            "additionalProperties": false
                        }
                    },
                    "transition_frames": {"type": "integer", "minimum": 0, "maximum": 120, "default": 4},
                    "interpolation": {"type": "string", "enum": ["BEZIER", "LINEAR", "CONSTANT"], "default": "BEZIER"}
                },
                "required": ["cues"],
                "additionalProperties": false
            }),
            false,
        ),
        proposal_tool(
            "everchara_propose_shader_template",
            "Propose duplicating one target material and applying an allowlisted shader template. The original material remains available, but the scene change still requires explicit UI confirmation.",
            "Propose shader template",
            json!({
                "type": "object",
                "properties": {
                    "object": {"type": "string", "minLength": 1, "maxLength": 128},
                    "material": {"type": "string", "minLength": 1, "maxLength": 128},
                    "slot": {"type": "integer", "minimum": 0, "maximum": 255},
                    "output_name": {"type": "string", "minLength": 1, "maxLength": 128},
                    "template": {"type": "string", "enum": SHADER_TEMPLATES}
                },
                "required": ["object", "template"],
                "additionalProperties": false
            }),
            false,
        ),
        proposal_tool(
            "everchara_propose_shader_graph",
            "Propose a constrained custom shader graph for one exact material. The graph is size-limited, validated again by the Blender extension, duplicates the source material, and requires explicit UI confirmation.",
            "Propose custom shader graph",
            json!({
                "type": "object",
                "properties": {
                    "object": {"type": "string", "minLength": 1, "maxLength": 128},
                    "material": {"type": "string", "minLength": 1, "maxLength": 128},
                    "slot": {"type": "integer", "minimum": 0, "maximum": 255},
                    "output_name": {"type": "string", "minLength": 1, "maxLength": 128},
                    "graph": {
                        "type": "object",
                        "properties": {
                            "nodes": {"type": "array", "minItems": 1, "maxItems": 32},
                            "links": {"type": "array", "maxItems": 64}
                        },
                        "required": ["nodes", "links"],
                        "additionalProperties": false
                    }
                },
                "required": ["object", "graph"],
                "additionalProperties": false
            }),
            false,
        ),
        proposal_tool(
            "everchara_propose_material_adjust",
            "Propose a semantic brightness adjustment for avatar hair, skin, clothes, eyes, or all materials. Matching materials are resolved by the companion extension and duplicated before editing. The change is queued and never executes until the user confirms it in the UI.",
            "Propose material brightness adjustment",
            json!({
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "enum": ["hair", "skin", "clothes", "eyes", "all"]
                    },
                    "brightness": {
                        "type": "number",
                        "minimum": 0.25,
                        "maximum": 2.0,
                        "description": "Multiplicative brightness factor; 1.0 preserves current brightness."
                    }
                },
                "required": ["target", "brightness"],
                "additionalProperties": false
            }),
            false,
        ),
        proposal_tool(
            "everchara_propose_clear_animation",
            "Propose detaching the active armature Action without deleting its data block. This is destructive to the active animation state and always requires explicit UI confirmation.",
            "Propose clearing active animation",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            false,
        ),
    ]
}

fn proposal_tool(
    name: &str,
    description: &str,
    title: &str,
    input_schema: Value,
    destructive: bool,
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "outputSchema": proposal_output_schema(),
        "annotations": {
            "title": title,
            "readOnlyHint": false,
            "destructiveHint": destructive,
            "idempotentHint": false,
            "openWorldHint": false
        }
    })
}

fn behavior_plan_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "intent": {
                "type": "string",
                "minLength": 1,
                "maxLength": 160,
                "description": "Short internal description of the visible conversational beat."
            },
            "expression": {
                "type": "string",
                "enum": EXPRESSIONS,
                "description": "Bounded semantic facial expression."
            },
            "expression_intensity": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0,
                "default": 0.75
            },
            "hold_seconds": {
                "type": "number",
                "minimum": 0.25,
                "maximum": 12.0,
                "default": 4.0
            },
            "fallback_motion": {
                "type": ["string", "null"],
                "enum": [
                    null, "wave", "nod", "walk", "bow", "head_tilt",
                    "shake_head", "shrug", "kick", "raise_hand_left",
                    "raise_hand_right"
                ],
                "description": "Approximate preset used only by 2D mode or if generated playback fails."
            },
            "duration_scale": {
                "type": "number",
                "minimum": 0.5,
                "maximum": 2.0,
                "default": 1.0
            },
            "easing": {
                "type": "string",
                "enum": ["SINE", "BEZIER", "LINEAR"],
                "default": "SINE"
            },
            "keyframes": {
                "type": "array",
                "maxItems": 8,
                "items": {
                    "type": "object",
                    "properties": {
                        "at": {
                            "type": "number",
                            "minimum": 0.0,
                            "maximum": 1.0,
                            "description": "Normalized time within the generated action. Values must increase strictly."
                        },
                        "rotations": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": 12,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "role": {
                                        "type": "string",
                                        "enum": BEHAVIOR_ROLES,
                                        "description": "Advertised semantic role; do not combine with bone."
                                    },
                                    "bone": {
                                        "type": "string",
                                        "minLength": 1,
                                        "maxLength": 128,
                                        "description": "Exact advertised bone name; use only when a semantic role is unavailable."
                                    },
                                    "degrees": {
                                        "type": "array",
                                        "items": {
                                            "type": "number",
                                            "minimum": -90.0,
                                            "maximum": 90.0
                                        },
                                        "minItems": 3,
                                        "maxItems": 3,
                                        "description": "XYZ Euler offsets relative to the current persona idle pose."
                                    }
                                },
                                "required": ["degrees"],
                                "oneOf": [
                                    {"required": ["role"], "not": {"required": ["bone"]}},
                                    {"required": ["bone"], "not": {"required": ["role"]}}
                                ],
                                "additionalProperties": false
                            }
                        },
                        "translations": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": 6,
                            "description": "IK/control-bone offsets. Use only control roles advertised by ravichara_get_avatar_capabilities.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "role": {
                                        "type": "string",
                                        "minLength": 1,
                                        "maxLength": 128,
                                        "description": "Exact advertised control_roles key, such as foot_left."
                                    },
                                    "offset": {
                                        "type": "array",
                                        "items": {
                                            "type": "number",
                                            "minimum": -1.0,
                                            "maximum": 1.0
                                        },
                                        "minItems": 3,
                                        "maxItems": 3,
                                        "description": "Normalized XYZ offset; the backend clamps every axis to the active rig's advertised limit."
                                    }
                                },
                                "required": ["role", "offset"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["at"],
                    "anyOf": [
                        {"required": ["rotations"]},
                        {"required": ["translations"]}
                    ],
                    "additionalProperties": false
                }
            },
            "morphs": {
                "type": "array",
                "maxItems": 6,
                "items": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 128,
                            "description": "Exact morph name advertised by avatar capabilities."
                        },
                        "weight": {
                            "type": "number",
                            "minimum": 0.0,
                            "maximum": 1.0
                        }
                    },
                    "required": ["name", "weight"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["expression", "keyframes", "morphs"],
        "additionalProperties": false
    })
}

fn interaction_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ui_dispatched": {"type": "boolean"},
            "blender_dispatched": {"type": "boolean"},
            "event_sequence": {"type": "integer"},
            "detail": {"type": "string"}
        },
        "required": [
            "ui_dispatched",
            "blender_dispatched",
            "event_sequence",
            "detail"
        ],
        "additionalProperties": false
    })
}

fn reaction_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ui_dispatched": {"type": "boolean"},
            "blender_dispatched": {"type": "boolean"},
            "event_sequences": {
                "type": "array",
                "items": {"type": "integer"},
                "minItems": 1,
                "maxItems": 2
            },
            "detail": {"type": "string"}
        },
        "required": [
            "ui_dispatched",
            "blender_dispatched",
            "event_sequences",
            "detail"
        ],
        "additionalProperties": false
    })
}

fn behavior_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ui_dispatched": {"type": "boolean"},
            "blender_dispatched": {"type": "boolean"},
            "generated_attempted": {"type": "boolean"},
            "generated_dispatched": {"type": "boolean"},
            "fallback_used": {"type": "boolean"},
            "normalized_adjustments": {"type": "integer"},
            "event_sequences": {
                "type": "array",
                "items": {"type": "integer"},
                "minItems": 1,
                "maxItems": 2
            },
            "detail": {"type": "string"}
        },
        "required": [
            "ui_dispatched",
            "blender_dispatched",
            "generated_attempted",
            "generated_dispatched",
            "fallback_used",
            "normalized_adjustments",
            "event_sequences",
            "detail"
        ],
        "additionalProperties": false
    })
}

fn bridge_data_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "data": {"type": "object"},
            "detail": {"type": "string"}
        },
        "required": ["data", "detail"],
        "additionalProperties": false
    })
}

fn proposal_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "proposal_id": {"type": "string"},
            "kind": {"type": "string"},
            "summary": {"type": "string"},
            "risk": {"type": "string"},
            "confirmation_required": {"type": "boolean"},
            "expires_at": {"type": "string"}
        },
        "required": [
            "proposal_id",
            "kind",
            "summary",
            "risk",
            "confirmation_required",
            "expires_at"
        ],
        "additionalProperties": false
    })
}

async fn call_tool(state: &AppState, params: &Value) -> Result<Value, String> {
    let requested_name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call requires params.name".to_string())?;
    let canonical_name = requested_name.replacen("ravichara_", "everchara_", 1);
    let name = canonical_name.as_str();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match name {
        "everchara_apply_behavior_plan" => {
            let bridge = active_blender_bridge(state).await?;
            let capabilities = bridge
                .inspect_avatar_capabilities()
                .await
                .map_err(|error| error.to_string())?;
            let plan = normalize_behavior_plan(&arguments, &capabilities)?;
            dispatch_behavior_plan(state, &bridge, plan).await
        }
        "everchara_apply_reaction" => {
            let expression =
                bounded_enum_argument(&arguments, "expression", EXPRESSIONS)?;
            let motion = arguments
                .get("motion")
                .map(|_| bounded_enum_argument(&arguments, "motion", MOTIONS))
                .transpose()?;
            let intensity = arguments
                .get("intensity")
                .and_then(Value::as_f64)
                .unwrap_or(1.0);
            if !intensity.is_finite() || !(0.0..=1.0).contains(&intensity) {
                return Err("intensity must be between 0 and 1".to_string());
            }
            dispatch_reaction(
                state,
                expression,
                motion,
                intensity as f32,
            )
            .await
        }
        "everchara_set_expression" => {
            let expression = bounded_enum_argument(&arguments, "expression", EXPRESSIONS)?;
            let intensity = arguments
                .get("intensity")
                .and_then(Value::as_f64)
                .unwrap_or(1.0);
            if !(0.0..=1.0).contains(&intensity) {
                return Err("intensity must be between 0 and 1".to_string());
            }
            let event = realtime::publish(
                &state.interactions,
                "expression",
                expression,
                intensity as f32,
                "mcp",
            );
            let config = state.config.read().await.blender.clone();
            let blender_result = if config.enabled
                && RenderMode::from_str(&config.render_mode) != RenderMode::Off
            {
                Some(
                    BlenderBridge::from_config(&config)
                        .apply_expression(expression, intensity as f32)
                        .await,
                )
            } else {
                None
            };
            Ok(interaction_tool_result(
                event.sequence,
                blender_result,
                format!("expression '{expression}' dispatched"),
            ))
        }
        "everchara_play_motion" => {
            let motion = bounded_enum_argument(&arguments, "motion", MOTIONS)?;
            let event =
                realtime::publish(&state.interactions, "motion", motion, 1.0, "mcp");
            let config = state.config.read().await.blender.clone();
            let blender_result = if config.enabled
                && RenderMode::from_str(&config.render_mode) != RenderMode::Off
            {
                Some(BlenderBridge::from_config(&config).play_motion(motion).await)
            } else {
                None
            };
            Ok(interaction_tool_result(
                event.sequence,
                blender_result,
                format!("motion '{motion}' dispatched"),
            ))
        }
        "everchara_get_render_status" => {
            let config = state.config.read().await.blender.clone();
            let health = check_blender_health(&config).await;
            let preview_status = if health.reachable {
                BlenderBridge::from_config(&config)
                    .preview_status()
                    .await
                    .ok()
            } else {
                None
            };
            let preview_error = preview_status
                .as_ref()
                .and_then(|status| status.get("last_error"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let preview_available = preview_status
                .as_ref()
                .and_then(|status| status.get("available"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && preview_error.is_empty();
            let detail = if preview_error.is_empty() {
                health.detail.clone()
            } else {
                format!("{}; preview error: {preview_error}", health.detail)
            };
            let structured = json!({
                "ui_available": true,
                "blender_enabled": health.enabled,
                "blender_reachable": health.reachable,
                "preview_available": preview_available,
                "detail": detail,
            });
            Ok(tool_success(
                structured.clone(),
                &format!(
                    "UI available; Blender reachable: {}; preview available: {}",
                    health.reachable, preview_available
                ),
            ))
        }
        "everchara_get_avatar_capabilities" => {
            let bridge = active_blender_bridge(state).await?;
            let data = bridge
                .inspect_avatar_capabilities()
                .await
                .map_err(|error| error.to_string())?;
            Ok(bridge_data_result(
                data,
                "Avatar capabilities inspected",
            ))
        }
        "everchara_inspect_rig" => {
            let limit = optional_u32_argument(&arguments, "limit", 1, 200)?
                .unwrap_or(100);
            let offset =
                optional_u32_argument(&arguments, "offset", 0, 1_000_000)?
                    .unwrap_or(0);
            let bridge = active_blender_bridge(state).await?;
            let data = bridge
                .inspect_rig(limit, offset)
                .await
                .map_err(|error| error.to_string())?;
            Ok(bridge_data_result(data, "Rig inspected"))
        }
        "everchara_inspect_expressions" => {
            let query = arguments
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if query.chars().count() > 128
                || query.chars().any(char::is_control)
            {
                return Err(
                    "query must not exceed 128 characters or contain controls"
                        .to_string(),
                );
            }
            let category = arguments
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("all");
            if !matches!(
                category,
                "all" | "eyebrow" | "eye" | "mouth" | "other" | "unknown"
            ) {
                return Err(
                    "category must be all, eyebrow, eye, mouth, other, or unknown"
                        .to_string(),
                );
            }
            let limit = optional_u32_argument(&arguments, "limit", 1, 100)?
                .unwrap_or(50);
            let offset =
                optional_u32_argument(&arguments, "offset", 0, 1_000_000)?
                    .unwrap_or(0);
            let bridge = active_blender_bridge(state).await?;
            let data = bridge
                .inspect_expression_inventory(
                    query, category, limit, offset,
                )
                .await
                .map_err(|error| error.to_string())?;
            Ok(bridge_data_result(data, "Facial morphs inspected"))
        }
        "everchara_list_scene_objects" => {
            let limit = optional_u32_argument(&arguments, "limit", 1, 100)?
                .unwrap_or(50);
            let offset =
                optional_u32_argument(&arguments, "offset", 0, 1_000_000)?
                    .unwrap_or(0);
            let bridge = active_blender_bridge(state).await?;
            let data = bridge
                .list_scene_objects(limit, offset)
                .await
                .map_err(|error| error.to_string())?;
            Ok(bridge_data_result(data, "Scene objects listed"))
        }
        "everchara_list_materials" => {
            let object = bounded_name_argument(&arguments, "object", 128)?;
            let bridge = active_blender_bridge(state).await?;
            let data = bridge
                .list_materials(object)
                .await
                .map_err(|error| error.to_string())?;
            Ok(bridge_data_result(data, "Material slots listed"))
        }
        "everchara_inspect_shader" => {
            let object = bounded_name_argument(&arguments, "object", 128)?;
            let material = arguments
                .get("material")
                .map(|_| bounded_name_argument(&arguments, "material", 128))
                .transpose()?;
            let slot =
                optional_u32_argument(&arguments, "slot", 0, 255)?;
            let bridge = active_blender_bridge(state).await?;
            let data = bridge
                .inspect_shader(object, material, slot)
                .await
                .map_err(|error| error.to_string())?;
            Ok(bridge_data_result(data, "Shader inspected"))
        }
        "everchara_propose_rig_pose" => {
            queue_proposal(state, "rig_pose", &arguments).await
        }
        "everchara_propose_motion_action" => {
            queue_proposal(state, "motion_action", &arguments).await
        }
        "everchara_propose_morph_weights" => {
            queue_proposal(state, "morph_weights", &arguments).await
        }
        "everchara_propose_expression_sequence" => {
            queue_proposal(state, "expression_sequence", &arguments).await
        }
        "everchara_propose_shader_template" => {
            queue_proposal(state, "shader_template", &arguments).await
        }
        "everchara_propose_shader_graph" => {
            queue_proposal(state, "shader_graph", &arguments).await
        }
        "everchara_propose_material_adjust" => {
            queue_proposal(state, "material_adjust", &arguments).await
        }
        "everchara_propose_clear_animation" => {
            queue_proposal(state, "clear_animation", &arguments).await
        }
        _ => Err(format!(
            "Unknown RaViChara tool '{requested_name}'. Call tools/list and use an exact name."
        )),
    }
}

async fn dispatch_reaction(
    state: &AppState,
    expression: &str,
    motion: Option<&str>,
    intensity: f32,
) -> Result<Value, String> {
    let expression_event = realtime::publish(
        &state.interactions,
        "expression",
        expression,
        intensity,
        "mcp",
    );
    let mut event_sequences = vec![expression_event.sequence];
    if let Some(motion) = motion {
        let motion_event = realtime::publish(
            &state.interactions,
            "motion",
            motion,
            1.0,
            "mcp",
        );
        event_sequences.push(motion_event.sequence);
    }

    let config = state.config.read().await.blender.clone();
    let active = config.enabled
        && RenderMode::from_str(&config.render_mode) != RenderMode::Off;
    let mut errors = Vec::new();
    if active {
        let bridge = BlenderBridge::from_config(&config);
        let persona = state.persona.read().await.clone();
        if let Err(error) = bridge
            .configure_idle(&persona_idle_profile(&persona))
            .await
        {
            errors.push(format!("idle: {}", error.message));
        }
        if let Err(error) = bridge.apply_expression(expression, intensity).await {
            errors.push(format!("expression: {}", error.message));
        }
        if let Some(motion) = motion {
            if let Err(error) = bridge.play_motion(motion).await {
                errors.push(format!("motion: {}", error.message));
            }
        }
    }
    let blender_dispatched = active && errors.is_empty();
    let detail = if !active {
        "Reaction reached the UI; Blender is disabled".to_string()
    } else if errors.is_empty() {
        "Reaction reached the UI and Blender".to_string()
    } else {
        format!(
            "Reaction reached the UI; Blender reported {}",
            errors.join("; ")
        )
    };
    let structured = json!({
        "ui_dispatched": true,
        "blender_dispatched": blender_dispatched,
        "event_sequences": event_sequences,
        "detail": detail,
    });
    Ok(tool_success(structured, &detail))
}

async fn dispatch_behavior_plan(
    state: &AppState,
    bridge: &BlenderBridge,
    plan: BehaviorPlan,
) -> Result<Value, String> {
    let expression_event = realtime::publish(
        &state.interactions,
        "expression",
        &plan.expression,
        plan.expression_intensity as f32,
        "mcp",
    );
    let mut event_sequences = vec![expression_event.sequence];
    if let Some(motion) = plan.fallback_motion.as_deref() {
        let motion_event = realtime::publish(
            &state.interactions,
            "motion",
            motion,
            1.0,
            "mcp",
        );
        event_sequences.push(motion_event.sequence);
    }
    let normalized_adjustments = plan.normalized_adjustments;
    let persona = state.persona.read().await.clone();
    let generation = state.begin_behavior_generation();
    let _control_guard = state.blender_control_lock.lock().await;
    let execution = bridge
        .execute_behavior_with_generation(&persona, &plan, generation)
        .await;
    let blender_dispatched = execution.dispatched();
    let detail = if execution.errors.is_empty() {
        if execution.generated_dispatched {
            "Generated behavior reached Blender and will return to persona idle"
                .to_string()
        } else if execution.fallback_used {
            "Preset fallback reached Blender and will return to persona idle"
                .to_string()
        } else {
            "Expression reached Blender; no body motion was required".to_string()
        }
    } else {
        format!(
            "Behavior dispatch completed with: {}",
            execution.errors.join("; ")
        )
    };
    let structured = json!({
        "ui_dispatched": true,
        "blender_dispatched": blender_dispatched,
        "generated_attempted": execution.generated_attempted,
        "generated_dispatched": execution.generated_dispatched,
        "fallback_used": execution.fallback_used,
        "normalized_adjustments": normalized_adjustments,
        "event_sequences": event_sequences,
        "detail": detail,
    });
    Ok(tool_success(structured, &detail))
}

async fn active_blender_bridge(
    state: &AppState,
) -> Result<BlenderBridge, String> {
    let config = state.config.read().await.blender.clone();
    if !config.enabled
        || RenderMode::from_str(&config.render_mode) == RenderMode::Off
    {
        return Err(
            "Blender control is disabled. Enable it before using this tool."
                .to_string(),
        );
    }
    if config.profile != "virtual_c" {
        return Err(
            "This tool requires the virtual_c Blender profile.".to_string(),
        );
    }
    Ok(BlenderBridge::from_config(&config))
}

async fn queue_proposal(
    state: &AppState,
    kind: &str,
    arguments: &Value,
) -> Result<Value, String> {
    let config = state.config.read().await.blender.clone();
    if !config.scene_proposals_enabled {
        return Err(
            "Blender scene proposals are disabled in settings.".to_string(),
        );
    }
    if !config.enabled
        || RenderMode::from_str(&config.render_mode) == RenderMode::Off
    {
        return Err(
            "Blender control is disabled; no proposal was queued.".to_string(),
        );
    }
    if config.profile != "virtual_c" {
        return Err(
            "Scene proposals require the virtual_c Blender profile.".to_string(),
        );
    }
    let proposal = build_proposal(kind, arguments)?;
    {
        let mut pending = state.blender_proposals.write().await;
        pending.retain(|_, item| !item.is_expired());
        if pending.len() >= MAX_PENDING_PROPOSALS {
            if let Some(oldest_id) = pending
                .values()
                .min_by(|left, right| left.created_at.cmp(&right.created_at))
                .map(|item| item.id.clone())
            {
                pending.remove(&oldest_id);
            }
        }
        pending.insert(proposal.id.clone(), proposal.clone());
    }
    realtime::publish(
        &state.interactions,
        "blender_proposal",
        &proposal.id,
        1.0,
        "mcp",
    );
    let structured = json!({
        "proposal_id": proposal.id,
        "kind": proposal.kind,
        "summary": proposal.summary,
        "risk": proposal.risk,
        "confirmation_required": true,
        "expires_at": proposal.expires_at,
    });
    Ok(tool_success(
        structured,
        "Proposal queued for explicit user confirmation; it has not been applied.",
    ))
}

fn bridge_data_result(data: Value, detail: &str) -> Value {
    let structured = json!({
        "data": data,
        "detail": detail,
    });
    tool_success(structured, detail)
}

fn bounded_name_argument<'a>(
    arguments: &'a Value,
    key: &str,
    max_chars: usize,
) -> Result<&'a str, String> {
    let value = arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| format!("{key} must be a string"))?;
    if value.is_empty()
        || value.chars().count() > max_chars
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{key} must contain 1..={max_chars} non-control characters"
        ));
    }
    Ok(value)
}

fn optional_u32_argument(
    arguments: &Value,
    key: &str,
    minimum: u32,
    maximum: u32,
) -> Result<Option<u32>, String> {
    let Some(value) = arguments.get(key) else {
        return Ok(None);
    };
    let value = value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value >= minimum && *value <= maximum)
        .ok_or_else(|| {
            format!("{key} must be an integer between {minimum} and {maximum}")
        })?;
    Ok(Some(value))
}

fn bounded_enum_argument<'a>(
    arguments: &'a Value,
    key: &str,
    allowed: &[&str],
) -> Result<&'a str, String> {
    let value = arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be a string"))?;
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "Unsupported {key} '{value}'. Allowed values: {}",
            allowed.join(", ")
        ))
    }
}

fn interaction_tool_result(
    event_sequence: u64,
    blender_result: Option<Result<Value, crate::blender::BlenderError>>,
    detail: String,
) -> Value {
    let (blender_dispatched, final_detail) = match blender_result {
        Some(Ok(_)) => (true, detail),
        Some(Err(error)) => (
            false,
            format!("{detail}; Blender bridge reported {}", error.message),
        ),
        None => (false, format!("{detail}; Blender is disabled")),
    };
    let structured = json!({
        "ui_dispatched": true,
        "blender_dispatched": blender_dispatched,
        "event_sequence": event_sequence,
        "detail": final_detail,
    });
    tool_success(structured.clone(), &final_detail)
}

fn tool_success(structured: Value, text: &str) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "structuredContent": structured,
        "isError": false
    })
}

fn tool_error(message: &str) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": message
        }],
        "isError": true
    })
}

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn origin_is_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) else {
        return true;
    };
    origin == "null"
        || origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("http://localhost:")
        || origin.starts_with("https://127.0.0.1:")
        || origin.starts_with("https://localhost:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_catalog_has_bounded_inputs_and_annotations() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 19);
        assert_eq!(tools[0]["name"], "ravichara_set_expression");
        assert_eq!(tools[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(tools[2]["annotations"]["readOnlyHint"], true);
        let reaction = tools
            .iter()
            .find(|tool| tool["name"] == "ravichara_apply_reaction")
            .unwrap();
        assert_eq!(
            reaction["inputSchema"]["additionalProperties"],
            false
        );
        let graph = tools
            .iter()
            .find(|tool| {
                tool["name"] == "ravichara_propose_shader_graph"
            })
            .unwrap();
        assert_eq!(graph["annotations"]["destructiveHint"], false);
        assert!(tools.iter().any(|tool| {
            tool["name"] == "ravichara_propose_material_adjust"
        }));
        let behavior = tools
            .iter()
            .find(|tool| tool["name"] == "ravichara_apply_behavior_plan")
            .unwrap();
        assert_eq!(behavior["inputSchema"]["additionalProperties"], false);
        assert_eq!(behavior["annotations"]["destructiveHint"], false);
        let keyframe = &behavior["inputSchema"]["properties"]["keyframes"]
            ["items"];
        assert_eq!(keyframe["required"], json!(["at"]));
        assert_eq!(
            keyframe["properties"]["translations"]["maxItems"],
            6
        );
        assert_eq!(
            keyframe["properties"]["translations"]["items"]
                ["required"],
            json!(["role", "offset"])
        );
        assert_eq!(
            keyframe["anyOf"],
            json!([
                {"required": ["rotations"]},
                {"required": ["translations"]}
            ])
        );
    }

    #[test]
    fn enum_arguments_reject_unlisted_controls() {
        let arguments = json!({"expression": "../../unsafe"});
        let error =
            bounded_enum_argument(&arguments, "expression", EXPRESSIONS).unwrap_err();
        assert!(error.contains("Unsupported expression"));
    }
}
