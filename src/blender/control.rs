use super::{
    virtual_c_expression_preset, BlenderBridge, BlenderError,
};
use crate::mcp::{EXPRESSIONS, MOTIONS};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

pub const SHADER_TEMPLATES: &[&str] = &["skin", "fabric", "metal"];
pub const MAX_PENDING_PROPOSALS: usize = 16;
pub const PROPOSAL_TTL_SECONDS: i64 = 600;
pub const BEHAVIOR_ROLES: &[&str] = &[
    "center",
    "upper_body",
    "upper_body2",
    "neck",
    "head",
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
    "toe_left",
    "toe_right",
];

pub const BEHAVIOR_CONTROL_ROLES: &[&str] = &[
    "center",
    "hand_left",
    "hand_right",
    "foot_left",
    "foot_right",
];

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorRotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bone: Option<String>,
    pub degrees: [f64; 3],
}

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorTranslation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bone: Option<String>,
    pub offset: [f64; 3],
}

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorKeyframe {
    pub at: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rotations: Vec<BehaviorRotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub translations: Vec<BehaviorTranslation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorMorph {
    pub name: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorPlan {
    pub intent: String,
    pub expression: String,
    pub expression_intensity: f64,
    pub hold_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_motion: Option<String>,
    pub duration_scale: f64,
    pub easing: String,
    pub keyframes: Vec<BehaviorKeyframe>,
    pub morphs: Vec<BehaviorMorph>,
    #[serde(skip_serializing)]
    pub normalized_adjustments: u32,
}

impl BehaviorPlan {
    pub fn fallback(
        expression: impl Into<String>,
        motion: Option<String>,
    ) -> Self {
        Self {
            intent: "bounded fallback reaction".to_string(),
            expression: expression.into(),
            expression_intensity: 1.0,
            hold_seconds: 4.0,
            fallback_motion: motion,
            duration_scale: 1.0,
            easing: "SINE".to_string(),
            keyframes: Vec::new(),
            morphs: Vec::new(),
            normalized_adjustments: 0,
        }
    }

    pub fn has_generated_channels(&self) -> bool {
        !self.keyframes.is_empty() || !self.morphs.is_empty()
    }
}

pub fn normalize_behavior_plan(
    arguments: &Value,
    capabilities: &Value,
) -> Result<BehaviorPlan, String> {
    let intent = arguments
        .get("intent")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("natural conversational reaction");
    if intent.chars().count() > 160 || intent.chars().any(char::is_control) {
        return Err("intent must contain at most 160 non-control characters".to_string());
    }
    let expression = arguments
        .get("expression")
        .and_then(Value::as_str)
        .unwrap_or("neutral");
    if !EXPRESSIONS.contains(&expression) {
        return Err(format!(
            "Unsupported expression '{expression}'. Allowed values: {}",
            EXPRESSIONS.join(", ")
        ));
    }
    let expression_intensity = optional_f64(
        arguments,
        "expression_intensity",
        0.0,
        1.0,
    )?
    .unwrap_or(0.75);
    let hold_seconds = optional_f64(arguments, "hold_seconds", 0.25, 12.0)?
        .unwrap_or(4.0);
    let duration_scale = optional_f64(arguments, "duration_scale", 0.5, 2.0)?
        .unwrap_or(1.0);
    let easing = arguments
        .get("easing")
        .and_then(Value::as_str)
        .unwrap_or("SINE");
    if !matches!(easing, "SINE" | "BEZIER" | "LINEAR") {
        return Err("easing must be SINE, BEZIER, or LINEAR".to_string());
    }
    let fallback_motion = match arguments.get("fallback_motion") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value == "none" => None,
        Some(Value::String(value)) if MOTIONS.contains(&value.as_str()) => {
            Some(value.clone())
        }
        _ => {
            return Err(format!(
                "fallback_motion must be null or one of {}",
                MOTIONS.join(", ")
            ))
        }
    };

    let generated = capabilities
        .get("generated_behavior")
        .and_then(Value::as_object);
    let generated_available = generated
        .and_then(|value| value.get("available"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let role_limits = generated
        .and_then(|value| value.get("bone_roles"))
        .and_then(Value::as_object)
        .map(|roles| {
            roles
                .iter()
                .filter(|(role, _)| BEHAVIOR_ROLES.contains(&role.as_str()))
                .map(|(role, specification)| {
                    (role.clone(), degree_limits(specification, [20.0; 3]))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let exact_bone_limits = generated
        .and_then(|value| value.get("bones"))
        .and_then(Value::as_array)
        .map(|bones| {
            bones
                .iter()
                .filter_map(|bone| {
                    let name = bone.get("name")?.as_str()?.trim();
                    if name.is_empty()
                        || name.chars().count() > 128
                        || name.chars().any(char::is_control)
                    {
                        return None;
                    }
                    Some((
                        name.to_string(),
                        degree_limits(bone, [20.0; 3]),
                    ))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let control_role_limits = generated
        .and_then(|value| value.get("control_roles"))
        .and_then(Value::as_object)
        .map(|roles| {
            roles
                .iter()
                .filter(|(role, _)| BEHAVIOR_CONTROL_ROLES.contains(&role.as_str()))
                .map(|(role, specification)| {
                    (
                        role.clone(),
                        normalized_offset_limits(specification, [1.0; 3]),
                    )
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let exact_morphs = generated
        .and_then(|value| value.get("morphs"))
        .and_then(Value::as_array)
        .map(|morphs| {
            morphs
                .iter()
                .filter_map(|morph| {
                    morph
                        .get("name")
                        .and_then(Value::as_str)
                        .or_else(|| morph.as_str())
                })
                .map(ToString::to_string)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    let raw_keyframes = arguments
        .get("keyframes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if raw_keyframes.len() > 8 {
        return Err("keyframes must contain at most 8 entries".to_string());
    }
    let raw_morphs = arguments
        .get("morphs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if raw_morphs.len() > 6 {
        return Err("morphs must contain at most 6 entries".to_string());
    }
    if (!raw_keyframes.is_empty() || !raw_morphs.is_empty())
        && !generated_available
    {
        return Err(
            "the installed Blender bridge does not advertise generated behavior support"
                .to_string(),
        );
    }

    let mut normalized_adjustments = 0_u32;
    let mut keyframes = Vec::with_capacity(raw_keyframes.len());
    let mut previous_at = -1.0_f64;
    for keyframe in raw_keyframes {
        let at = keyframe
            .get("at")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .ok_or_else(|| "each keyframe.at must be between 0 and 1".to_string())?;
        if at <= previous_at {
            return Err("keyframe.at values must be strictly increasing".to_string());
        }
        previous_at = at;
        let raw_rotations = keyframe
            .get("rotations")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let raw_translations = keyframe
            .get("translations")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if raw_rotations.len() > 12 {
            return Err(
                "each keyframe may contain at most 12 rotations".to_string(),
            );
        }
        if raw_translations.len() > 6 {
            return Err("each keyframe may contain at most 6 translations".to_string());
        }
        if raw_rotations.is_empty() && raw_translations.is_empty() {
            return Err(
                "each keyframe requires at least one rotation or translation".to_string(),
            );
        }
        let mut targets = HashSet::new();
        let mut rotations = Vec::with_capacity(raw_rotations.len());
        for rotation in raw_rotations {
            let role = rotation
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let bone = rotation
                .get("bone")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if role.is_some() == bone.is_some() {
                return Err(
                    "each rotation must specify exactly one of role or bone"
                        .to_string(),
                );
            }
            let (role, bone, limits, identity) = if let Some(role) = role {
                let limits = role_limits.get(role).copied().ok_or_else(|| {
                    format!("role '{role}' is not available on the active armature")
                })?;
                (
                    Some(role.to_string()),
                    None,
                    limits,
                    format!("role:{role}"),
                )
            } else {
                let bone = bone.unwrap_or_default();
                let limits = exact_bone_limits.get(bone).copied().ok_or_else(|| {
                    format!("bone '{bone}' is not in the advertised controllable catalog")
                })?;
                (
                    None,
                    Some(bone.to_string()),
                    limits,
                    format!("bone:{bone}"),
                )
            };
            if !targets.insert(identity) {
                return Err("a keyframe cannot target the same bone twice".to_string());
            }
            let raw_degrees = rotation
                .get("degrees")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| "rotation.degrees must contain exactly 3 numbers".to_string())?;
            let mut degrees = [0.0_f64; 3];
            for axis in 0..3 {
                let value = raw_degrees[axis]
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| "rotation degrees must be finite numbers".to_string())?;
                degrees[axis] = value.clamp(-limits[axis], limits[axis]);
                if (degrees[axis] - value).abs() > f64::EPSILON {
                    normalized_adjustments = normalized_adjustments.saturating_add(1);
                }
            }
            rotations.push(BehaviorRotation {
                role,
                bone,
                degrees,
            });
        }
        let mut translation_targets = HashSet::new();
        let mut translations = Vec::with_capacity(raw_translations.len());
        for translation in raw_translations {
            let role = translation
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let bone = translation
                .get("bone")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if role.is_some() == bone.is_some() {
                return Err(
                    "each translation must specify exactly one of role or bone"
                        .to_string(),
                );
            }
            let (role, bone, limits, identity) = if let Some(role) = role {
                let limits = control_role_limits.get(role).copied().ok_or_else(|| {
                    format!("control role '{role}' is not available on the active armature")
                })?;
                (
                    Some(role.to_string()),
                    None,
                    limits,
                    format!("control:{role}"),
                )
            } else {
                return Err(
                    "exact control-bone translation requires an advertised control catalog"
                        .to_string(),
                );
            };
            if !translation_targets.insert(identity) {
                return Err(
                    "a keyframe cannot translate the same control twice".to_string(),
                );
            }
            let raw_offset = translation
                .get("offset")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| {
                    "translation.offset must contain exactly 3 normalized numbers"
                        .to_string()
                })?;
            let mut offset = [0.0_f64; 3];
            for axis in 0..3 {
                let value = raw_offset[axis]
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| {
                        "translation offsets must be finite numbers".to_string()
                    })?;
                offset[axis] = value.clamp(-limits[axis], limits[axis]);
                if (offset[axis] - value).abs() > f64::EPSILON {
                    normalized_adjustments = normalized_adjustments.saturating_add(1);
                }
            }
            translations.push(BehaviorTranslation { role, bone, offset });
        }
        keyframes.push(BehaviorKeyframe {
            at,
            rotations,
            translations,
        });
    }

    let mut morph_names = HashSet::new();
    let mut morphs = Vec::with_capacity(raw_morphs.len());
    for morph in raw_morphs {
        let name = morph
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .ok_or_else(|| "each morph requires an exact name".to_string())?;
        if !exact_morphs.contains(name) {
            return Err(format!(
                "morph '{name}' is not in the advertised exact morph catalog"
            ));
        }
        if !morph_names.insert(name.to_string()) {
            return Err(format!("morph '{name}' was specified more than once"));
        }
        let weight = morph
            .get("weight")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .ok_or_else(|| "morph weight must be between 0 and 1".to_string())?;
        morphs.push(BehaviorMorph {
            name: name.to_string(),
            weight,
        });
    }

    Ok(BehaviorPlan {
        intent: intent.to_string(),
        expression: expression.to_string(),
        expression_intensity,
        hold_seconds,
        fallback_motion,
        duration_scale,
        easing: easing.to_string(),
        keyframes,
        morphs,
        normalized_adjustments,
    })
}

/// Ensure that an explicit lower-body request remains visible on an IK rig.
///
/// Small planners frequently return an FK leg rotation even when RigProfile has
/// withheld that chain, or they emit a technically valid but imperceptible IK
/// offset.  This post-validation step does not invent a bone.  It only uses an
/// advertised semantic foot control whose axes are character-left, forward and
/// up, and it keeps every value inside the bridge's published limits.
pub fn ensure_explicit_leg_motion(
    plan: &mut BehaviorPlan,
    capabilities: &Value,
    user_message: &str,
) -> bool {
    let user = user_message.to_lowercase();
    let explicit_leg_motion = [
        "踢", "抬腿", "抬脚", "抬膝", "伸腿", "腿部动作", "动一下腿",
        "走一步", "走两步", "走几步", "走路", "走一下", "迈步",
        "kick", "raise leg", "lift leg", "raise foot",
        "lift foot", "move leg", "move your leg", "take a step",
    ]
    .iter()
    .any(|term| user.contains(term));
    if !explicit_leg_motion {
        return false;
    }

    let control_roles = capabilities
        .pointer("/generated_behavior/control_roles")
        .and_then(Value::as_object);
    let Some(control_roles) = control_roles else {
        return false;
    };
    let wants_left = ["左腿", "左脚", "左足", "左膝", "left leg", "left foot"]
        .iter()
        .any(|term| user.contains(term));
    let wants_right = ["右腿", "右脚", "右足", "右膝", "right leg", "right foot"]
        .iter()
        .any(|term| user.contains(term));
    let preferred = if wants_left && !wants_right {
        "foot_left"
    } else {
        "foot_right"
    };
    let role = if control_roles.contains_key(preferred) {
        preferred
    } else if preferred == "foot_left" && control_roles.contains_key("foot_right") {
        "foot_right"
    } else if preferred == "foot_right" && control_roles.contains_key("foot_left") {
        "foot_left"
    } else {
        return false;
    };
    let specification = &control_roles[role];
    let semantic_axes = specification
        .get("normalized_axes")
        .and_then(Value::as_array)
        .map(|axes| {
            axes.iter().filter_map(Value::as_str).collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if semantic_axes
        != ["character_left", "character_forward", "character_up"]
    {
        // Older bridge versions exposed pose-channel axes.  Synthesizing a
        // semantic kick against those coordinates would recreate the original
        // backwards-leg defect, so remain fail-closed.
        return false;
    }
    let limits = normalized_offset_limits(specification, [0.0; 3]);
    let is_kick = ["踢", "kick"].iter().any(|term| user.contains(term));
    let forward_peak: f64 = if is_kick { 0.46 } else { 0.18 };
    let up_peak: f64 = if is_kick { 0.30 } else { 0.34 };
    let peak = [
        0.0,
        forward_peak.min(limits[1] * 0.78),
        up_peak.min(limits[2] * 0.72),
    ];
    if peak[1] <= 1e-6 && peak[2] <= 1e-6 {
        return false;
    }

    if plan.keyframes.is_empty() {
        plan.keyframes = [0.18, 0.52, 0.82]
            .into_iter()
            .map(|at| BehaviorKeyframe {
                at,
                rotations: Vec::new(),
                translations: Vec::new(),
            })
            .collect();
    }

    let mut changed = false;
    for keyframe in &mut plan.keyframes {
        let phase = (PI * keyframe.at.clamp(0.0, 1.0)).sin().max(0.0).powf(1.25);
        let desired = [0.0, peak[1] * phase, peak[2] * phase];
        if let Some(existing) = keyframe.translations.iter_mut().find(|translation| {
            translation.role.as_deref() == Some(role)
        }) {
            for axis in 1..=2 {
                if existing.offset[axis].abs() + 1e-6 < desired[axis].abs() {
                    existing.offset[axis] = desired[axis];
                    changed = true;
                }
            }
        } else if keyframe.translations.len() < 6 {
            keyframe.translations.push(BehaviorTranslation {
                role: Some(role.to_string()),
                bone: None,
                offset: desired,
            });
            changed = true;
        }
    }
    if changed {
        if is_kick {
            plan.fallback_motion = Some("kick".to_string());
        }
        plan.normalized_adjustments = plan.normalized_adjustments.saturating_add(1);
    }
    changed
}

/// Make explicit arm requests visible without relying on imported PMX Euler axes.
///
/// Protocol v4 arm roles advertise anatomical axes.  This validator only adds
/// coordinated keyframes when that contract is present; older bridges remain
/// fail-closed because applying these values as local XYZ would be unsafe.
pub fn ensure_explicit_arm_motion(
    plan: &mut BehaviorPlan,
    capabilities: &Value,
    user_message: &str,
) -> bool {
    let user = user_message.to_lowercase();
    let explicit = [
        "举手", "抬手", "挥手", "挥一下", "挥动", "招手", "摆手", "伸手", "手臂",
        "胳膊", "握拳", "拳头", "raise hand", "wave", "move your arm",
        "move arm", "fist",
    ]
    .iter()
    .any(|term| user.contains(term));
    if !explicit {
        return false;
    }
    let Some(roles) = capabilities
        .pointer("/generated_behavior/bone_roles")
        .and_then(Value::as_object)
    else {
        return false;
    };

    let wants_left = ["左手", "左臂", "左胳膊", "left hand", "left arm"]
        .iter()
        .any(|term| user.contains(term));
    let wants_right = ["右手", "右臂", "右胳膊", "right hand", "right arm"]
        .iter()
        .any(|term| user.contains(term));
    let wants_both = ["双手", "两只手", "两手", "双臂", "both hands", "both arms"]
        .iter()
        .any(|term| user.contains(term));
    let requested_sides: Vec<&str> = if wants_both || (wants_left && wants_right) {
        vec!["left", "right"]
    } else if wants_left {
        vec!["left"]
    } else {
        vec!["right"]
    };
    let is_wave = ["挥手", "挥一下", "挥动", "招手", "摆手", "wave"]
        .iter()
        .any(|term| user.contains(term));
    let is_fist = ["握拳", "拳头", "fist"]
        .iter()
        .any(|term| user.contains(term));

    let sides = requested_sides
        .into_iter()
        .filter(|side| {
            semantic_arm_role_available(
                roles,
                &format!("upper_arm_{side}"),
                &["front_raise", "outward_raise", "axial_twist"],
            )
        })
        .collect::<Vec<_>>();
    if sides.is_empty() {
        return false;
    }
    if plan.keyframes.is_empty() {
        plan.keyframes = [0.16, 0.38, 0.62, 0.84]
            .into_iter()
            .map(|at| BehaviorKeyframe {
                at,
                rotations: Vec::new(),
                translations: Vec::new(),
            })
            .collect();
    }

    let mut changed = false;
    for side in sides {
        let upper_role = format!("upper_arm_{side}");
        let forearm_role = format!("forearm_{side}");
        let hand_role = format!("hand_{side}");
        let upper_limits = degree_limits(&roles[&upper_role], [0.0; 3]);
        let forearm_available = semantic_arm_role_available(
            roles,
            &forearm_role,
            &["forward_elbow_bend", "outward_bias", "axial_twist"],
        );
        let forearm_limits = forearm_available
            .then(|| degree_limits(&roles[&forearm_role], [0.0; 3]));
        let hand_available = is_wave && semantic_arm_role_available(
            roles,
            &hand_role,
            &["forward_wrist_bend", "outward_wave", "axial_twist"],
        );
        let hand_limits = hand_available
            .then(|| degree_limits(&roles[&hand_role], [0.0; 3]));

        for (index, keyframe) in plan.keyframes.iter_mut().enumerate() {
            let phase = (PI * keyframe.at.clamp(0.0, 1.0))
                .sin()
                .max(0.0)
                .powf(1.1);
            let (front_peak, outward_peak, elbow_peak) = if is_wave {
                (16.0_f64, 112.0_f64, 48.0_f64)
            } else if is_fist {
                (30.0_f64, 42.0_f64, 62.0_f64)
            } else {
                (18.0_f64, 118.0_f64, 34.0_f64)
            };
            let upper = [
                (front_peak * phase).min(upper_limits[0]),
                (outward_peak * phase).min(upper_limits[1]),
                0.0,
            ];
            changed |= upsert_semantic_rotation(keyframe, &upper_role, upper);

            if let Some(limits) = forearm_limits {
                let forearm = [
                    (elbow_peak * phase).min(limits[0]),
                    0.0,
                    0.0,
                ];
                changed |= upsert_semantic_rotation(keyframe, &forearm_role, forearm);
            }
            if let Some(limits) = hand_limits {
                let wave_sign = if index % 2 == 0 { -1.0 } else { 1.0 };
                let hand = [
                    (5.0_f64 * phase).min(limits[0]),
                    (wave_sign * 28.0 * phase).clamp(-limits[1], limits[1]),
                    0.0,
                ];
                changed |= upsert_semantic_rotation(keyframe, &hand_role, hand);
            }
        }
    }
    if changed {
        if is_wave {
            plan.fallback_motion = Some("wave".to_string());
        }
        plan.normalized_adjustments = plan.normalized_adjustments.saturating_add(1);
    }
    changed
}

fn semantic_arm_role_available(
    roles: &Map<String, Value>,
    role: &str,
    expected_axes: &[&str; 3],
) -> bool {
    let Some(specification) = roles.get(role) else {
        return false;
    };
    if specification
        .get("rotation_space")
        .and_then(Value::as_str)
        != Some("character-semantic-degrees")
    {
        return false;
    }
    specification
        .get("rotation_axes")
        .and_then(Value::as_array)
        .is_some_and(|axes| {
            axes.len() == 3
                && axes
                    .iter()
                    .zip(expected_axes.iter())
                    .all(|(actual, expected)| actual.as_str() == Some(*expected))
        })
}

fn upsert_semantic_rotation(
    keyframe: &mut BehaviorKeyframe,
    role: &str,
    desired: [f64; 3],
) -> bool {
    if let Some(existing) = keyframe
        .rotations
        .iter_mut()
        .find(|rotation| rotation.role.as_deref() == Some(role))
    {
        let existing_motion = existing.degrees[0].abs() + existing.degrees[1].abs();
        let desired_motion = desired[0].abs() + desired[1].abs();
        if existing_motion + 1e-6 < desired_motion * 0.72 {
            existing.degrees = desired;
            return true;
        }
        return false;
    }
    if keyframe.rotations.len() >= 12 {
        return false;
    }
    keyframe.rotations.push(BehaviorRotation {
        role: Some(role.to_string()),
        bone: None,
        degrees: desired,
    });
    true
}

fn degree_limits(specification: &Value, fallback: [f64; 3]) -> [f64; 3] {
    let Some(values) = specification
        .get("max_degrees")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
    else {
        return fallback;
    };
    let mut limits = fallback;
    for axis in 0..3 {
        if let Some(value) = values[axis]
            .as_f64()
            .filter(|value| value.is_finite() && (0.0..=135.0).contains(value))
        {
            limits[axis] = value;
        }
    }
    limits
}

fn normalized_offset_limits(
    specification: &Value,
    fallback: [f64; 3],
) -> [f64; 3] {
    let Some(values) = specification
        .get("normalized_offset_limit")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
    else {
        return fallback;
    };
    let mut limits = fallback;
    for axis in 0..3 {
        if let Some(value) = values[axis]
            .as_f64()
            .filter(|value| value.is_finite() && (0.0..=2.0).contains(value))
        {
            limits[axis] = value;
        }
    }
    limits
}

const PROPOSAL_KINDS: &[&str] = &[
    "rig_pose",
    "motion_action",
    "morph_weights",
    "expression_sequence",
    "shader_template",
    "shader_graph",
    "material_adjust",
    "clear_animation",
];

#[derive(Debug, Clone, Serialize)]
pub struct BlenderProposal {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub parameters: Value,
    pub risk: String,
    pub created_at: String,
    pub expires_at: String,
}

impl BlenderProposal {
    pub fn is_expired(&self) -> bool {
        chrono::DateTime::parse_from_rfc3339(&self.expires_at)
            .map(|expiry| expiry < chrono::Utc::now())
            .unwrap_or(true)
    }
}

pub fn build_proposal(
    kind: &str,
    arguments: &Value,
) -> Result<BlenderProposal, String> {
    if !PROPOSAL_KINDS.contains(&kind) {
        return Err(format!("unsupported Blender proposal kind '{kind}'"));
    }
    let (parameters, summary, risk) = match kind {
        "rig_pose" => normalize_rig_pose(arguments)?,
        "motion_action" => normalize_motion_action(arguments)?,
        "morph_weights" => normalize_morph_weights(arguments)?,
        "expression_sequence" => normalize_expression_sequence(arguments)?,
        "shader_template" => normalize_shader_template(arguments)?,
        "shader_graph" => normalize_shader_graph(arguments)?,
        "material_adjust" => normalize_material_adjust(arguments)?,
        "clear_animation" => (
            json!({}),
            "Clear the active armature animation without deleting its Action"
                .to_string(),
            "destructive".to_string(),
        ),
        _ => unreachable!(),
    };
    let created = chrono::Utc::now();
    let sequence = super::REQUEST_COUNTER.fetch_add(
        1,
        std::sync::atomic::Ordering::Relaxed,
    );
    Ok(BlenderProposal {
        id: format!("bp-{}-{sequence}", created.timestamp_millis()),
        kind: kind.to_string(),
        summary,
        parameters,
        risk,
        created_at: created.to_rfc3339(),
        expires_at: (created
            + chrono::Duration::seconds(PROPOSAL_TTL_SECONDS))
        .to_rfc3339(),
    })
}

impl BlenderBridge {
    pub async fn list_scene_objects(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Value, BlenderError> {
        self.send_command(
            "scene.inspect",
            json!({
                "limit": limit.clamp(1, 100),
                "offset": offset.min(1_000_000),
            }),
        )
        .await
    }

    pub async fn inspect_rig(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Value, BlenderError> {
        let target = self.resolve_model_target().await?;
        self.send_command(
            "rig.inspect",
            json!({
                "armature": target.armature,
                "limit": limit.clamp(1, 200),
                "offset": offset.min(1_000_000),
            }),
        )
        .await
    }

    pub async fn inspect_expression_inventory(
        &self,
        query: &str,
        category: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Value, BlenderError> {
        let target = self.resolve_model_target().await?;
        self.send_command(
            "expression.inspect",
            json!({
                "model": target.root,
                "query": query,
                "category": category,
                "limit": limit.clamp(1, 100),
                "offset": offset.min(1_000_000),
            }),
        )
        .await
    }

    pub async fn list_materials(
        &self,
        object: &str,
    ) -> Result<Value, BlenderError> {
        self.send_command("material.list", json!({"object": object}))
            .await
    }

    pub async fn inspect_shader(
        &self,
        object: &str,
        material: Option<&str>,
        slot: Option<u32>,
    ) -> Result<Value, BlenderError> {
        let mut parameters = Map::new();
        parameters.insert("object".to_string(), json!(object));
        if let Some(material) = material {
            parameters.insert("material".to_string(), json!(material));
        }
        if let Some(slot) = slot {
            parameters.insert("slot".to_string(), json!(slot));
        }
        self.send_command("shader.inspect", Value::Object(parameters))
            .await
    }

    pub async fn execute_proposal(
        &self,
        proposal: &BlenderProposal,
    ) -> Result<Value, BlenderError> {
        match proposal.kind.as_str() {
            "rig_pose" => {
                let target = self.resolve_model_target().await?;
                let parameters =
                    with_target(&proposal.parameters, "armature", &target.armature);
                self.send_command("rig.set_pose", parameters).await
            }
            "motion_action" => {
                let target = self.resolve_model_target().await?;
                let parameters =
                    with_target(&proposal.parameters, "armature", &target.armature);
                self.send_command("rig.create_action", parameters).await
            }
            "morph_weights" => {
                let target = self.resolve_model_target().await?;
                let parameters =
                    with_target(&proposal.parameters, "model", &target.root);
                self.send_command("expression.set_weights", parameters)
                    .await
            }
            "expression_sequence" => {
                let target = self.resolve_model_target().await?;
                let mut parameters = proposal
                    .parameters
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                parameters.insert("model".to_string(), json!(target.root));
                if let Some(cues) = parameters
                    .get_mut("cues")
                    .and_then(Value::as_array_mut)
                {
                    for cue in cues {
                        if let Some(expression) = cue
                            .get("expression")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                        {
                            cue["expression"] =
                                json!(virtual_c_expression_preset(&expression));
                        }
                    }
                }
                self.send_command(
                    "expression.create_sequence",
                    Value::Object(parameters),
                )
                .await
            }
            "shader_template" => {
                self.send_command(
                    "shader.apply_template",
                    proposal.parameters.clone(),
                )
                .await
            }
            "shader_graph" => {
                self.send_command(
                    "shader.apply_graph",
                    proposal.parameters.clone(),
                )
                .await
            }
            "material_adjust" => {
                let target = self.resolve_model_target().await?;
                let parameters =
                    with_target(&proposal.parameters, "model", &target.root);
                self.send_compatible_command(
                    "ravichara.material.adjust",
                    "everchara.material.adjust",
                    parameters,
                )
                .await
            }
            "clear_animation" => {
                let target = self.resolve_model_target().await?;
                self.send_command(
                    "rig.clear_animation",
                    json!({
                        "armature": target.armature,
                        "delete_action": false,
                    }),
                )
                .await
            }
            other => Err(BlenderError {
                code: "invalid_proposal".to_string(),
                message: format!("unsupported proposal kind '{other}'"),
            }),
        }
    }
}

fn normalize_rig_pose(
    arguments: &Value,
) -> Result<(Value, String, String), String> {
    let bone = bounded_name(arguments, "bone", 128)?;
    let mut parameters = Map::new();
    parameters.insert("bone".to_string(), json!(bone));
    let mut supplied = false;
    if let Some(value) = arguments.get("location") {
        parameters.insert(
            "location".to_string(),
            json!(bounded_vector(value, 3, 1.0, "location")?),
        );
        supplied = true;
    }
    if let Some(value) = arguments.get("rotation_degrees") {
        parameters.insert(
            "rotation_degrees".to_string(),
            json!(bounded_vector(
                value,
                3,
                180.0,
                "rotation_degrees"
            )?),
        );
        supplied = true;
    }
    if !supplied {
        return Err(
            "rig_pose requires location or rotation_degrees".to_string()
        );
    }
    if let Some(frame) = optional_u32(arguments, "frame", 1, 1_000_000)? {
        parameters.insert("frame".to_string(), json!(frame));
    }
    Ok((
        Value::Object(parameters),
        format!("Set pose for bone '{bone}'"),
        "persistent".to_string(),
    ))
}

fn normalize_motion_action(
    arguments: &Value,
) -> Result<(Value, String, String), String> {
    let preset = bounded_enum(arguments, "preset", MOTIONS)?;
    let start_frame =
        optional_u32(arguments, "start_frame", 1, 1_000_000)?.unwrap_or(1);
    let duration =
        optional_u32(arguments, "duration", 2, 240)?.unwrap_or(48);
    let intensity = optional_f64(arguments, "intensity", 0.0, 1.0)?
        .unwrap_or(1.0);
    let side = arguments
        .get("side")
        .and_then(Value::as_str)
        .unwrap_or("right");
    if !matches!(side, "left" | "right") {
        return Err("side must be left or right".to_string());
    }
    let mut parameters = json!({
        "preset": preset,
        "start_frame": start_frame,
        "duration": duration,
        "intensity": intensity,
        "side": side,
    });
    if arguments.get("action_name").is_some() {
        parameters["action_name"] =
            json!(bounded_name(arguments, "action_name", 128)?);
    }
    Ok((
        parameters,
        format!(
            "Create persistent '{preset}' Action at frame {start_frame}"
        ),
        "persistent".to_string(),
    ))
}

fn normalize_expression_sequence(
    arguments: &Value,
) -> Result<(Value, String, String), String> {
    let cues = arguments
        .get("cues")
        .and_then(Value::as_array)
        .ok_or_else(|| "cues must be an array".to_string())?;
    if !(1..=16).contains(&cues.len()) {
        return Err("cues must contain between 1 and 16 entries".to_string());
    }
    let mut normalized = Vec::with_capacity(cues.len());
    let mut previous = 0_u32;
    for cue in cues {
        let frame = required_u32(cue, "frame", 1, 1_000_000)?;
        if frame <= previous {
            return Err(
                "cue frames must be unique and strictly increasing".to_string(),
            );
        }
        previous = frame;
        let expression = bounded_enum(cue, "expression", EXPRESSIONS)?;
        let intensity =
            optional_f64(cue, "intensity", 0.0, 1.0)?.unwrap_or(1.0);
        normalized.push(json!({
            "frame": frame,
            "expression": expression,
            "intensity": intensity,
        }));
    }
    let transition_frames =
        optional_u32(arguments, "transition_frames", 0, 120)?.unwrap_or(4);
    let interpolation = arguments
        .get("interpolation")
        .and_then(Value::as_str)
        .unwrap_or("BEZIER");
    if !matches!(interpolation, "BEZIER" | "LINEAR" | "CONSTANT") {
        return Err(
            "interpolation must be BEZIER, LINEAR, or CONSTANT".to_string(),
        );
    }
    Ok((
        json!({
            "cues": normalized,
            "replace": true,
            "transition_frames": transition_frames,
            "interpolation": interpolation,
        }),
        format!(
            "Create a persistent facial sequence with {} cues",
            cues.len()
        ),
        "persistent".to_string(),
    ))
}

fn normalize_morph_weights(
    arguments: &Value,
) -> Result<(Value, String, String), String> {
    let weights = arguments
        .get("weights")
        .and_then(Value::as_array)
        .ok_or_else(|| "weights must be an array".to_string())?;
    if !(1..=16).contains(&weights.len()) {
        return Err(
            "weights must contain between 1 and 16 morph entries".to_string(),
        );
    }
    let mut normalized = Vec::with_capacity(weights.len());
    let mut names = std::collections::HashSet::new();
    for item in weights {
        let morph = bounded_name(item, "morph", 128)?;
        if !names.insert(morph.clone()) {
            return Err(format!("morph '{morph}' was specified more than once"));
        }
        let weight = optional_f64(item, "weight", 0.0, 1.0)?
            .ok_or_else(|| "each morph requires weight".to_string())?;
        normalized.push(json!({"morph": morph, "weight": weight}));
    }
    let reset_scope = arguments
        .get("reset_scope")
        .and_then(Value::as_str)
        .unwrap_or("target_categories");
    if !matches!(
        reset_scope,
        "none" | "target_categories" | "all_facial"
    ) {
        return Err(
            "reset_scope must be none, target_categories, or all_facial"
                .to_string(),
        );
    }
    let transition_frames =
        optional_u32(arguments, "transition_frames", 0, 120)?.unwrap_or(4);
    let interpolation = arguments
        .get("interpolation")
        .and_then(Value::as_str)
        .unwrap_or("BEZIER");
    if !matches!(interpolation, "BEZIER" | "LINEAR" | "CONSTANT") {
        return Err(
            "interpolation must be BEZIER, LINEAR, or CONSTANT".to_string(),
        );
    }
    let mut parameters = json!({
        "weights": normalized,
        "reset_scope": reset_scope,
        "transition_frames": transition_frames,
        "interpolation": interpolation,
    });
    if let Some(frame) = optional_u32(arguments, "frame", 1, 1_000_000)? {
        parameters["frame"] = json!(frame);
    }
    Ok((
        parameters,
        format!("Set {} exact facial morph weights", weights.len()),
        "persistent".to_string(),
    ))
}

fn normalize_shader_template(
    arguments: &Value,
) -> Result<(Value, String, String), String> {
    let object = bounded_name(arguments, "object", 128)?;
    let template = bounded_enum(arguments, "template", SHADER_TEMPLATES)?;
    let parameters = material_target_parameters(arguments, &object)?;
    let mut parameters = parameters.as_object().cloned().unwrap_or_default();
    parameters.insert("template".to_string(), json!(template));
    Ok((
        Value::Object(parameters),
        format!("Duplicate the target material and apply '{template}' shader"),
        "persistent".to_string(),
    ))
}

fn normalize_shader_graph(
    arguments: &Value,
) -> Result<(Value, String, String), String> {
    let object = bounded_name(arguments, "object", 128)?;
    let graph = arguments
        .get("graph")
        .cloned()
        .ok_or_else(|| "graph is required".to_string())?;
    let serialized = serde_json::to_vec(&graph)
        .map_err(|_| "graph must be valid JSON".to_string())?;
    if serialized.len() > 32 * 1024 {
        return Err("graph must not exceed 32 KiB".to_string());
    }
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "graph.nodes must be an array".to_string())?;
    let links = graph
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| "graph.links must be an array".to_string())?;
    if !(1..=32).contains(&nodes.len()) || links.len() > 64 {
        return Err(
            "shader graph requires 1..=32 nodes and at most 64 links"
                .to_string(),
        );
    }
    for node in nodes {
        bounded_name(node, "id", 64)?;
        bounded_name(node, "type", 128)?;
    }
    let node_count = nodes.len();
    let parameters = material_target_parameters(arguments, &object)?;
    let mut parameters = parameters.as_object().cloned().unwrap_or_default();
    parameters.insert("graph".to_string(), graph);
    Ok((
        Value::Object(parameters),
        format!(
            "Duplicate the target material and apply a custom {}-node shader graph",
            node_count
        ),
        "persistent".to_string(),
    ))
}

fn normalize_material_adjust(
    arguments: &Value,
) -> Result<(Value, String, String), String> {
    const TARGETS: &[&str] = &["hair", "skin", "clothes", "eyes", "all"];
    let target = bounded_enum(arguments, "target", TARGETS)?;
    let brightness = optional_f64(arguments, "brightness", 0.25, 2.0)?
        .ok_or_else(|| "material_adjust requires brightness".to_string())?;
    Ok((
        json!({
            "target": target,
            "brightness": brightness,
        }),
        format!(
            "Adjust the avatar {target} material brightness to {brightness:.2}x"
        ),
        "persistent".to_string(),
    ))
}

fn material_target_parameters(
    arguments: &Value,
    object: &str,
) -> Result<Value, String> {
    let mut parameters = Map::new();
    parameters.insert("object".to_string(), json!(object));
    if arguments.get("material").is_some() {
        parameters.insert(
            "material".to_string(),
            json!(bounded_name(arguments, "material", 128)?),
        );
    }
    if let Some(slot) = optional_u32(arguments, "slot", 0, 255)? {
        parameters.insert("slot".to_string(), json!(slot));
    }
    if arguments.get("output_name").is_some() {
        parameters.insert(
            "output_name".to_string(),
            json!(bounded_name(arguments, "output_name", 128)?),
        );
    }
    Ok(Value::Object(parameters))
}

fn with_target(parameters: &Value, key: &str, value: &str) -> Value {
    let mut output = parameters.as_object().cloned().unwrap_or_default();
    output.insert(key.to_string(), json!(value));
    Value::Object(output)
}

fn bounded_name(
    arguments: &Value,
    key: &str,
    max_chars: usize,
) -> Result<String, String> {
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
    Ok(value.to_string())
}

fn bounded_enum<'a>(
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
            "{key} must be one of {}",
            allowed.join(", ")
        ))
    }
}

fn bounded_vector(
    value: &Value,
    length: usize,
    bound: f64,
    key: &str,
) -> Result<Vec<f64>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| format!("{key} must be an array"))?;
    if values.len() != length {
        return Err(format!("{key} must contain exactly {length} numbers"));
    }
    values
        .iter()
        .map(|value| {
            let number = value
                .as_f64()
                .filter(|number| number.is_finite() && number.abs() <= bound)
                .ok_or_else(|| {
                    format!("{key} components must be within +/-{bound}")
                })?;
            Ok(number)
        })
        .collect()
}

fn required_u32(
    arguments: &Value,
    key: &str,
    minimum: u32,
    maximum: u32,
) -> Result<u32, String> {
    optional_u32(arguments, key, minimum, maximum)?
        .ok_or_else(|| format!("{key} is required"))
}

fn optional_u32(
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
        .filter(|value| (*value >= minimum) && (*value <= maximum))
        .ok_or_else(|| {
            format!("{key} must be an integer between {minimum} and {maximum}")
        })?;
    Ok(Some(value))
}

fn optional_f64(
    arguments: &Value,
    key: &str,
    minimum: f64,
    maximum: f64,
) -> Result<Option<f64>, String> {
    let Some(value) = arguments.get(key) else {
        return Ok(None);
    };
    let value = value
        .as_f64()
        .filter(|value| {
            value.is_finite() && *value >= minimum && *value <= maximum
        })
        .ok_or_else(|| {
            format!("{key} must be between {minimum} and {maximum}")
        })?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposals_are_bounded_and_expiring() {
        let proposal = build_proposal(
            "shader_template",
            &json!({"object": "Body", "slot": 0, "template": "fabric"}),
        )
        .unwrap();
        assert_eq!(proposal.kind, "shader_template");
        assert_eq!(proposal.parameters["object"], "Body");
        assert!(!proposal.is_expired());
    }

    #[test]
    fn graph_and_pose_validation_rejects_unbounded_inputs() {
        assert!(build_proposal(
            "rig_pose",
            &json!({"bone": "Head", "rotation_degrees": [0, 999, 0]}),
        )
        .is_err());
        assert!(build_proposal(
            "shader_graph",
            &json!({
                "object": "Body",
                "graph": {"nodes": [], "links": []}
            }),
        )
        .is_err());
    }

    #[test]
    fn expression_cues_require_strict_frame_order() {
        assert!(build_proposal(
            "expression_sequence",
            &json!({
                "cues": [
                    {"frame": 10, "expression": "happy"},
                    {"frame": 10, "expression": "neutral"}
                ]
            }),
        )
        .is_err());
    }

    #[test]
    fn morph_weights_require_unique_bounded_channels() {
        assert!(build_proposal(
            "morph_weights",
            &json!({
                "weights": [
                    {"morph": "Blink", "weight": 0.5},
                    {"morph": "Blink", "weight": 1.0}
                ]
            }),
        )
        .is_err());
    }

    #[test]
    fn semantic_material_adjustments_are_bounded() {
        let proposal = build_proposal(
            "material_adjust",
            &json!({"target": "hair", "brightness": 1.18}),
        )
        .unwrap();
        assert_eq!(proposal.parameters["target"], "hair");
        assert!(build_proposal(
            "material_adjust",
            &json!({"target": "unknown", "brightness": 1.18}),
        )
        .is_err());
        assert!(build_proposal(
            "material_adjust",
            &json!({"target": "hair", "brightness": 10.0}),
        )
        .is_err());
    }

    #[test]
    fn generated_behavior_uses_only_advertised_avatar_channels() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "bone_roles": {
                    "head": {
                        "bone": "Head",
                        "max_degrees": [30.0, 45.0, 35.0]
                    }
                },
                "bones": [{
                    "name": "CustomCapeBone",
                    "max_degrees": [12.0, 12.0, 12.0]
                }],
                "morphs": [{"name": "SoftSmile"}]
            }
        });
        let plan = normalize_behavior_plan(
            &json!({
                "intent": "gentle acknowledgement",
                "expression": "happy",
                "expression_intensity": 0.7,
                "fallback_motion": "nod",
                "keyframes": [
                    {
                        "at": 0.25,
                        "rotations": [
                            {"role": "head", "degrees": [100.0, 4.0, 5.0]},
                            {"bone": "CustomCapeBone", "degrees": [3.0, 2.0, 1.0]}
                        ]
                    },
                    {
                        "at": 0.75,
                        "rotations": [
                            {"role": "head", "degrees": [-3.0, -2.0, 0.0]}
                        ]
                    }
                ],
                "morphs": [{"name": "SoftSmile", "weight": 0.5}]
            }),
            &capabilities,
        )
        .unwrap();
        assert!(plan.has_generated_channels());
        assert_eq!(plan.keyframes[0].rotations[0].degrees[0], 30.0);
        assert_eq!(plan.normalized_adjustments, 1);
        assert_eq!(plan.morphs[0].name, "SoftSmile");
    }

    #[test]
    fn generated_behavior_rejects_unadvertised_targets_and_bad_timing() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "bone_roles": {"head": {"max_degrees": [30, 45, 35]}},
                "bones": [],
                "morphs": []
            }
        });
        assert!(normalize_behavior_plan(
            &json!({
                "expression": "neutral",
                "keyframes": [{
                    "at": 0.5,
                    "rotations": [{"bone": "Invented", "degrees": [1, 2, 3]}]
                }],
                "morphs": []
            }),
            &capabilities,
        )
        .is_err());
        assert!(normalize_behavior_plan(
            &json!({
                "expression": "neutral",
                "keyframes": [
                    {"at": 0.5, "rotations": [{"role": "head", "degrees": [1, 2, 3]}]},
                    {"at": 0.5, "rotations": [{"role": "head", "degrees": [0, 0, 0]}]}
                ],
                "morphs": []
            }),
            &capabilities,
        )
        .is_err());

        let fallback = normalize_behavior_plan(
            &json!({
                "expression": "neutral",
                "fallback_motion": "nod",
                "keyframes": [],
                "morphs": []
            }),
            &json!({}),
        )
        .unwrap();
        assert!(!fallback.has_generated_channels());
        assert_eq!(fallback.fallback_motion.as_deref(), Some("nod"));
    }

    #[test]
    fn ik_controls_accept_only_advertised_translation_channels() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "bone_roles": {
                    "head": {"max_degrees": [30, 45, 35]}
                },
                "control_roles": {
                    "hand_left": {
                        "bone": "LeftArmIK",
                        "active_channel": "ik",
                        "normalized_offset_limit": [0.5, 0.75, 0.4]
                    }
                },
                "bones": [],
                "morphs": []
            }
        });
        let plan = normalize_behavior_plan(
            &json!({
                "expression": "happy",
                "keyframes": [{
                    "at": 0.5,
                    "translations": [{
                        "role": "hand_left",
                        "offset": [0.8, -0.25, 0.9]
                    }]
                }]
            }),
            &capabilities,
        )
        .unwrap();
        assert_eq!(plan.keyframes[0].translations[0].offset, [0.5, -0.25, 0.4]);
        assert_eq!(plan.normalized_adjustments, 2);

        let rejected = normalize_behavior_plan(
            &json!({
                "expression": "neutral",
                "keyframes": [{
                    "at": 0.5,
                    "rotations": [{
                        "role": "upper_arm_left",
                        "degrees": [5, 0, 0]
                    }]
                }]
            }),
            &capabilities,
        );
        assert!(rejected.is_err());
    }

    #[test]
    fn pmx_fixed_axis_and_locked_control_channels_remain_zero() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "bone_roles": {
                    "knee_left": {
                        "drive_mode": "mmd_fixed_axis",
                        "max_degrees": [70.0, 0.0, 0.0],
                        "fixed_axis_local": [0.0, 1.0, 0.0]
                    }
                },
                "control_roles": {
                    "foot_left": {
                        "bone": "LeftLegIK",
                        "active_channel": "ik",
                        "normalized_offset_limit": [1.0, 0.0, 0.5]
                    }
                },
                "bones": [],
                "morphs": []
            }
        });
        let plan = normalize_behavior_plan(
            &json!({
                "expression": "neutral",
                "keyframes": [{
                    "at": 0.5,
                    "rotations": [{
                        "role": "knee_left",
                        "degrees": [30.0, 12.0, -8.0]
                    }],
                    "translations": [{
                        "role": "foot_left",
                        "offset": [0.4, 0.8, -0.7]
                    }]
                }]
            }),
            &capabilities,
        )
        .unwrap();
        assert_eq!(
            plan.keyframes[0].rotations[0].degrees,
            [30.0, 0.0, 0.0]
        );
        assert_eq!(
            plan.keyframes[0].translations[0].offset,
            [0.4, 0.0, -0.5]
        );
        // Two forbidden fixed-axis rotation components, one locked IK axis,
        // and one over-limit IK component are each normalized independently.
        assert_eq!(plan.normalized_adjustments, 4);
    }

    #[test]
    fn explicit_leg_requests_get_visible_semantic_ik_motion() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "control_roles": {
                    "foot_right": {
                        "bone": "RightLegIK",
                        "normalized_axes": [
                            "character_left", "character_forward", "character_up"
                        ],
                        "normalized_offset_limit": [1.0, 1.0, 1.0]
                    }
                }
            }
        });
        let mut plan = BehaviorPlan::fallback("neutral", Some("kick".to_string()));
        assert!(ensure_explicit_leg_motion(
            &mut plan,
            &capabilities,
            "请明显地踢一下右腿",
        ));
        assert_eq!(plan.keyframes.len(), 3);
        let peak = plan
            .keyframes
            .iter()
            .flat_map(|frame| &frame.translations)
            .find(|translation| translation.offset[1] > 0.4)
            .expect("a visible forward kick peak should be synthesized");
        assert_eq!(peak.role.as_deref(), Some("foot_right"));
        assert!(peak.offset[2] > 0.2);
        assert_eq!(plan.normalized_adjustments, 1);
    }

    #[test]
    fn explicit_leg_synthesis_rejects_legacy_pose_channel_axes() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "control_roles": {
                    "foot_right": {
                        "bone": "RightLegIK",
                        "normalized_axes": [
                            "pose_channel_x", "pose_channel_y", "pose_channel_z"
                        ],
                        "normalized_offset_limit": [1.0, 1.0, 1.0]
                    }
                }
            }
        });
        let mut plan = BehaviorPlan::fallback("neutral", Some("kick".to_string()));
        assert!(!ensure_explicit_leg_motion(
            &mut plan,
            &capabilities,
            "kick with the right leg",
        ));
        assert!(plan.keyframes.is_empty());
    }

    #[test]
    fn explicit_wave_uses_coordinated_semantic_arm_chain() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "bone_roles": {
                    "upper_arm_right": {
                        "rotation_space": "character-semantic-degrees",
                        "rotation_axes": ["front_raise", "outward_raise", "axial_twist"],
                        "max_degrees": [100.0, 125.0, 35.0]
                    },
                    "forearm_right": {
                        "rotation_space": "character-semantic-degrees",
                        "rotation_axes": ["forward_elbow_bend", "outward_bias", "axial_twist"],
                        "max_degrees": [115.0, 30.0, 30.0]
                    },
                    "hand_right": {
                        "rotation_space": "character-semantic-degrees",
                        "rotation_axes": ["forward_wrist_bend", "outward_wave", "axial_twist"],
                        "max_degrees": [35.0, 40.0, 25.0]
                    }
                }
            }
        });
        let mut plan = BehaviorPlan::fallback("happy", None);
        assert!(ensure_explicit_arm_motion(
            &mut plan,
            &capabilities,
            "请挥一下右手",
        ));
        assert_eq!(plan.keyframes.len(), 4);
        let peak = &plan.keyframes[1];
        let upper = peak
            .rotations
            .iter()
            .find(|rotation| rotation.role.as_deref() == Some("upper_arm_right"))
            .unwrap();
        let forearm = peak
            .rotations
            .iter()
            .find(|rotation| rotation.role.as_deref() == Some("forearm_right"))
            .unwrap();
        assert!(upper.degrees[1] > 80.0);
        assert!(forearm.degrees[0] > 35.0);
        assert!(peak.rotations.iter().any(|rotation| {
            rotation.role.as_deref() == Some("hand_right")
                && rotation.degrees[1].abs() > 15.0
        }));
    }

    #[test]
    fn explicit_arm_synthesis_rejects_legacy_local_axes() {
        let capabilities = json!({
            "generated_behavior": {
                "available": true,
                "bone_roles": {
                    "upper_arm_right": {
                        "rotation_space": "relative-idle-pose-basis-degrees",
                        "rotation_axes": ["local_x", "local_y", "local_z"],
                        "max_degrees": [65.0, 55.0, 85.0]
                    }
                }
            }
        });
        let mut plan = BehaviorPlan::fallback("neutral", None);
        assert!(!ensure_explicit_arm_motion(
            &mut plan,
            &capabilities,
            "raise your right hand",
        ));
        assert!(plan
            .keyframes
            .iter()
            .all(|keyframe| keyframe.rotations.is_empty()));
    }
}
