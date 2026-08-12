use crate::config::{ResolvedTtsConfig, TtsSettings, VoiceConfig};
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{json, Value};
use std::fmt;
use std::time::Duration;

const MAX_TTS_INPUT_CHARS: usize = 4096;
const MAX_AUDIO_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct VoiceStatus {
    pub tts_provider: String,
    pub tts_available: bool,
    pub tts_location: String,
    pub tts_auto_play: bool,
    pub asr_provider: String,
    pub asr_location: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SynthesizedAudio {
    pub provider: String,
    pub model: String,
    pub voice: String,
    pub mime_type: String,
    pub audio_base64: String,
}

impl SynthesizedAudio {
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime_type, self.audio_base64)
    }
}

#[derive(Debug, Clone)]
pub struct VoiceError {
    pub code: &'static str,
    pub message: String,
    pub status: Option<StatusCode>,
}

impl VoiceError {
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

impl fmt::Display for VoiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for VoiceError {}

pub fn status(config: &VoiceConfig) -> VoiceStatus {
    let resolved = config.tts.resolve();
    let is_frontend = resolved.provider == "browser";
    let is_backend = matches!(
        resolved.provider.as_str(),
        "openai" | "mimo" | "custom-openai"
    );
    let requires_key = matches!(resolved.provider.as_str(), "openai" | "mimo");
    let backend_ready = is_backend
        && !resolved.base_url.is_empty()
        && !resolved.model.is_empty()
        && !resolved.voice.is_empty()
        && (!requires_key || !resolved.api_key.is_empty());
    let tts_available = is_frontend || backend_ready;
    let detail = match resolved.provider.as_str() {
        "none" => "TTS is disabled".to_string(),
        "browser" => {
            "Browser SpeechSynthesis is selected; availability is checked in the UI"
                .to_string()
        }
        _ if backend_ready => format!(
            "{} TTS backend is configured with model '{}'",
            resolved.provider, resolved.model
        ),
        _ if requires_key && resolved.api_key.is_empty() => format!(
            "{} TTS requires an API key in the configured environment variable",
            resolved.provider
        ),
        _ => format!("{} TTS configuration is incomplete", resolved.provider),
    };
    VoiceStatus {
        tts_provider: resolved.provider,
        tts_available,
        tts_location: if is_frontend {
            "frontend".to_string()
        } else if is_backend {
            "backend".to_string()
        } else {
            "disabled".to_string()
        },
        tts_auto_play: resolved.auto_play,
        asr_provider: config.asr.provider.clone(),
        asr_location: if config.asr.provider == "browser" {
            "frontend".to_string()
        } else {
            "not_implemented".to_string()
        },
        detail,
    }
}

pub async fn synthesize(
    settings: &TtsSettings,
    text: &str,
) -> Result<SynthesizedAudio, VoiceError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(VoiceError::new(
            "empty_tts_input",
            "TTS input must not be empty",
        ));
    }
    if text.chars().count() > MAX_TTS_INPUT_CHARS {
        return Err(VoiceError::new(
            "tts_input_too_large",
            format!("TTS input exceeds {MAX_TTS_INPUT_CHARS} characters"),
        ));
    }
    let resolved = settings.resolve();
    match resolved.provider.as_str() {
        "openai" | "custom-openai" => synthesize_openai(&resolved, text).await,
        "mimo" => synthesize_mimo(&resolved, text).await,
        "browser" => Err(VoiceError::new(
            "frontend_tts",
            "browser TTS must be synthesized by window.speechSynthesis",
        )),
        "none" => Err(VoiceError::new("tts_disabled", "TTS is disabled")),
        _ => Err(VoiceError::new(
            "unsupported_tts_provider",
            format!("unsupported TTS provider '{}'", resolved.provider),
        )),
    }
}

async fn synthesize_openai(
    config: &ResolvedTtsConfig,
    text: &str,
) -> Result<SynthesizedAudio, VoiceError> {
    validate_backend_config(config)?;
    let client = tts_client()?;
    let url = format!("{}/audio/speech", config.base_url.trim_end_matches('/'));
    let mut request = client.post(&url);
    if !config.api_key.is_empty() {
        request = request.bearer_auth(&config.api_key);
    }
    let mut payload = json!({
        "model": config.model,
        "input": text,
        "voice": config.voice,
        "response_format": config.response_format,
        "speed": config.speed,
    });
    if !config.style.is_empty() {
        payload["instructions"] = Value::String(config.style.clone());
    }
    let response = request.json(&payload).send().await.map_err(|error| {
        VoiceError::new(
            "tts_connection_failed",
            format!("failed to reach {}: {error}", config.base_url),
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(VoiceError::with_status(
            "tts_provider_error",
            status,
            truncate(&body),
        ));
    }
    if response.content_length().is_some_and(|size| size as usize > MAX_AUDIO_BYTES) {
        return Err(VoiceError::new(
            "tts_audio_too_large",
            "TTS response exceeded the audio size limit",
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| VoiceError::new("tts_invalid_response", error.to_string()))?;
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
        return Err(VoiceError::new(
            "tts_invalid_response",
            "TTS provider returned empty or oversized audio",
        ));
    }
    Ok(SynthesizedAudio {
        provider: config.provider.clone(),
        model: config.model.clone(),
        voice: config.voice.clone(),
        mime_type: audio_mime_type(&config.response_format).to_string(),
        audio_base64: base64_encode(&bytes),
    })
}

async fn synthesize_mimo(
    config: &ResolvedTtsConfig,
    text: &str,
) -> Result<SynthesizedAudio, VoiceError> {
    validate_backend_config(config)?;
    if config.api_key.is_empty() {
        return Err(VoiceError::new(
            "tts_api_key_missing",
            "MiMo TTS requires MIMO_API_KEY or a session API key",
        ));
    }
    let client = tts_client()?;
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let mut messages = Vec::new();
    if !config.style.is_empty() {
        messages.push(json!({"role": "user", "content": config.style}));
    }
    messages.push(json!({"role": "assistant", "content": text}));
    let payload = json!({
        "model": config.model,
        "stream": false,
        "messages": messages,
        "audio": {
            "format": config.response_format,
            "voice": config.voice,
        }
    });
    let response = client
        .post(&url)
        .bearer_auth(&config.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            VoiceError::new(
                "tts_connection_failed",
                format!("failed to reach {}: {error}", config.base_url),
            )
        })?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| VoiceError::new("tts_invalid_response", error.to_string()))?;
    if !status.is_success() {
        return Err(VoiceError::with_status(
            "tts_provider_error",
            status,
            truncate(&body),
        ));
    }
    let value: Value = serde_json::from_str(&body).map_err(|error| {
        VoiceError::new(
            "tts_invalid_response",
            format!("MiMo returned invalid JSON: {error}"),
        )
    })?;
    let audio_base64 = value
        .pointer("/choices/0/message/audio/data")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            VoiceError::new(
                "tts_invalid_response",
                "MiMo returned no choices[0].message.audio.data",
            )
        })?;
    if audio_base64.len() > MAX_AUDIO_BYTES * 2 {
        return Err(VoiceError::new(
            "tts_audio_too_large",
            "MiMo audio response exceeded the size limit",
        ));
    }
    Ok(SynthesizedAudio {
        provider: config.provider.clone(),
        model: config.model.clone(),
        voice: config.voice.clone(),
        mime_type: audio_mime_type(&config.response_format).to_string(),
        audio_base64: audio_base64.to_string(),
    })
}

fn validate_backend_config(config: &ResolvedTtsConfig) -> Result<(), VoiceError> {
    if config.base_url.is_empty() || config.model.is_empty() || config.voice.is_empty() {
        Err(VoiceError::new(
            "tts_invalid_config",
            "TTS base_url, model, and voice must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn tts_client() -> Result<reqwest::Client, VoiceError> {
    crate::http_client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| VoiceError::new("tts_client_init_failed", error.to_string()))
}

fn audio_mime_type(format: &str) -> &'static str {
    match format {
        "wav" => "audio/wav",
        "opus" => "audio/ogg; codecs=opus",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        _ => "audio/mpeg",
    }
}

fn truncate(body: &str) -> String {
    if body.chars().count() <= 1200 {
        body.to_string()
    } else {
        let mut result = body.chars().take(1200).collect::<String>();
        result.push('…');
        result
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(c & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TtsSettings;
    use axum::{extract::Json, routing::post, Router};
    use tokio::net::TcpListener;

    #[test]
    fn disabled_tts_is_reported_truthfully() {
        let status = status(&VoiceConfig::default());
        assert!(!status.tts_available);
        assert_eq!(status.tts_provider, "none");
    }

    #[test]
    fn browser_tts_is_reported_as_frontend_capability() {
        let mut config = VoiceConfig::default();
        config.tts.provider = "browser".to_string();
        let status = status(&config);
        assert!(status.tts_available);
        assert_eq!(status.tts_location, "frontend");
    }

    #[test]
    fn base64_encoder_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[tokio::test]
    async fn openai_tts_adapter_returns_audio() {
        async fn speech(Json(value): Json<Value>) -> ([(&'static str, &'static str); 1], Vec<u8>) {
            assert_eq!(value["model"], "test-tts");
            assert_eq!(value["voice"], "test-voice");
            ([("content-type", "audio/mpeg")], b"fake-mp3".to_vec())
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/v1/audio/speech", post(speech)),
            )
            .await
            .unwrap();
        });
        let settings = TtsSettings {
            provider: "custom-openai".to_string(),
            base_url: format!("http://{address}/v1"),
            model: "test-tts".to_string(),
            voice: "test-voice".to_string(),
            ..TtsSettings::default()
        };
        let result = synthesize(&settings, "hello").await.unwrap();
        assert_eq!(result.audio_base64, "ZmFrZS1tcDM=");
        server.abort();
    }

    #[tokio::test]
    async fn mimo_tts_adapter_reads_base64_audio() {
        async fn speech(Json(value): Json<Value>) -> Json<Value> {
            assert_eq!(value["model"], "mimo-v2.5-tts");
            assert_eq!(value["messages"][0]["role"], "assistant");
            Json(json!({
                "choices": [{"message": {"audio": {"data": "UklGRg=="}}}]
            }))
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/v1/chat/completions", post(speech)),
            )
            .await
            .unwrap();
        });
        let settings = TtsSettings {
            provider: "mimo".to_string(),
            base_url: format!("http://{address}/v1"),
            model: "mimo-v2.5-tts".to_string(),
            voice: "冰糖".to_string(),
            api_key: "test-key".to_string(),
            response_format: "wav".to_string(),
            ..TtsSettings::default()
        };
        let result = synthesize(&settings, "你好").await.unwrap();
        assert_eq!(result.audio_base64, "UklGRg==");
        assert_eq!(result.mime_type, "audio/wav");
        server.abort();
    }
}
