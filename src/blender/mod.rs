use crate::config::BlenderConfig;
use crate::mcp::MOTIONS;
use crate::persona::PersonaCard;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

mod control;
pub use control::{
    build_proposal, ensure_explicit_arm_motion, ensure_explicit_leg_motion,
    normalize_behavior_plan,
    BehaviorPlan, BlenderProposal, BEHAVIOR_ROLES, MAX_PENDING_PROPOSALS,
    SHADER_TEMPLATES,
};

const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    Off,
    Pose,
    Snapshot,
    Animation,
    Auto,
}

impl RenderMode {
    pub fn from_str(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "pose" => RenderMode::Pose,
            "snapshot" => RenderMode::Snapshot,
            "animation" => RenderMode::Animation,
            "auto" => RenderMode::Auto,
            _ => RenderMode::Off,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlenderBridge {
    host: String,
    port: u16,
    token: String,
    pub profile: String,
    pub mode: RenderMode,
    pub model_name: String,
    playback_range: (u32, u32),
    action_range: (u32, u32),
    transition_frames: u32,
    expression_hold_seconds: f64,
    timeout: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlenderHealth {
    pub enabled: bool,
    pub reachable: bool,
    pub profile: String,
    pub render_mode: RenderMode,
    pub address: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorExecution {
    pub idle_configured: bool,
    pub expression_dispatched: bool,
    pub generated_attempted: bool,
    pub generated_dispatched: bool,
    pub fallback_used: bool,
    pub fallback_motion: Option<String>,
    pub errors: Vec<String>,
}

impl BehaviorExecution {
    pub fn dispatched(&self) -> bool {
        self.expression_dispatched
            || self.generated_dispatched
            || self.fallback_used
    }
}

#[derive(Debug, Clone)]
pub struct BlenderError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct BlenderModelTarget {
    root: String,
    armature: String,
}

impl fmt::Display for BlenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for BlenderError {}

impl BlenderBridge {
    pub fn from_config(config: &BlenderConfig) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            token: config.effective_token(),
            profile: config.profile.clone(),
            mode: RenderMode::from_str(&config.render_mode),
            model_name: config.model_name.clone(),
            playback_range: (
                config.playback_start_frame,
                config.playback_end_frame,
            ),
            action_range: (config.action_start_frame, config.action_end_frame),
            transition_frames: config.transition_frames,
            expression_hold_seconds: config.auto_downgrade_ms as f64 / 1000.0,
            timeout: Duration::from_secs(4),
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.mode, RenderMode::Off)
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub async fn ping(&self) -> Result<Value, BlenderError> {
        self.send_command("system.ping", json!({})).await
    }

    pub async fn inspect_scene(&self) -> Result<Value, BlenderError> {
        self.send_command("scene.inspect", json!({})).await
    }

    pub async fn inspect_expressions(&self) -> Result<Value, BlenderError> {
        if self.profile == "virtual_c" {
            let target = self.resolve_model_target().await?;
            self.send_command(
                "expression.inspect",
                json!({"model": target.root, "limit": 100}),
            )
            .await
        } else {
            self.send_command(
                "expression.inspect",
                json!({"model_name": self.model_name}),
            )
            .await
        }
    }

    pub async fn inspect_avatar_capabilities(
        &self,
    ) -> Result<Value, BlenderError> {
        let target = self.resolve_model_target().await?;
        let parameters = json!({
            "model": target.root,
            "armature": target.armature,
        });
        let generated_result = match self
            .send_command("ravichara.avatar.capabilities", parameters.clone())
            .await
        {
            Ok(value) => Ok(value),
            Err(error) if unsupported_command(&error) => self
                .send_command("everchara.avatar.capabilities", parameters)
                .await,
            Err(error) => Err(error),
        };
        let generated_behavior = match generated_result {
            Ok(mut capabilities) => {
                if let Some(object) = capabilities.as_object_mut() {
                    object.insert("available".to_string(), json!(true));
                }
                capabilities
            }
            Err(error) => json!({
                "available": false,
                "error_code": error.code,
                "detail": error.message,
                "required_addon_version": "0.5.5",
            }),
        };
        // Current add-ons return all rig, morph, and expression information in
        // one main-thread traversal. Fall back to the legacy inspections only
        // when the generated-capability command is unavailable.
        let (bone_count, active_action, expression_presets) =
            if generated_behavior.get("available").and_then(Value::as_bool) == Some(true) {
                (
                    generated_behavior
                        .pointer("/rig_profile/bone_count")
                        .cloned()
                        .unwrap_or(Value::Null),
                    generated_behavior
                        .get("active_action")
                        .cloned()
                        .unwrap_or(Value::Null),
                    generated_behavior
                        .get("expression_presets")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                )
            } else {
                let expressions = self
                    .send_command(
                        "expression.inspect",
                        json!({"model": target.root, "limit": 1}),
                    )
                    .await?;
                let rig = self
                    .send_command(
                        "rig.inspect",
                        json!({"armature": target.armature, "limit": 1}),
                    )
                    .await?;
                (
                    rig.get("total").cloned().unwrap_or(Value::Null),
                    rig.get("action").cloned().unwrap_or(Value::Null),
                    expressions
                        .get("preset_support")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                )
            };
        Ok(json!({
            "model": target.root,
            "armature": target.armature,
            "bone_count": bone_count,
            "active_action": active_action,
            "expression_presets": expression_presets,
            "generated_behavior": generated_behavior,
            "motion_presets": MOTIONS,
            "shader_templates": SHADER_TEMPLATES,
            "mutation_policy": {
                "automatic": [
                    "bounded expression",
                    "capability-validated transient behavior plan",
                    "one-shot preset fallback"
                ],
                "confirmation_required": [
                    "bone pose",
                    "persistent action",
                    "expression sequence",
                    "shader template",
                    "shader graph",
                    "semantic material adjustment",
                    "clear animation"
                ]
            }
        }))
    }

    pub async fn list_models(&self) -> Result<Value, BlenderError> {
        self.send_command("mmd.list_models", json!({"limit": 100}))
            .await
    }

    pub async fn preview_status(&self) -> Result<Value, BlenderError> {
        self.send_command("render.preview.status", json!({})).await
    }

    pub async fn render_preview(
        &self,
        width: u32,
        height: u32,
        refresh: bool,
        transparent: bool,
    ) -> Result<Value, BlenderError> {
        let mut preview_bridge = self.clone();
        preview_bridge.timeout = Duration::from_secs(60);
        preview_bridge
            .send_command(
                "render.preview",
                json!({
                    "width": width.clamp(128, 1024),
                    "height": height.clamp(128, 1024),
                    "refresh": refresh,
                    "transparent": transparent,
                }),
            )
            .await
    }

    pub async fn render_viewport(
        &self,
        width: u32,
        height: u32,
        transparent: bool,
    ) -> Result<Value, BlenderError> {
        let width = width.clamp(128, 1024);
        let height = height.clamp(128, 1024);
        let mut preview_bridge = self.clone();
        // A newly opened EEVEE scene may spend tens of seconds compiling
        // shaders before its first managed camera frame. Subsequent frames are
        // fast, but aborting the socket at 15 s leaves an expensive command
        // running in Blender while the client queues another one.
        preview_bridge.timeout = Duration::from_secs(60);
        match preview_bridge
            .send_command(
                "render.viewport",
                json!({
                    "width": width,
                    "height": height,
                    "transparent": transparent,
                }),
            )
            .await
        {
            Ok(frame) => Ok(frame),
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "unknown_command"
                        | "unsupported_command"
                        | "command_not_found"
                        | "invalid_command"
                ) =>
            {
                tracing::debug!(
                    "render.viewport is unavailable; falling back to render.preview"
                );
                let mut frame = self
                    .render_preview(width, height, true, transparent)
                    .await?;
                if let Some(object) = frame.as_object_mut() {
                    object.insert(
                        "capture_mode".to_string(),
                        json!("render-fallback"),
                    );
                }
                Ok(frame)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn apply_expression(
        &self,
        preset: &str,
        weight: f32,
    ) -> Result<Value, BlenderError> {
        if self.profile == "virtual_c" {
            let target = self.resolve_model_target().await?;
            let requested = virtual_c_expression_preset(preset);
            let expression = self
                .resolve_supported_expression(&target.root, requested)
                .await?;
            let parameters = json!({
                "model": target.root,
                "expression": expression,
                "intensity": weight.clamp(0.0, 1.0),
                "replace": true,
                "reset_expression": "neutral",
                "hold_seconds": self.expression_hold_seconds.clamp(0.25, 60.0),
            });
            match self
                .send_compatible_command(
                    "ravichara.expression.apply_timed",
                    "everchara.expression.apply_timed",
                    parameters.clone(),
                )
                .await
            {
                Ok(result) => Ok(result),
                Err(error) if unsupported_command(&error) => {
                    self.send_command(
                        "expression.apply",
                        json!({
                            "model": parameters["model"],
                            "expression": parameters["expression"],
                            "intensity": parameters["intensity"],
                            "replace": true,
                        }),
                    )
                    .await
                }
                Err(error) => Err(error),
            }
        } else {
            self.send_command(
                "expression.apply",
                json!({
                    "preset": preset,
                    "weight": weight.clamp(0.0, 1.0),
                    "model_name": self.model_name,
                }),
            )
            .await
        }
    }

    pub async fn play_motion(&self, preset: &str) -> Result<Value, BlenderError> {
        if self.profile == "virtual_c" {
            let target = self.resolve_model_target().await?;
            let parameters = json!({
                "armature": target.armature,
                "preset": preset,
                "playback_start": self.playback_range.0,
                "action_start": self.action_range.0,
                "action_end": self.action_range.1,
                "playback_end": self.playback_range.1,
                "transition_frames": self.transition_frames,
                "intensity": 1.0,
            });
            match self
                .send_compatible_command(
                    "ravichara.animation.play_once",
                    "everchara.animation.play_once",
                    parameters,
                )
                .await
            {
                Ok(result) => Ok(result),
                Err(error) if unsupported_command(&error) => {
                    let action = self
                        .send_command(
                            "rig.create_action",
                            json!({
                                "armature": target.armature,
                                "preset": preset,
                                "start_frame": self.action_range.0,
                                "duration": self.action_range.1 - self.action_range.0,
                                "intensity": 1.0,
                            }),
                        )
                        .await?;
                    let playback = self
                        .send_command(
                            "rig.play",
                            json!({
                                "start_frame": self.playback_range.0,
                                "end_frame": self.playback_range.1,
                            }),
                        )
                        .await?;
                    Ok(json!({
                        "action": action,
                        "playback": playback,
                        "lifecycle": "legacy-fallback",
                    }))
                }
                Err(error) => Err(error),
            }
        } else {
            self.send_command(
                "rig.play",
                json!({
                    "preset": preset,
                    "model_name": self.model_name,
                }),
            )
            .await
        }
    }

    async fn play_generated_behavior(
        &self,
        plan: &BehaviorPlan,
    ) -> Result<Value, BlenderError> {
        if self.profile != "virtual_c" {
            return Err(BlenderError {
                code: "unsupported_profile".to_string(),
                message: "generated behavior requires the virtual_c profile"
                    .to_string(),
            });
        }
        let parameters = self.behavior_parameters(plan).await?;
        self.send_compatible_command(
            "ravichara.behavior.play_plan",
            "everchara.behavior.play_plan",
            parameters,
        )
        .await
    }

    async fn behavior_parameters(
        &self,
        plan: &BehaviorPlan,
    ) -> Result<Value, BlenderError> {
        let target = self.resolve_model_target().await?;
        let configured_duration = self
            .action_range
            .1
            .saturating_sub(self.action_range.0)
            .max(8);
        let duration = ((configured_duration as f64) * plan.duration_scale)
            .round()
            .clamp(8.0, 240.0) as u32;
        let transition_in = self
            .action_range
            .0
            .saturating_sub(self.playback_range.0)
            .max(self.transition_frames);
        let transition_out = self
            .playback_range
            .1
            .saturating_sub(self.action_range.1)
            .max(self.transition_frames);
        let action_start = self.action_range.0.max(1);
        let action_end = action_start.saturating_add(duration);
        let playback_start = action_start.saturating_sub(transition_in).max(1);
        let playback_end = action_end.saturating_add(transition_out);
        let mut parameters = serde_json::to_value(plan).map_err(|error| {
            BlenderError {
                code: "encode_failed".to_string(),
                message: error.to_string(),
            }
        })?;
        let object = parameters.as_object_mut().ok_or_else(|| BlenderError {
            code: "encode_failed".to_string(),
            message: "behavior plan did not serialize to an object".to_string(),
        })?;
        object.insert(
            "expression".to_string(),
            json!(virtual_c_expression_preset(&plan.expression)),
        );
        object.insert("model".to_string(), json!(target.root));
        object.insert("armature".to_string(), json!(target.armature));
        object.insert("playback_start".to_string(), json!(playback_start));
        object.insert("action_start".to_string(), json!(action_start));
        object.insert("action_end".to_string(), json!(action_end));
        object.insert("playback_end".to_string(), json!(playback_end));
        object.insert(
            "transition_frames".to_string(),
            json!(self.transition_frames),
        );
        Ok(parameters)
    }

    /// Executes idle, expression, generated channels, and fallback as one
    /// main-thread Blender command. The monotonic generation lets the add-on
    /// discard work that reached its queue after a newer chat turn.
    pub async fn execute_behavior_with_generation(
        &self,
        persona: &PersonaCard,
        plan: &BehaviorPlan,
        generation: u64,
    ) -> BehaviorExecution {
        if self.profile != "virtual_c" {
            return self.execute_behavior(persona, plan).await;
        }
        let mut execution = BehaviorExecution {
            idle_configured: false,
            expression_dispatched: false,
            generated_attempted: plan.has_generated_channels(),
            generated_dispatched: false,
            fallback_used: false,
            fallback_motion: plan.fallback_motion.clone(),
            errors: Vec::new(),
        };
        let mut behavior = match self.behavior_parameters(plan).await {
            Ok(parameters) => parameters,
            Err(error) => {
                execution.errors.push(format!("behavior target: {error}"));
                return execution;
            }
        };
        if let Some(object) = behavior.as_object_mut() {
            object.insert("generation".to_string(), json!(generation));
        }
        let parameters = json!({
            "generation": generation,
            "idle": persona_idle_profile(persona),
            "behavior": behavior,
        });
        let result = match self
            .send_command("ravichara.behavior.execute", parameters.clone())
            .await
        {
            Ok(value) => Ok(value),
            Err(error) if unsupported_command(&error) => self
                .send_command("everchara.behavior.execute", parameters)
                .await,
            Err(error) => Err(error),
        };
        let response = match result {
            Ok(value) => value,
            Err(error) if unsupported_command(&error) => {
                // Compatibility with add-on versions before atomic behavior
                // execution. This path is intentionally retained for one
                // migration cycle.
                return self.execute_behavior(persona, plan).await;
            }
            Err(error) => {
                execution.errors.push(format!("atomic behavior: {error}"));
                return execution;
            }
        };
        if response.get("ignored").and_then(Value::as_bool) == Some(true) {
            execution.errors.push(format!(
                "behavior generation {generation} was superseded before execution"
            ));
            return execution;
        }
        execution.idle_configured = response
            .get("idle")
            .is_some_and(|value| !value.is_null());
        execution.expression_dispatched = response
            .get("expression")
            .is_some_and(|value| !value.is_null());
        if let Some(behavior) = response.get("behavior").filter(|value| !value.is_null()) {
            execution.generated_dispatched = behavior
                .get("action")
                .is_some_and(|value| !value.is_null())
                || behavior
                    .get("applied_morphs")
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty());
        }
        execution.fallback_used = response
            .get("fallback")
            .is_some_and(|value| !value.is_null());
        if let Some(errors) = response.get("errors").and_then(Value::as_array) {
            execution.errors.extend(
                errors
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string),
            );
        }
        execution
    }

    pub async fn execute_behavior(
        &self,
        persona: &PersonaCard,
        plan: &BehaviorPlan,
    ) -> BehaviorExecution {
        let mut execution = BehaviorExecution {
            idle_configured: false,
            expression_dispatched: false,
            generated_attempted: plan.has_generated_channels(),
            generated_dispatched: false,
            fallback_used: false,
            fallback_motion: plan.fallback_motion.clone(),
            errors: Vec::new(),
        };
        match self.configure_idle(&persona_idle_profile(persona)).await {
            Ok(_) => execution.idle_configured = true,
            Err(error) => execution.errors.push(format!("idle: {error}")),
        }
        match self
            .apply_expression(
                &plan.expression,
                plan.expression_intensity.clamp(0.0, 1.0) as f32,
            )
            .await
        {
            Ok(_) => execution.expression_dispatched = true,
            Err(error) => execution.errors.push(format!("expression: {error}")),
        }

        let mut needs_fallback = !plan.has_generated_channels();
        if plan.has_generated_channels() {
            match self.play_generated_behavior(plan).await {
                Ok(_) => execution.generated_dispatched = true,
                Err(error) => {
                    execution
                        .errors
                        .push(format!("generated behavior: {error}"));
                    needs_fallback = true;
                }
            }
        }
        if needs_fallback {
            if let Some(motion) = plan.fallback_motion.as_deref() {
                match self.play_motion(motion).await {
                    Ok(_) => execution.fallback_used = true,
                    Err(error) => execution
                        .errors
                        .push(format!("fallback motion: {error}")),
                }
            }
        }
        execution
    }

    pub async fn stop_motion(&self) -> Result<Value, BlenderError> {
        self.send_compatible_command(
            "ravichara.animation.stop",
            "everchara.animation.stop",
            json!({}),
        )
        .await
    }

    pub async fn configure_idle(
        &self,
        profile: &Value,
    ) -> Result<Value, BlenderError> {
        if self.profile != "virtual_c" {
            return Ok(json!({
                "configured": false,
                "detail": "idle profiles require the virtual_c Blender profile",
            }));
        }
        let target = self.resolve_model_target().await?;
        let mut parameters = profile.as_object().cloned().unwrap_or_default();
        parameters.insert("armature".to_string(), json!(target.armature));
        self.send_compatible_command(
            "ravichara.animation.set_idle",
            "everchara.animation.set_idle",
            Value::Object(parameters),
        )
        .await
    }

    async fn resolve_model_target(&self) -> Result<BlenderModelTarget, BlenderError> {
        let response = self.list_models().await?;
        let models = response
            .get("models")
            .and_then(Value::as_array)
            .ok_or_else(|| BlenderError {
                code: "invalid_model_list".to_string(),
                message: "mmd.list_models returned no models array".to_string(),
            })?;
        let requested = self.model_name.trim();
        let candidates = models
            .iter()
            .filter(|model| {
                if requested.is_empty() {
                    return true;
                }
                model.get("root").and_then(Value::as_str) == Some(requested)
                    || model.get("name_j").and_then(Value::as_str) == Some(requested)
                    || model.get("name_e").and_then(Value::as_str) == Some(requested)
                    || model
                        .get("armatures")
                        .and_then(Value::as_array)
                        .is_some_and(|armatures| {
                            armatures.iter().any(|armature| {
                                armature.as_str() == Some(requested)
                            })
                        })
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(BlenderError {
                code: "model_not_found".to_string(),
                message: if requested.is_empty() {
                    "no MMD model was found in the Blender scene".to_string()
                } else {
                    format!("configured Blender model '{requested}' was not found")
                },
            });
        }
        if candidates.len() > 1 {
            let names = candidates
                .iter()
                .filter_map(|model| model.get("root").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(BlenderError {
                code: "model_ambiguous".to_string(),
                message: format!(
                    "multiple MMD models are available ({names}); set blender.model_name"
                ),
            });
        }
        let model = candidates[0];
        let root = model
            .get("root")
            .and_then(Value::as_str)
            .ok_or_else(|| BlenderError {
                code: "invalid_model_list".to_string(),
                message: "model entry is missing root".to_string(),
            })?
            .to_string();
        let armatures = model
            .get("armatures")
            .and_then(Value::as_array)
            .ok_or_else(|| BlenderError {
                code: "armature_not_found".to_string(),
                message: format!("MMD model '{root}' has no armatures list"),
            })?;
        let armature = if !requested.is_empty()
            && armatures
                .iter()
                .any(|value| value.as_str() == Some(requested))
        {
            requested.to_string()
        } else if armatures.len() == 1 {
            armatures[0]
                .as_str()
                .unwrap_or_default()
                .to_string()
        } else {
            return Err(BlenderError {
                code: "armature_ambiguous".to_string(),
                message: format!(
                    "MMD model '{root}' has {} armatures; set blender.model_name to one armature",
                    armatures.len()
                ),
            });
        };
        if armature.is_empty() {
            return Err(BlenderError {
                code: "armature_not_found".to_string(),
                message: format!("MMD model '{root}' has no usable armature"),
            });
        }
        Ok(BlenderModelTarget { root, armature })
    }

    async fn resolve_supported_expression(
        &self,
        model: &str,
        requested: &str,
    ) -> Result<String, BlenderError> {
        let inspection = self
            .send_command(
                "expression.inspect",
                json!({"model": model, "limit": 1}),
            )
            .await?;
        let Some(support) = inspection.get("preset_support") else {
            return Ok(requested.to_string());
        };
        for candidate in virtual_c_expression_fallbacks(requested) {
            if support
                .get(candidate)
                .and_then(|value| value.get("supported"))
                .and_then(Value::as_bool)
                == Some(true)
            {
                return Ok(candidate.to_string());
            }
        }
        Err(BlenderError {
            code: "expression_unsupported".to_string(),
            message: format!(
                "the active MMD model supports no safe fallback for expression '{requested}'"
            ),
        })
    }

    async fn send_compatible_command(
        &self,
        primary: &str,
        legacy: &str,
        parameters: Value,
    ) -> Result<Value, BlenderError> {
        match self.send_command(primary, parameters.clone()).await {
            Ok(value) => Ok(value),
            Err(error) if unsupported_command(&error) => {
                self.send_command(legacy, parameters).await
            }
            Err(error) => Err(error),
        }
    }

    pub async fn send_command(
        &self,
        command: &str,
        parameters: Value,
    ) -> Result<Value, BlenderError> {
        let address = self.address();
        let mut stream = timeout(self.timeout, TcpStream::connect(&address))
            .await
            .map_err(|_| BlenderError {
                code: "connection_timeout".to_string(),
                message: format!("timed out connecting to {address}"),
            })?
            .map_err(|error| BlenderError {
                code: "connection_failed".to_string(),
                message: format!("failed to connect to {address}: {error}"),
            })?;

        let request = json!({
            "version": 1,
            "request_id": request_id(),
            "command": command,
            "params": parameters,
            "token": self.token,
        });
        let encoded = serde_json::to_vec(&request).map_err(|error| BlenderError {
            code: "encode_failed".to_string(),
            message: error.to_string(),
        })?;
        if encoded.len() > MAX_FRAME_BYTES {
            return Err(BlenderError {
                code: "frame_too_large".to_string(),
                message: "request exceeded the bridge frame limit".to_string(),
            });
        }

        let length = (encoded.len() as u32).to_be_bytes();
        timeout(self.timeout, async {
            stream.write_all(&length).await?;
            stream.write_all(&encoded).await?;
            stream.flush().await
        })
        .await
        .map_err(|_| BlenderError {
            code: "write_timeout".to_string(),
            message: "timed out sending the Blender command".to_string(),
        })?
        .map_err(|error| BlenderError {
            code: "write_failed".to_string(),
            message: error.to_string(),
        })?;

        let mut length_bytes = [0_u8; 4];
        timeout(self.timeout, stream.read_exact(&mut length_bytes))
            .await
            .map_err(|_| BlenderError {
                code: "read_timeout".to_string(),
                message: "timed out waiting for Blender response".to_string(),
            })?
            .map_err(|error| BlenderError {
                code: "read_failed".to_string(),
                message: error.to_string(),
            })?;
        let response_length = u32::from_be_bytes(length_bytes) as usize;
        if response_length == 0 || response_length > MAX_FRAME_BYTES {
            return Err(BlenderError {
                code: "invalid_frame".to_string(),
                message: format!("invalid Blender response size: {response_length}"),
            });
        }

        let mut response_bytes = vec![0_u8; response_length];
        timeout(self.timeout, stream.read_exact(&mut response_bytes))
            .await
            .map_err(|_| BlenderError {
                code: "read_timeout".to_string(),
                message: "timed out reading Blender response".to_string(),
            })?
            .map_err(|error| BlenderError {
                code: "read_failed".to_string(),
                message: error.to_string(),
            })?;
        let response: Value =
            serde_json::from_slice(&response_bytes).map_err(|error| BlenderError {
                code: "invalid_response".to_string(),
                message: error.to_string(),
            })?;

        if response.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(response.get("data").cloned().unwrap_or(Value::Null))
        } else {
            let code = response
                .pointer("/error/code")
                .or_else(|| response.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("bridge_error")
                .to_string();
            let message = response
                .pointer("/error/message")
                .or_else(|| response.get("error").filter(|value| value.is_string()))
                .or_else(|| response.get("message"))
                .or_else(|| response.get("detail"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    format!(
                        "Blender bridge returned an unspecified error: {}",
                        truncate_response(&response)
                    )
                });
            Err(BlenderError { code, message })
        }
    }
}

pub fn persona_idle_profile(persona: &PersonaCard) -> Value {
    let description = format!(
        "{} {} {} {}",
        persona.personality,
        persona.background,
        persona.speech_style,
        persona.name,
    )
    .to_lowercase();
    let energetic = [
        "活泼", "开朗", "元气", "淘气", "俏皮", "energetic", "playful", "lively",
    ]
    .iter()
    .any(|keyword| description.contains(keyword));
    let restrained = [
        "安静", "温柔", "害羞", "冷静", "沉稳", "quiet", "gentle", "shy", "calm",
    ]
    .iter()
    .any(|keyword| description.contains(keyword));
    let (sway, breathe, head, arm_drop, frames) = if energetic {
        (2.8, 2.2, 1.8, 18.0, 84)
    } else if restrained {
        (1.2, 1.2, 0.8, 15.0, 120)
    } else {
        (1.8, 1.6, 1.2, 16.0, 104)
    };
    json!({
        "profile_id": format!("{}:{}", persona.name, if energetic { "energetic" } else if restrained { "restrained" } else { "balanced" }),
        "sway_degrees": sway,
        "breath_degrees": breathe,
        "head_degrees": head,
        "arm_drop_degrees": arm_drop,
        "duration_frames": frames,
    })
}

pub async fn check_health(config: &BlenderConfig) -> BlenderHealth {
    let bridge = BlenderBridge::from_config(config);
    let address = bridge.address();
    if !config.enabled || !bridge.is_active() {
        return BlenderHealth {
            enabled: config.enabled,
            reachable: false,
            profile: bridge.profile,
            render_mode: bridge.mode,
            address,
            detail: "Blender integration is disabled".to_string(),
        };
    }

    match bridge.ping().await {
        Ok(_) => BlenderHealth {
            enabled: true,
            reachable: true,
            profile: bridge.profile,
            render_mode: bridge.mode,
            address,
            detail: "Blender bridge responded to system.ping".to_string(),
        },
        Err(error) => BlenderHealth {
            enabled: true,
            reachable: false,
            profile: bridge.profile,
            render_mode: bridge.mode,
            address,
            detail: error.to_string(),
        },
    }
}

fn request_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{sequence}")
}

fn truncate_response(value: &Value) -> String {
    let serialized = value.to_string();
    if serialized.chars().count() <= 800 {
        serialized
    } else {
        let mut output = serialized.chars().take(800).collect::<String>();
        output.push('…');
        output
    }
}

fn unsupported_command(error: &BlenderError) -> bool {
    matches!(
        error.code.as_str(),
        "unknown_command"
            | "unsupported_command"
            | "command_not_found"
            | "invalid_command"
    )
}

fn virtual_c_expression_preset(value: &str) -> &str {
    match value {
        "happy" | "excited" => "joy",
        "shy" => "smile",
        "default" => "neutral",
        "wink" => "wink_left",
        other => other,
    }
}

fn virtual_c_expression_fallbacks(value: &str) -> Vec<&str> {
    match value {
        "joy" => vec!["joy", "smile", "wink_left", "wink_right", "neutral"],
        "smile" => vec!["smile", "joy", "wink_left", "wink_right", "neutral"],
        "blink" => vec!["blink", "wink_left", "wink_right", "neutral"],
        "wink_left" => vec!["wink_left", "wink_right", "neutral"],
        "wink_right" => vec!["wink_right", "wink_left", "neutral"],
        "surprised" => vec!["surprised", "mouth_open", "neutral"],
        "sad" => vec!["sad", "neutral"],
        "angry" => vec!["angry", "neutral"],
        _ => vec![value, "neutral"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bridge_sends_length_prefixed_authenticated_json() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut length = [0_u8; 4];
            socket.read_exact(&mut length).await.unwrap();
            let mut payload = vec![0_u8; u32::from_be_bytes(length) as usize];
            socket.read_exact(&mut payload).await.unwrap();
            let request: Value = serde_json::from_slice(&payload).unwrap();
            assert_eq!(request["version"], 1);
            assert_eq!(request["command"], "system.ping");
            assert_eq!(request["token"], "test-token");

            let response = serde_json::to_vec(&json!({
                "ok": true,
                "data": {"version": 1}
            }))
            .unwrap();
            socket
                .write_all(&(response.len() as u32).to_be_bytes())
                .await
                .unwrap();
            socket.write_all(&response).await.unwrap();
        });

        let config = BlenderConfig {
            enabled: true,
            host: address.ip().to_string(),
            port: address.port(),
            token: "test-token".to_string(),
            render_mode: "pose".to_string(),
            ..BlenderConfig::default()
        };
        let response = BlenderBridge::from_config(&config).ping().await.unwrap();
        assert_eq!(response["version"], 1);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn virtual_c_profile_discovers_model_and_uses_native_parameters() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let expected = [
                "mmd.list_models",
                "expression.inspect",
                "ravichara.expression.apply_timed",
                "mmd.list_models",
                "ravichara.animation.play_once",
            ];
            for command in expected {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut length = [0_u8; 4];
                socket.read_exact(&mut length).await.unwrap();
                let mut payload =
                    vec![0_u8; u32::from_be_bytes(length) as usize];
                socket.read_exact(&mut payload).await.unwrap();
                let request: Value = serde_json::from_slice(&payload).unwrap();
                assert_eq!(request["command"], command);
                if command == "ravichara.expression.apply_timed" {
                    assert_eq!(request["params"]["model"], "MMD Root");
                    assert_eq!(request["params"]["expression"], "joy");
                    assert_eq!(request["params"]["intensity"], 0.75);
                    assert_eq!(request["params"]["reset_expression"], "neutral");
                }
                if command == "ravichara.animation.play_once" {
                    assert_eq!(request["params"]["armature"], "MMD Armature");
                    assert_eq!(request["params"]["preset"], "nod");
                    assert_eq!(request["params"]["playback_start"], 1);
                    assert_eq!(request["params"]["action_start"], 9);
                    assert_eq!(request["params"]["action_end"], 41);
                    assert_eq!(request["params"]["playback_end"], 49);
                }

                let data = if command == "mmd.list_models" {
                    json!({
                        "total": 1,
                        "models": [{
                            "root": "MMD Root",
                            "name_j": "モデル",
                            "name_e": "Model",
                            "armatures": ["MMD Armature"]
                        }]
                    })
                } else if command == "expression.inspect" {
                    json!({
                        "preset_support": {
                            "joy": {"supported": true}
                        }
                    })
                } else {
                    json!({"accepted": true})
                };
                let response =
                    serde_json::to_vec(&json!({"ok": true, "data": data}))
                        .unwrap();
                socket
                    .write_all(&(response.len() as u32).to_be_bytes())
                    .await
                    .unwrap();
                socket.write_all(&response).await.unwrap();
            }
        });

        let config = BlenderConfig {
            enabled: true,
            profile: "virtual_c".to_string(),
            host: address.ip().to_string(),
            port: address.port(),
            render_mode: "pose".to_string(),
            ..BlenderConfig::default()
        };
        let bridge = BlenderBridge::from_config(&config);
        bridge.apply_expression("happy", 0.75).await.unwrap();
        bridge.play_motion("nod").await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn generated_behavior_uses_discovered_capabilities_and_bounded_protocol() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let expected = [
                "mmd.list_models",
                "ravichara.avatar.capabilities",
                "mmd.list_models",
                "ravichara.behavior.play_plan",
            ];
            for command in expected {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut length = [0_u8; 4];
                socket.read_exact(&mut length).await.unwrap();
                let mut payload =
                    vec![0_u8; u32::from_be_bytes(length) as usize];
                socket.read_exact(&mut payload).await.unwrap();
                let request: Value = serde_json::from_slice(&payload).unwrap();
                assert_eq!(request["command"], command);
                if command == "ravichara.behavior.play_plan" {
                    assert_eq!(request["params"]["model"], "MMD Root");
                    assert_eq!(request["params"]["armature"], "MMD Armature");
                    assert_eq!(request["params"]["keyframes"][0]["rotations"][0]["role"], "head");
                    assert_eq!(request["params"]["playback_start"], 1);
                    assert_eq!(request["params"]["action_start"], 9);
                }
                let data = match command {
                    "mmd.list_models" => json!({
                        "total": 1,
                        "models": [{
                            "root": "MMD Root",
                            "armatures": ["MMD Armature"]
                        }]
                    }),
                    "ravichara.avatar.capabilities" => json!({
                        "available": true,
                        "active_action": null,
                        "expression_presets": {
                            "neutral": {"supported": true}
                        },
                        "rig_profile": {"bone_count": 42},
                        "bone_roles": {
                            "head": {"bone": "Head", "max_degrees": [30, 45, 35]}
                        },
                        "bones": [],
                        "morphs": []
                    }),
                    _ => json!({"accepted": true}),
                };
                let response =
                    serde_json::to_vec(&json!({"ok": true, "data": data}))
                        .unwrap();
                socket
                    .write_all(&(response.len() as u32).to_be_bytes())
                    .await
                    .unwrap();
                socket.write_all(&response).await.unwrap();
            }
        });
        let config = BlenderConfig {
            enabled: true,
            profile: "virtual_c".to_string(),
            host: address.ip().to_string(),
            port: address.port(),
            render_mode: "pose".to_string(),
            ..BlenderConfig::default()
        };
        let bridge = BlenderBridge::from_config(&config);
        let capabilities = bridge.inspect_avatar_capabilities().await.unwrap();
        assert_eq!(capabilities["generated_behavior"]["available"], true);
        let plan = normalize_behavior_plan(
            &json!({
                "expression": "neutral",
                "fallback_motion": "head_tilt",
                "keyframes": [{
                    "at": 0.5,
                    "rotations": [{"role": "head", "degrees": [2, 3, 7]}]
                }],
                "morphs": []
            }),
            &capabilities,
        )
        .unwrap();
        bridge.play_generated_behavior(&plan).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn atomic_behavior_carries_generation_and_single_bundle() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for expected in ["mmd.list_models", "ravichara.behavior.execute"] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut length = [0_u8; 4];
                socket.read_exact(&mut length).await.unwrap();
                let mut payload = vec![0_u8; u32::from_be_bytes(length) as usize];
                socket.read_exact(&mut payload).await.unwrap();
                let request: Value = serde_json::from_slice(&payload).unwrap();
                assert_eq!(request["command"], expected);
                let data = if expected == "mmd.list_models" {
                    json!({
                        "models": [{
                            "root": "MMD Root",
                            "armatures": ["MMD Armature"]
                        }]
                    })
                } else {
                    assert_eq!(request["params"]["generation"], 42);
                    assert_eq!(request["params"]["behavior"]["model"], "MMD Root");
                    assert_eq!(request["params"]["behavior"]["armature"], "MMD Armature");
                    assert_eq!(request["params"]["behavior"]["expression"], "joy");
                    assert!(request["params"]["idle"]["profile_id"].is_string());
                    json!({
                        "ignored": false,
                        "idle": {"configured": true},
                        "expression": {"expression": "neutral"},
                        "behavior": {"action": null, "applied_morphs": []},
                        "fallback": null,
                        "errors": [],
                        "dispatched": true
                    })
                };
                let response = serde_json::to_vec(&json!({"ok": true, "data": data})).unwrap();
                socket
                    .write_all(&(response.len() as u32).to_be_bytes())
                    .await
                    .unwrap();
                socket.write_all(&response).await.unwrap();
            }
        });
        let config = BlenderConfig {
            enabled: true,
            profile: "virtual_c".to_string(),
            host: address.ip().to_string(),
            port: address.port(),
            render_mode: "pose".to_string(),
            ..BlenderConfig::default()
        };
        let execution = BlenderBridge::from_config(&config)
            .execute_behavior_with_generation(
                &PersonaCard::default(),
                &BehaviorPlan::fallback("happy", None),
                42,
            )
            .await;
        assert!(execution.idle_configured);
        assert!(execution.expression_dispatched);
        assert!(execution.errors.is_empty());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn preview_command_uses_bounded_render_parameters() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut length = [0_u8; 4];
            socket.read_exact(&mut length).await.unwrap();
            let mut payload = vec![0_u8; u32::from_be_bytes(length) as usize];
            socket.read_exact(&mut payload).await.unwrap();
            let request: Value = serde_json::from_slice(&payload).unwrap();
            assert_eq!(request["command"], "render.preview");
            assert_eq!(request["params"]["width"], 1024);
            assert_eq!(request["params"]["height"], 128);
            assert_eq!(request["params"]["refresh"], true);

            let response = serde_json::to_vec(&json!({
                "ok": true,
                "data": {
                    "mime_type": "image/png",
                    "image_base64": "iVBORw0KGgo=",
                    "width": 1024,
                    "height": 128
                }
            }))
            .unwrap();
            socket
                .write_all(&(response.len() as u32).to_be_bytes())
                .await
                .unwrap();
            socket.write_all(&response).await.unwrap();
        });
        let config = BlenderConfig {
            enabled: true,
            host: address.ip().to_string(),
            port: address.port(),
            render_mode: "pose".to_string(),
            ..BlenderConfig::default()
        };
        let response = BlenderBridge::from_config(&config)
            .render_preview(4000, 1, true, false)
            .await
            .unwrap();
        assert_eq!(response["width"], 1024);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn viewport_capture_falls_back_for_v01_preview_addon() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for expected in ["render.viewport", "render.preview"] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut length = [0_u8; 4];
                socket.read_exact(&mut length).await.unwrap();
                let mut payload = vec![0_u8; u32::from_be_bytes(length) as usize];
                socket.read_exact(&mut payload).await.unwrap();
                let request: Value = serde_json::from_slice(&payload).unwrap();
                assert_eq!(request["command"], expected);
                let response = if expected == "render.viewport" {
                    json!({
                        "ok": false,
                        "error": {
                            "code": "unknown_command",
                            "message": "not installed"
                        }
                    })
                } else {
                    json!({
                        "ok": true,
                        "data": {
                            "mime_type": "image/png",
                            "image_base64": "iVBORw0KGgo=",
                            "width": 256,
                            "height": 256
                        }
                    })
                };
                let encoded = serde_json::to_vec(&response).unwrap();
                socket
                    .write_all(&(encoded.len() as u32).to_be_bytes())
                    .await
                    .unwrap();
                socket.write_all(&encoded).await.unwrap();
            }
        });
        let config = BlenderConfig {
            enabled: true,
            host: address.ip().to_string(),
            port: address.port(),
            render_mode: "pose".to_string(),
            ..BlenderConfig::default()
        };
        let frame = BlenderBridge::from_config(&config)
            .render_viewport(256, 256, false)
            .await
            .unwrap();
        assert_eq!(frame["capture_mode"], "render-fallback");
        server.await.unwrap();
    }

    #[test]
    fn unknown_render_mode_safely_becomes_off() {
        assert_eq!(RenderMode::from_str("invalid"), RenderMode::Off);
    }

    #[test]
    fn virtual_c_expression_aliases_match_plugin_presets() {
        assert_eq!(virtual_c_expression_preset("happy"), "joy");
        assert_eq!(virtual_c_expression_preset("shy"), "smile");
        assert_eq!(virtual_c_expression_preset("sad"), "sad");
        assert_eq!(
            virtual_c_expression_fallbacks("joy"),
            vec!["joy", "smile", "wink_left", "wink_right", "neutral"]
        );
    }
}
