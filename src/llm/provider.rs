use crate::config::ResolvedLlmConfig;
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmError {
    pub code: &'static str,
    pub message: String,
    pub status: Option<StatusCode>,
}

impl LlmError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            status: None,
        }
    }

    fn with_status(
        code: &'static str,
        status: StatusCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            status: Some(status),
        }
    }
}

impl fmt::Display for LlmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for LlmError {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub attempts: u32,
}

impl LlmUsage {
    fn add(&mut self, other: &Self) {
        self.prompt_tokens = add_optional(self.prompt_tokens, other.prompt_tokens);
        self.completion_tokens =
            add_optional(self.completion_tokens, other.completion_tokens);
        self.total_tokens = add_optional(self.total_tokens, other.total_tokens);
        self.reasoning_tokens =
            add_optional(self.reasoning_tokens, other.reasoning_tokens);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmGeneration {
    pub content: String,
    pub finish_reason: String,
    pub usage: LlmUsage,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    async fn generate(
        &self,
        messages: &[ChatMessage],
    ) -> Result<LlmGeneration, LlmError>;
    async fn generate_stream(
        &self,
        messages: &[ChatMessage],
        on_delta: &(dyn Fn(String) -> bool + Send + Sync),
    ) -> Result<LlmGeneration, LlmError> {
        let generation = self.generate(messages).await?;
        if !on_delta(generation.content.clone()) {
            return Err(LlmError::new(
                "stream_cancelled",
                "the response consumer disconnected",
            ));
        }
        Ok(generation)
    }
    async fn health_check(&self) -> Result<(), LlmError>;
}

pub struct MockProvider {
    model: String,
}

impl MockProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
        }
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(
        &self,
        messages: &[ChatMessage],
    ) -> Result<LlmGeneration, LlmError> {
        let prompt = messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content.as_str())
            .unwrap_or_default();
        if prompt.contains("记得") || prompt.to_lowercase().contains("remember") {
            Ok(LlmGeneration {
                content:
                    "我会根据已经保存的记忆认真回答；目前这是离线演示模型。"
                        .to_string(),
                finish_reason: "stop".to_string(),
                usage: LlmUsage {
                    attempts: 1,
                    ..LlmUsage::default()
                },
            })
        } else {
            Ok(LlmGeneration {
                content: format!(
                    "我收到了你的消息：“{}”。目前正在使用离线演示模型。",
                    prompt
                ),
                finish_reason: "stop".to_string(),
                usage: LlmUsage {
                    attempts: 1,
                    ..LlmUsage::default()
                },
            })
        }
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        Ok(())
    }
}

pub struct OpenAICompatibleProvider {
    client: reqwest::Client,
    provider_name: String,
    base_url: String,
    api_key: String,
    model: String,
    temperature: f32,
    max_tokens: u32,
    thinking_mode: String,
    retry_empty_reasoning: bool,
    mcp_enabled: bool,
    mcp_server_url: String,
    mcp_integration_id: String,
}

impl OpenAICompatibleProvider {
    pub fn new(config: &ResolvedLlmConfig) -> Result<Self, LlmError> {
        if config.base_url.trim().is_empty() {
            return Err(LlmError::new(
                "invalid_config",
                "LLM base_url is empty",
            ));
        }
        if config.model.trim().is_empty() {
            return Err(LlmError::new("invalid_config", "LLM model is empty"));
        }

        let client = crate::http_client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .build()
            .map_err(|error| LlmError::new("client_init_failed", error.to_string()))?;

        Ok(Self {
            client,
            provider_name: config.provider.clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            thinking_mode: config.thinking_mode.clone(),
            retry_empty_reasoning: config.retry_empty_reasoning,
            mcp_enabled: config.mcp_enabled,
            mcp_server_url: config.mcp_server_url.clone(),
            mcp_integration_id: config.mcp_integration_id.clone(),
        })
    }

    fn request(&self, url: &str) -> reqwest::RequestBuilder {
        let request = self.client.get(url);
        if self.api_key.is_empty() {
            request
        } else {
            request.bearer_auth(&self.api_key)
        }
    }

    fn post_request(&self, url: &str) -> reqwest::RequestBuilder {
        let request = self.client.post(url);
        if self.api_key.is_empty() {
            request
        } else {
            request.bearer_auth(&self.api_key)
        }
    }

    async fn request_completion(
        &self,
        messages: &[ChatMessage],
        max_tokens: u32,
    ) -> Result<Value, LlmError> {
        if self.provider_name == "lmstudio" {
            return self
                .request_lmstudio_native(messages, max_tokens)
                .await;
        }
        let url = format!("{}/chat/completions", self.base_url);
        let payload = self.completion_payload(messages, max_tokens, false);
        let response = self
            .post_request(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                LlmError::new(
                    "connection_failed",
                    format!("failed to reach {}: {error}", self.base_url),
                )
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| LlmError::new("invalid_response", error.to_string()))?;
        if !status.is_success() {
            return Err(LlmError::with_status(
                "provider_error",
                status,
                truncate_error_body(&body),
            ));
        }

        serde_json::from_str(&body).map_err(|error| {
            LlmError::new(
                "invalid_response",
                format!("provider returned invalid JSON: {error}"),
            )
        })
    }

    fn completion_payload(
        &self,
        messages: &[ChatMessage],
        max_tokens: u32,
        stream: bool,
    ) -> Value {
        let mut payload = json!({
            "model": self.model,
            "stream": stream,
            "temperature": self.temperature,
            "max_tokens": max_tokens,
            "messages": messages,
        });
        if matches!(self.provider_name.as_str(), "lmstudio" | "ollama" | "custom")
            && self.thinking_mode != "auto"
        {
            payload["chat_template_kwargs"] = json!({
                "enable_thinking": self.thinking_mode == "enabled"
            });
        }
        if self.provider_name == "deepseek" {
            if self.thinking_mode != "auto" {
                payload["thinking"] = json!({
                    "type": if self.thinking_mode == "enabled" {
                        "enabled"
                    } else {
                        "disabled"
                    }
                });
            }
            // DeepSeek V4 defaults to thinking in auto mode. Sampling controls
            // are unsupported while thinking, so omit temperature unless
            // non-thinking was explicitly selected.
            if self.thinking_mode != "disabled" {
                if let Some(object) = payload.as_object_mut() {
                    object.remove("temperature");
                }
            }
            if self.thinking_mode == "enabled" {
                payload["reasoning_effort"] = json!("high");
            }
        }
        payload
    }

    async fn request_streaming_completion(
        &self,
        messages: &[ChatMessage],
        max_tokens: u32,
        on_delta: &(dyn Fn(String) -> bool + Send + Sync),
    ) -> Result<LlmGeneration, LlmError> {
        // The OpenAI-compatible endpoint is deliberately used for streaming,
        // including LM Studio. Native MCP/tool execution is handled outside the
        // text critical path so that avatar work can never delay first-token UI.
        let url = format!("{}/chat/completions", self.base_url);
        let payload = self.completion_payload(messages, max_tokens, true);
        let mut response = self
            .post_request(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                LlmError::new(
                    "connection_failed",
                    format!("failed to reach {}: {error}", self.base_url),
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "the streaming endpoint returned an error".to_string());
            return Err(LlmError::with_status(
                "provider_error",
                status,
                truncate_error_body(&body),
            ));
        }

        let mut pending = Vec::<u8>::new();
        let mut content = String::new();
        let mut finish_reason = "stop".to_string();
        let mut usage = LlmUsage {
            attempts: 1,
            ..LlmUsage::default()
        };
        let mut done = false;
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            LlmError::new(
                "invalid_response",
                format!("failed while reading the provider stream: {error}"),
            )
        })? {
            pending.extend_from_slice(&chunk);
            while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
                let mut line = pending.drain(..=newline).collect::<Vec<_>>();
                while matches!(line.last(), Some(b'\n' | b'\r')) {
                    line.pop();
                }
                if process_sse_line(
                    &line,
                    &mut content,
                    &mut finish_reason,
                    &mut usage,
                    on_delta,
                )? {
                    done = true;
                    break;
                }
            }
            if done {
                break;
            }
        }
        if !done && !pending.is_empty() {
            process_sse_line(
                &pending,
                &mut content,
                &mut finish_reason,
                &mut usage,
                on_delta,
            )?;
        }
        if content.trim().is_empty() {
            return Err(LlmError::new(
                "empty_completion",
                "the provider stream ended without assistant content",
            ));
        }
        Ok(LlmGeneration {
            content,
            finish_reason,
            usage,
        })
    }

    async fn request_lmstudio_native(
        &self,
        messages: &[ChatMessage],
        max_tokens: u32,
    ) -> Result<Value, LlmError> {
        let root = self
            .base_url
            .strip_suffix("/v1")
            .unwrap_or(&self.base_url);
        let url = format!("{root}/api/v1/chat");
        let system_prompt = messages
            .iter()
            .filter(|message| message.role == "system")
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let input = messages
            .iter()
            .filter(|message| message.role != "system")
            .map(|message| {
                let role = match message.role.as_str() {
                    "assistant" => "Assistant",
                    _ => "User",
                };
                format!("{role}: {}", message.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let mut payload = json!({
            "model": self.model,
            "input": input,
            "system_prompt": system_prompt,
            "stream": false,
            "temperature": self.temperature.clamp(0.0, 1.0),
            "max_output_tokens": max_tokens,
            "store": false,
        });
        let allowed_avatar_tools = allowed_avatar_tools(messages);
        if self.mcp_enabled && !self.mcp_integration_id.is_empty() {
            payload["integrations"] = json!([{
                "type": "plugin",
                "id": self.mcp_integration_id,
                "allowed_tools": allowed_avatar_tools
            }]);
        } else if self.mcp_enabled && self.mcp_server_url.starts_with("https://") {
            payload["integrations"] = json!([{
                "type": "ephemeral_mcp",
                "server_label": "ravichara",
                "server_url": self.mcp_server_url,
                "allowed_tools": allowed_avatar_tools
            }]);
        }
        let qwen35_without_reasoning_control =
            self.model.to_lowercase().contains("qwen3.5");
        if !qwen35_without_reasoning_control {
            match self.thinking_mode.as_str() {
                "disabled" => payload["reasoning"] = json!("off"),
                "enabled" => payload["reasoning"] = json!("on"),
                _ => {}
            }
        }
        let response = self
            .post_request(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                LlmError::new(
                    "connection_failed",
                    format!("failed to reach LM Studio native API at {url}: {error}"),
                )
            })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| LlmError::new("invalid_response", error.to_string()))?;
        if !status.is_success() {
            return Err(LlmError::with_status(
                "provider_error",
                status,
                truncate_error_body(&body),
            ));
        }
        let native: Value = serde_json::from_str(&body).map_err(|error| {
            LlmError::new(
                "invalid_response",
                format!("LM Studio returned invalid JSON: {error}"),
            )
        })?;
        let content = native
            .get("output")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
            .filter_map(|item| item.get("content").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        let reasoning_present = native
            .get("output")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|item| {
                item.get("type").and_then(Value::as_str) == Some("reasoning")
                    && item
                        .get("content")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty())
            });
        let input_tokens = native.pointer("/stats/input_tokens").cloned();
        let output_tokens = native.pointer("/stats/total_output_tokens").cloned();
        let reasoning_tokens =
            native.pointer("/stats/reasoning_output_tokens").cloned();
        let total_tokens = match (
            input_tokens.as_ref().and_then(Value::as_u64),
            output_tokens.as_ref().and_then(Value::as_u64),
        ) {
            (Some(input), Some(output)) => json!(input.saturating_add(output)),
            _ => Value::Null,
        };
        Ok(json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": content,
                    "reasoning_content": if reasoning_present { "present" } else { "" },
                },
                "finish_reason": if content.is_empty() && reasoning_present {
                    "length"
                } else {
                    "stop"
                }
            }],
            "usage": {
                "prompt_tokens": input_tokens.unwrap_or(Value::Null),
                "completion_tokens": output_tokens.unwrap_or(Value::Null),
                "total_tokens": total_tokens,
                "completion_tokens_details": {
                    "reasoning_tokens": reasoning_tokens.unwrap_or(Value::Null),
                }
            }
        }))
    }
}

#[async_trait]
impl LlmProvider for OpenAICompatibleProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(
        &self,
        messages: &[ChatMessage],
    ) -> Result<LlmGeneration, LlmError> {
        let mut token_limit = self.max_tokens;
        let mut value = self.request_completion(messages, token_limit).await?;
        let mut content = completion_content(&value);
        let mut usage = completion_usage(&value);
        usage.attempts = 1;

        if content.is_empty() {
            let (finish_reason, reasoning_present) = completion_state(&value);
            if self.retry_empty_reasoning
                && reasoning_present
                && finish_reason == "length"
                && token_limit < 4096
            {
                token_limit = token_limit.saturating_mul(2).min(4096);
                tracing::warn!(
                    "reasoning model produced no final content; retrying with max_tokens={token_limit}"
                );
                value = self.request_completion(messages, token_limit).await?;
                content = completion_content(&value);
                usage.add(&completion_usage(&value));
                usage.attempts = 2;
            }
        }

        if content.is_empty() {
            let (finish_reason, reasoning_present) = completion_state(&value);
            let hint = if reasoning_present
                && finish_reason == "length"
                && self.retry_empty_reasoning
            {
                "the reasoning model exhausted the configured retry token budget before producing final content"
            } else if reasoning_present && finish_reason == "length" {
                "the reasoning model consumed max_tokens before producing final content; automatic retry is disabled"
            } else {
                "the provider returned no final assistant content"
            };
            return Err(LlmError::new("empty_completion", hint));
        }

        Ok(LlmGeneration {
            content,
            finish_reason: completion_state(&value).0.to_string(),
            usage,
        })
    }

    async fn generate_stream(
        &self,
        messages: &[ChatMessage],
        on_delta: &(dyn Fn(String) -> bool + Send + Sync),
    ) -> Result<LlmGeneration, LlmError> {
        self.request_streaming_completion(messages, self.max_tokens, on_delta)
            .await
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        let url = format!("{}/models", self.base_url);
        let response = self.request(&url).send().await.map_err(|error| {
            LlmError::new(
                "connection_failed",
                format!("failed to reach {}: {error}", self.base_url),
            )
        })?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "the models endpoint returned an error".to_string());
            Err(LlmError::with_status(
                "provider_error",
                status,
                truncate_error_body(&body),
            ))
        }
    }
}

fn process_sse_line(
    line: &[u8],
    content: &mut String,
    finish_reason: &mut String,
    usage: &mut LlmUsage,
    on_delta: &(dyn Fn(String) -> bool + Send + Sync),
) -> Result<bool, LlmError> {
    let line = std::str::from_utf8(line).map_err(|error| {
        LlmError::new(
            "invalid_response",
            format!("provider stream contained invalid UTF-8: {error}"),
        )
    })?;
    let Some(data) = line.trim().strip_prefix("data:") else {
        return Ok(false);
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Ok(true);
    }
    if data.is_empty() {
        return Ok(false);
    }
    let value: Value = serde_json::from_str(data).map_err(|error| {
        LlmError::new(
            "invalid_response",
            format!("provider returned an invalid SSE event: {error}"),
        )
    })?;
    if let Some(reason) = value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        *finish_reason = reason.to_string();
    }
    let event_usage = completion_usage(&value);
    if event_usage.prompt_tokens.is_some()
        || event_usage.completion_tokens.is_some()
        || event_usage.total_tokens.is_some()
        || event_usage.reasoning_tokens.is_some()
    {
        let attempts = usage.attempts;
        *usage = event_usage;
        usage.attempts = attempts;
    }
    let delta = stream_delta_content(&value);
    if !delta.is_empty() {
        if !on_delta(delta.clone()) {
            return Err(LlmError::new(
                "stream_cancelled",
                "the response consumer disconnected",
            ));
        }
        content.push_str(&delta);
    }
    Ok(false)
}

fn stream_delta_content(value: &Value) -> String {
    let Some(content) = value.pointer("/choices/0/delta/content") else {
        return String::new();
    };
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("text")
                .and_then(Value::as_str)
                .or_else(|| item.get("content").and_then(Value::as_str))
        })
        .collect::<String>()
}

pub fn create_provider(
    config: &ResolvedLlmConfig,
) -> Result<Box<dyn LlmProvider>, LlmError> {
    if config.use_mock {
        Ok(Box::new(MockProvider::new(&config.model)))
    } else {
        Ok(Box::new(OpenAICompatibleProvider::new(config)?))
    }
}

pub(crate) fn allowed_avatar_tools(
    messages: &[ChatMessage],
) -> Vec<&'static str> {
    const TRANSIENT: &[&str] = &[
        "ravichara_get_avatar_capabilities",
        "ravichara_apply_behavior_plan",
        "ravichara_apply_reaction",
    ];
    const SCENE_AWARE: &[&str] = &[
        "ravichara_apply_behavior_plan",
        "ravichara_apply_reaction",
        "ravichara_get_render_status",
        "ravichara_get_avatar_capabilities",
        "ravichara_inspect_rig",
        "ravichara_inspect_expressions",
        "ravichara_list_scene_objects",
        "ravichara_list_materials",
        "ravichara_inspect_shader",
        "ravichara_propose_rig_pose",
        "ravichara_propose_motion_action",
        "ravichara_propose_morph_weights",
        "ravichara_propose_expression_sequence",
        "ravichara_propose_shader_template",
        "ravichara_propose_shader_graph",
        "ravichara_propose_material_adjust",
        "ravichara_propose_clear_animation",
    ];
    let latest_user = messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.to_lowercase())
        .unwrap_or_default();
    let scene_intent = [
        "blender",
        "shader",
        "material",
        "材质",
        "着色器",
        "节点",
        "骨骼",
        "姿态",
        "关键帧",
        "动画",
        "动作序列",
        "表情序列",
        "模型",
        "场景",
        "画面",
        "渲染",
        "相机",
        "镜头",
        "动作",
        "伸手",
        "抬手",
        "挥手",
        "转头",
        "低头",
        "抬头",
        "看向",
        "闭眼",
        "眨眼",
        "连续表情",
        "先笑",
        "衣服",
        "皮肤",
        "布料",
        "金属",
        "质感",
        "mesh",
        "armature",
        "bone",
        "rig",
        "pose",
        "action",
        "keyframe",
        "preview",
        "render",
        "camera",
    ]
    .iter()
    .any(|keyword| latest_user.contains(keyword));
    if latest_user.contains("全套 blender")
        || latest_user.contains("all blender controls")
    {
        return SCENE_AWARE.to_vec();
    }

    let has_any = |keywords: &[&str]| {
        keywords
            .iter()
            .any(|keyword| latest_user.contains(keyword))
    };
    let inspection_intent = has_any(&[
        "blender", "模型", "场景", "画面", "渲染", "相机", "镜头",
        "preview", "render", "camera",
    ]);
    let rig_intent = has_any(&[
        "骨骼", "姿态", "关键帧", "动画", "动作", "动作序列",
        "伸手", "抬手", "挥手", "转头", "低头", "抬头", "看向",
        "armature", "bone", "rig", "pose", "action", "keyframe",
    ]);
    let expression_sequence_intent = has_any(&[
        "表情序列", "连续表情", "先笑", "expression sequence",
    ]);
    let morph_intent = has_any(&[
        "表情权重", "形态键", "精确表情", "morph", "shape key",
    ]);
    let shader_intent = has_any(&[
        "shader", "material", "材质", "着色器", "节点", "衣服",
        "皮肤", "布料", "金属", "质感", "mesh",
    ]);

    // Keep explicit UTF-8 terms beside the legacy keyword set. This is
    // important for Chinese API prompts and avoids relying on provider-side
    // translation before tool scoping is decided.
    let utf8_inspection_intent = has_any(&[
        "模型", "场景", "画面", "渲染", "相机", "镜头",
    ]);
    let utf8_rig_intent = has_any(&[
        "骨骼", "姿势", "关键帧", "动画", "动作", "动作序列", "伸手", "抬手",
        "挥手", "转头", "低头", "抬头", "看向", "鞠躬", "踢腿", "歪头",
        "摇头", "耸肩",
    ]);
    let utf8_expression_sequence_intent =
        has_any(&["表情序列", "连续表情", "先笑"]);
    let utf8_morph_intent = has_any(&["表情权重", "形态键", "精确表情"]);
    let utf8_material_target = has_any(&[
        "头发", "皮肤", "衣服", "服装", "眼睛", "布料", "金属",
    ]);
    let utf8_material_adjustment = utf8_material_target
        && has_any(&["变亮", "变暗", "亮一点", "暗一点", "调亮", "调暗"]);
    let utf8_shader_intent = has_any(&["材质", "着色器", "节点", "质感"])
        || utf8_material_adjustment;

    let mut tools = TRANSIENT.to_vec();
    let mut add = |tool| {
        if !tools.contains(&tool) {
            tools.push(tool);
        }
    };
    if scene_intent
        || inspection_intent
        || utf8_inspection_intent
        || utf8_rig_intent
        || utf8_shader_intent
    {
        add("ravichara_get_render_status");
        add("ravichara_get_avatar_capabilities");
    }
    if rig_intent || utf8_rig_intent {
        add("ravichara_inspect_rig");
        add("ravichara_propose_rig_pose");
        add("ravichara_propose_motion_action");
        add("ravichara_propose_clear_animation");
    }
    if rig_intent
        || utf8_rig_intent
        || expression_sequence_intent
        || utf8_expression_sequence_intent
    {
        add("ravichara_propose_expression_sequence");
    }
    if morph_intent || utf8_morph_intent {
        add("ravichara_inspect_expressions");
        add("ravichara_propose_morph_weights");
    }
    if shader_intent || utf8_shader_intent {
        add("ravichara_list_scene_objects");
        add("ravichara_list_materials");
        add("ravichara_inspect_shader");
        add("ravichara_propose_shader_template");
        add("ravichara_propose_shader_graph");
        add("ravichara_propose_material_adjust");
    }
    tools
}

fn truncate_error_body(body: &str) -> String {
    const LIMIT: usize = 1200;
    if body.chars().count() <= LIMIT {
        body.to_string()
    } else {
        let mut truncated = body.chars().take(LIMIT).collect::<String>();
        truncated.push('…');
        truncated
    }
}

fn completion_content(value: &Value) -> String {
    value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn completion_state(value: &Value) -> (&str, bool) {
    let finish_reason = value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let reasoning_present = value
        .pointer("/choices/0/message/reasoning_content")
        .and_then(Value::as_str)
        .map(|reasoning| !reasoning.trim().is_empty())
        .unwrap_or(false);
    (finish_reason, reasoning_present)
}

fn completion_usage(value: &Value) -> LlmUsage {
    let integer = |pointer: &str| value.pointer(pointer).and_then(Value::as_u64);
    LlmUsage {
        prompt_tokens: integer("/usage/prompt_tokens"),
        completion_tokens: integer("/usage/completion_tokens"),
        total_tokens: integer("/usage/total_tokens"),
        reasoning_tokens: integer(
            "/usage/completion_tokens_details/reasoning_tokens",
        ),
        attempts: 0,
    }
}

fn add_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (None, None) => None,
        (left, right) => Some(left.unwrap_or(0).saturating_add(right.unwrap_or(0))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::Json, extract::State, routing::get, routing::post, Router};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    async fn start_fake_server(content: &'static str) -> String {
        async fn models() -> Json<Value> {
            Json(json!({"object": "list", "data": []}))
        }
        async fn completion(
            Json(payload): Json<Value>,
        ) -> Json<Value> {
            let content = payload
                .get("_test_content")
                .and_then(Value::as_str)
                .unwrap_or("FAKE_OK");
            Json(json!({
                "choices": [{
                    "message": {"role": "assistant", "content": content},
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 12,
                    "completion_tokens": 4,
                    "total_tokens": 16
                }
            }))
        }

        let app = Router::new()
            .route("/v1/models", get(models))
            .route("/v1/chat/completions", post(completion));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let _ = content;
        format!("http://{address}/v1")
    }

    #[tokio::test]
    async fn openai_compatible_provider_generates_and_checks_health() {
        let base_url = start_fake_server("FAKE_OK").await;
        let config = ResolvedLlmConfig {
            provider: "test".to_string(),
            base_url,
            model: "test-model".to_string(),
            api_key: String::new(),
            temperature: 0.2,
            max_tokens: 64,
            request_timeout_seconds: 5,
            use_mock: false,
            thinking_mode: "disabled".to_string(),
            retry_empty_reasoning: false,
            mcp_enabled: false,
            mcp_server_url: String::new(),
            mcp_integration_id: String::new(),
        };
        let provider = OpenAICompatibleProvider::new(&config).unwrap();
        provider.health_check().await.unwrap();
        let reply = provider
            .generate(&[ChatMessage::user("hello")])
            .await
            .unwrap();
        assert_eq!(reply.content, "FAKE_OK");
        assert_eq!(reply.usage.total_tokens, Some(16));
        assert_eq!(reply.usage.attempts, 1);
    }

    #[test]
    fn deepseek_v4_uses_official_thinking_toggle_without_local_template_fields() {
        let config = ResolvedLlmConfig {
            provider: "deepseek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-v4-flash".to_string(),
            api_key: "test-only".to_string(),
            temperature: 0.2,
            max_tokens: 64,
            request_timeout_seconds: 5,
            use_mock: false,
            thinking_mode: "disabled".to_string(),
            retry_empty_reasoning: false,
            mcp_enabled: false,
            mcp_server_url: String::new(),
            mcp_integration_id: String::new(),
        };
        let disabled = OpenAICompatibleProvider::new(&config).unwrap();
        let payload = disabled.completion_payload(
            &[ChatMessage::user("hello")],
            64,
            true,
        );
        assert_eq!(payload["thinking"]["type"], "disabled");
        assert!(
            (payload["temperature"].as_f64().unwrap_or_default() - 0.2).abs()
                < 1e-6
        );
        assert!(payload.get("chat_template_kwargs").is_none());
        assert!(payload.get("reasoning_effort").is_none());

        let mut enabled_config = config.clone();
        enabled_config.thinking_mode = "enabled".to_string();
        let enabled = OpenAICompatibleProvider::new(&enabled_config).unwrap();
        let payload = enabled.completion_payload(
            &[ChatMessage::user("hello")],
            64,
            false,
        );
        assert_eq!(payload["thinking"]["type"], "enabled");
        assert_eq!(payload["reasoning_effort"], "high");
        assert!(payload.get("temperature").is_none());

        let mut auto_config = config;
        auto_config.thinking_mode = "auto".to_string();
        let auto = OpenAICompatibleProvider::new(&auto_config).unwrap();
        let payload = auto.completion_payload(
            &[ChatMessage::user("hello")],
            64,
            false,
        );
        assert!(payload.get("thinking").is_none());
        assert!(payload.get("reasoning_effort").is_none());
        assert!(payload.get("temperature").is_none());
    }

    #[tokio::test]
    #[ignore = "requires RAVICHARA_TEST_DEEPSEEK_API_KEY and consumes a live API request"]
    async fn live_deepseek_v4_stream_smoke() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

        let api_key = std::env::var("RAVICHARA_TEST_DEEPSEEK_API_KEY")
            .expect("RAVICHARA_TEST_DEEPSEEK_API_KEY is required");
        let model = std::env::var("RAVICHARA_TEST_DEEPSEEK_MODEL")
            .unwrap_or_else(|_| "deepseek-v4-flash".to_string());
        let config = ResolvedLlmConfig {
            provider: "deepseek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            model,
            api_key,
            temperature: 0.2,
            max_tokens: 64,
            request_timeout_seconds: 30,
            use_mock: false,
            thinking_mode: "disabled".to_string(),
            retry_empty_reasoning: false,
            mcp_enabled: false,
            mcp_server_url: String::new(),
            mcp_integration_id: String::new(),
        };
        let provider = OpenAICompatibleProvider::new(&config).unwrap();
        provider.health_check().await.unwrap();
        let started = std::time::Instant::now();
        let first_delta_ms = AtomicU64::new(u64::MAX);
        let delta_chars = AtomicUsize::new(0);
        let on_delta = |delta: String| {
            let elapsed = started.elapsed().as_millis() as u64;
            let _ = first_delta_ms.compare_exchange(
                u64::MAX,
                elapsed,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            delta_chars.fetch_add(delta.chars().count(), Ordering::SeqCst);
            true
        };
        let reply = provider
            .generate_stream(
                &[ChatMessage::user("只回复 RAVICHARA_OK")],
                &on_delta,
            )
            .await
            .unwrap();
        let first_ms = first_delta_ms.load(Ordering::SeqCst);
        assert_ne!(first_ms, u64::MAX);
        assert!(!reply.content.trim().is_empty());
        assert!(delta_chars.load(Ordering::SeqCst) > 0);
        eprintln!(
            "DEEPSEEK_STREAM_SMOKE first_delta_ms={} total_ms={} chars={}",
            first_ms,
            started.elapsed().as_millis(),
            reply.content.chars().count(),
        );
    }

    #[tokio::test]
    async fn mock_provider_is_explicit_and_does_not_claim_real_recall() {
        let provider = MockProvider::new("mock");
        let reply = provider
            .generate(&[ChatMessage::user("你还记得吗？")])
            .await
            .unwrap();
        assert!(reply.content.contains("离线演示模型"));
    }

    #[tokio::test]
    async fn lmstudio_native_api_disables_reasoning_and_maps_stats() {
        async fn models() -> Json<Value> {
            Json(json!({"object": "list", "data": []}))
        }
        async fn chat(Json(payload): Json<Value>) -> Json<Value> {
            assert_eq!(payload["reasoning"], "off");
            assert_eq!(payload["store"], false);
            assert_eq!(payload["integrations"][0]["type"], "plugin");
            assert_eq!(payload["integrations"][0]["id"], "mcp/ravichara");
            let allowed = payload["integrations"][0]["allowed_tools"]
                .as_array()
                .unwrap();
            assert!(allowed.iter().any(|tool| {
                tool.as_str() == Some("ravichara_get_avatar_capabilities")
            }));
            assert!(allowed.iter().any(|tool| {
                tool.as_str() == Some("ravichara_apply_behavior_plan")
            }));
            assert!(payload["system_prompt"]
                .as_str()
                .unwrap()
                .contains("system rule"));
            Json(json!({
                "model_instance_id": "test-model",
                "output": [
                    {"type": "message", "content": "NATIVE_OK"}
                ],
                "stats": {
                    "input_tokens": 18,
                    "total_output_tokens": 3,
                    "reasoning_output_tokens": 0
                }
            }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/v1/models", get(models))
            .route("/api/v1/chat", post(chat));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let config = ResolvedLlmConfig {
            provider: "lmstudio".to_string(),
            base_url: format!("http://{address}/v1"),
            model: "test-model".to_string(),
            api_key: String::new(),
            temperature: 0.2,
            max_tokens: 64,
            request_timeout_seconds: 5,
            use_mock: false,
            thinking_mode: "disabled".to_string(),
            retry_empty_reasoning: false,
            mcp_enabled: true,
            mcp_server_url: "http://127.0.0.1:8760/mcp".to_string(),
            mcp_integration_id: "mcp/ravichara".to_string(),
        };
        let provider = OpenAICompatibleProvider::new(&config).unwrap();
        provider.health_check().await.unwrap();
        let result = provider
            .generate(&[
                ChatMessage::system("system rule"),
                ChatMessage::user("hello"),
            ])
            .await
            .unwrap();
        assert_eq!(result.content, "NATIVE_OK");
        assert_eq!(result.usage.prompt_tokens, Some(18));
        assert_eq!(result.usage.reasoning_tokens, Some(0));
        assert_eq!(result.usage.total_tokens, Some(21));
        server.abort();
    }

    #[test]
    fn avatar_tools_expand_only_for_scene_aware_requests() {
        let casual = allowed_avatar_tools(&[ChatMessage::user("hello")]);
        assert_eq!(
            casual,
            vec![
                "ravichara_get_avatar_capabilities",
                "ravichara_apply_behavior_plan",
                "ravichara_apply_reaction",
            ]
        );

        let scene =
            allowed_avatar_tools(&[ChatMessage::user("inspect the Blender shader")]);
        assert!(scene.contains(&"ravichara_inspect_shader"));
        assert!(scene.contains(&"ravichara_propose_shader_graph"));

        let expression_only =
            allowed_avatar_tools(&[ChatMessage::user("闭上眼睛")]);
        assert!(expression_only.contains(&"ravichara_apply_behavior_plan"));
        assert!(!expression_only.contains(&"ravichara_propose_shader_graph"));

        let pose =
            allowed_avatar_tools(&[ChatMessage::user("向前伸手")]);
        assert!(pose.contains(&"ravichara_inspect_rig"));
        assert!(pose.contains(&"ravichara_propose_rig_pose"));
        assert!(!pose.contains(&"ravichara_propose_shader_graph"));

        let morph =
            allowed_avatar_tools(&[ChatMessage::user("设置精确表情权重")]);
        assert!(morph.contains(&"ravichara_inspect_expressions"));
        assert!(morph.contains(&"ravichara_propose_morph_weights"));
    }

    #[tokio::test]
    async fn empty_reasoning_is_not_silently_retried_by_default() {
        async fn completion(
            State(counter): State<Arc<AtomicUsize>>,
            Json(payload): Json<Value>,
        ) -> Json<Value> {
            counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(
                payload["chat_template_kwargs"]["enable_thinking"],
                false
            );
            Json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "",
                        "reasoning_content": "hidden reasoning"
                    },
                    "finish_reason": "length"
                }],
                "usage": {
                    "prompt_tokens": 20,
                    "completion_tokens": 64,
                    "total_tokens": 84
                }
            }))
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/v1/chat/completions", post(completion))
            .with_state(counter.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let config = ResolvedLlmConfig {
            provider: "custom".to_string(),
            base_url: format!("http://{address}/v1"),
            model: "reasoning-model".to_string(),
            api_key: String::new(),
            temperature: 0.2,
            max_tokens: 64,
            request_timeout_seconds: 5,
            use_mock: false,
            thinking_mode: "disabled".to_string(),
            retry_empty_reasoning: false,
            mcp_enabled: false,
            mcp_server_url: String::new(),
            mcp_integration_id: String::new(),
        };
        let provider = OpenAICompatibleProvider::new(&config).unwrap();
        let error = provider
            .generate(&[ChatMessage::user("hello")])
            .await
            .unwrap_err();
        assert_eq!(error.code, "empty_completion");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        server.abort();
    }
}
