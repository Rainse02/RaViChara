use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_SETTINGS_PATH: &str = "config/settings.yaml";
pub const DEFAULT_OVERRIDES_PATH: &str = "data/overrides.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub character: CharacterConfig,
    pub llm: LlmConfig,
    pub embeddings: EmbeddingsConfig,
    pub memory: MemoryConfig,
    pub blender: BlenderConfig,
    pub video: VideoConfig,
    pub voice: VoiceConfig,
    pub server: ServerConfig,
    pub ui: UiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            character: CharacterConfig::default(),
            llm: LlmConfig::default(),
            embeddings: EmbeddingsConfig::default(),
            memory: MemoryConfig::default(),
            blender: BlenderConfig::default(),
            video: VideoConfig::default(),
            voice: VoiceConfig::default(),
            server: ServerConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterConfig {
    pub card: String,
    pub user_name: String,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        Self {
            card: "characters/lily.card.yaml".to_string(),
            user_name: "you".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub api_key_env: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub request_timeout_seconds: u64,
    pub mock_when_no_key: bool,
    pub thinking_mode: String,
    pub retry_empty_reasoning: bool,
    pub example_dialogue_limit: usize,
    pub mcp_enabled: bool,
    pub mcp_server_url: String,
    pub mcp_integration_id: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "lmstudio".to_string(),
            base_url: "http://127.0.0.1:1234/v1".to_string(),
            model: "qwen3.5-9b-uncensored-hauhaucs-aggressive".to_string(),
            api_key: String::new(),
            api_key_env: String::new(),
            temperature: 0.85,
            max_tokens: 2048,
            request_timeout_seconds: 180,
            mock_when_no_key: true,
            thinking_mode: "disabled".to_string(),
            retry_empty_reasoning: false,
            example_dialogue_limit: 1,
            mcp_enabled: true,
            mcp_server_url: "http://127.0.0.1:8760/mcp".to_string(),
            mcp_integration_id: "mcp/ravichara".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLlmConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub request_timeout_seconds: u64,
    pub use_mock: bool,
    pub thinking_mode: String,
    pub retry_empty_reasoning: bool,
    pub mcp_enabled: bool,
    pub mcp_server_url: String,
    pub mcp_integration_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub api_key_env: &'static str,
    pub requires_api_key: bool,
}

pub fn llm_provider_presets() -> Vec<LlmProviderPreset> {
    vec![
        LlmProviderPreset {
            id: "lmstudio",
            label: "LM Studio（本地）",
            base_url: "http://127.0.0.1:1234/v1",
            default_model: "qwen3.5-9b-uncensored-hauhaucs-aggressive",
            api_key_env: "LM_API_TOKEN",
            requires_api_key: false,
        },
        LlmProviderPreset {
            id: "ollama",
            label: "Ollama（本地）",
            base_url: "http://127.0.0.1:11434/v1",
            default_model: "qwen3.5:9b",
            api_key_env: "",
            requires_api_key: false,
        },
        LlmProviderPreset {
            id: "openai",
            label: "OpenAI",
            base_url: "https://api.openai.com/v1",
            default_model: "gpt-5.2",
            api_key_env: "OPENAI_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "deepseek",
            label: "DeepSeek",
            base_url: "https://api.deepseek.com",
            default_model: "deepseek-v4-flash",
            api_key_env: "DEEPSEEK_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "qwen",
            label: "阿里云百炼 / Qwen",
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
            default_model: "qwen3.7-plus",
            api_key_env: "DASHSCOPE_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "siliconflow",
            label: "SiliconFlow",
            base_url: "https://api.siliconflow.cn/v1",
            default_model: "Qwen/Qwen3.5-9B",
            api_key_env: "SILICONFLOW_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "zhipu",
            label: "智谱 GLM",
            base_url: "https://open.bigmodel.cn/api/paas/v4",
            default_model: "glm-5.2",
            api_key_env: "ZHIPU_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "openrouter",
            label: "OpenRouter",
            base_url: "https://openrouter.ai/api/v1",
            default_model: "openrouter/auto",
            api_key_env: "OPENROUTER_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "groq",
            label: "Groq",
            base_url: "https://api.groq.com/openai/v1",
            default_model: "llama-3.1-8b-instant",
            api_key_env: "GROQ_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "gemini",
            label: "Google Gemini",
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
            default_model: "gemini-3.6-flash",
            api_key_env: "GEMINI_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "mistral",
            label: "Mistral AI",
            base_url: "https://api.mistral.ai/v1",
            default_model: "mistral-small-latest",
            api_key_env: "MISTRAL_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "mimo",
            label: "小米 MiMo",
            base_url: "https://api.xiaomimimo.com/v1",
            default_model: "mimo-v2.5",
            api_key_env: "MIMO_API_KEY",
            requires_api_key: true,
        },
        LlmProviderPreset {
            id: "custom",
            label: "自定义 OpenAI-compatible",
            base_url: "",
            default_model: "",
            api_key_env: "CUSTOM_LLM_API_KEY",
            requires_api_key: false,
        },
        LlmProviderPreset {
            id: "mock",
            label: "离线 Mock（仅测试）",
            base_url: "",
            default_model: "mock",
            api_key_env: "",
            requires_api_key: false,
        },
    ]
}

impl LlmConfig {
    pub fn resolve(&self) -> ResolvedLlmConfig {
        let provider = self.provider.trim().to_lowercase();
        let preset = llm_provider_presets()
            .into_iter()
            .find(|candidate| candidate.id == provider);
        let preset_url = preset
            .as_ref()
            .map(|value| value.base_url)
            .unwrap_or_default();
        let preset_model = preset
            .as_ref()
            .map(|value| value.default_model)
            .unwrap_or_default();
        let preset_api_key_env = preset
            .as_ref()
            .map(|value| value.api_key_env)
            .unwrap_or_default();
        let requires_key = preset
            .as_ref()
            .map(|value| value.requires_api_key)
            .unwrap_or(false);

        let mut base_url = if self.base_url.trim().is_empty() {
            preset_url.to_string()
        } else {
            self.base_url.trim().trim_end_matches('/').to_string()
        };
        if matches!(provider.as_str(), "lmstudio" | "ollama")
            && !base_url.ends_with("/v1")
        {
            base_url.push_str("/v1");
        }

        let model = if self.model.trim().is_empty() {
            preset_model.to_string()
        } else {
            self.model.trim().to_string()
        };

        let api_key_env = if self.api_key_env.trim().is_empty() {
            preset_api_key_env
        } else {
            self.api_key_env.trim()
        };
        let api_key = if !self.api_key.trim().is_empty() {
            self.api_key.trim().to_string()
        } else if !api_key_env.is_empty() {
            std::env::var(api_key_env).unwrap_or_default()
        } else {
            String::new()
        };

        let use_mock = provider == "mock"
            || (requires_key && api_key.is_empty() && self.mock_when_no_key);

        ResolvedLlmConfig {
            provider,
            base_url,
            model,
            api_key,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            request_timeout_seconds: self.request_timeout_seconds,
            use_mock,
            thinking_mode: self.thinking_mode.trim().to_lowercase(),
            retry_empty_reasoning: self.retry_empty_reasoning,
            mcp_enabled: self.mcp_enabled,
            mcp_server_url: self.mcp_server_url.trim().to_string(),
            mcp_integration_id: self.mcp_integration_id.trim().to_string(),
        }
    }
}

impl ResolvedLlmConfig {
    pub fn mcp_model_access_enabled(&self) -> bool {
        self.mcp_enabled
            && (!self.mcp_integration_id.is_empty()
                || self.mcp_server_url.starts_with("https://"))
    }

    pub fn mcp_mode(&self) -> &'static str {
        if !self.mcp_enabled {
            "disabled"
        } else if !self.mcp_integration_id.is_empty() {
            "lmstudio_plugin"
        } else if self.mcp_server_url.starts_with("https://") {
            "ephemeral_remote"
        } else {
            "local_server_only"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingsConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub dimensions: usize,
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            provider: "hash".to_string(),
            base_url: String::new(),
            model: "local-hash-v1".to_string(),
            api_key_env: String::new(),
            dimensions: 384,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub db_path: String,
    pub directory: String,
    pub working_max_messages: usize,
    pub working_max_chars: usize,
    pub retrieve_facts: usize,
    pub retrieve_episodes: usize,
    pub consolidate_after: usize,
    pub consolidate_keep: usize,
    pub chapter_merge_at: usize,
    pub recency_halflife_days: f32,
    pub dedupe_similarity: f32,
    pub max_messages: usize,
    pub max_facts: usize,
    pub max_episodes: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            db_path: "data/memory.db".to_string(),
            directory: "data/memories".to_string(),
            working_max_messages: 24,
            working_max_chars: 7000,
            retrieve_facts: 6,
            retrieve_episodes: 3,
            consolidate_after: 28,
            consolidate_keep: 10,
            chapter_merge_at: 40,
            recency_halflife_days: 30.0,
            dedupe_similarity: 0.90,
            max_messages: 0,
            max_facts: 0,
            max_episodes: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BlenderConfig {
    pub enabled: bool,
    pub profile: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub token_env: String,
    pub model_name: String,
    pub render_mode: String,
    pub snapshot_size: (u32, u32),
    pub animation_frames: u32,
    pub auto_downgrade_ms: u64,
    pub stream_fps: u32,
    pub stream_size: (u32, u32),
    pub stream_size_mode: String,
    pub playback_start_frame: u32,
    pub action_start_frame: u32,
    pub action_end_frame: u32,
    pub playback_end_frame: u32,
    pub transition_frames: u32,
    pub control_analysis_enabled: bool,
    pub control_analysis_model: String,
    pub scene_proposals_enabled: bool,
}

impl Default for BlenderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            profile: "virtual_c".to_string(),
            host: "127.0.0.1".to_string(),
            port: 9876,
            token: String::new(),
            token_env: "RAVICHARA_BLENDER_TOKEN".to_string(),
            model_name: String::new(),
            render_mode: "pose".to_string(),
            snapshot_size: (512, 512),
            animation_frames: 24,
            auto_downgrade_ms: 4000,
            stream_fps: 12,
            stream_size: (512, 512),
            stream_size_mode: "camera".to_string(),
            playback_start_frame: 1,
            action_start_frame: 9,
            action_end_frame: 41,
            playback_end_frame: 49,
            transition_frames: 8,
            control_analysis_enabled: true,
            control_analysis_model: String::new(),
            scene_proposals_enabled: true,
        }
    }
}

impl BlenderConfig {
    pub fn effective_token(&self) -> String {
        if !self.token.trim().is_empty() {
            self.token.trim().to_string()
        } else if !self.token_env.trim().is_empty() {
            let configured = self.token_env.trim();
            std::env::var(configured).unwrap_or_else(|_| {
                if configured == "RAVICHARA_BLENDER_TOKEN" {
                    std::env::var("EVERCHARA_BLENDER_TOKEN").unwrap_or_default()
                } else {
                    String::new()
                }
            })
        } else {
            String::new()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoConfig {
    pub enabled: bool,
    pub source_type: String,
    pub source_url: String,
    pub autoplay: bool,
    pub loop_playback: bool,
    pub muted: bool,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            source_type: "video".to_string(),
            source_url: String::new(),
            autoplay: true,
            loop_playback: true,
            muted: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VoiceConfig {
    pub tts: TtsSettings,
    pub asr: AsrSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsSettings {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub voice: String,
    pub api_key: String,
    pub api_key_env: String,
    pub response_format: String,
    pub speed: f32,
    pub style: String,
    pub auto_play: bool,
    pub split_sentences: bool,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            base_url: String::new(),
            model: String::new(),
            voice: String::new(),
            api_key: String::new(),
            api_key_env: String::new(),
            response_format: "mp3".to_string(),
            speed: 1.0,
            style: String::new(),
            auto_play: false,
            split_sentences: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTtsConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub voice: String,
    pub api_key: String,
    pub response_format: String,
    pub speed: f32,
    pub style: String,
    pub auto_play: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub default_voice: &'static str,
    pub default_format: &'static str,
    pub api_key_env: &'static str,
    pub backend: bool,
}

pub fn tts_provider_presets() -> Vec<TtsProviderPreset> {
    vec![
        TtsProviderPreset {
            id: "none",
            label: "关闭语音合成",
            base_url: "",
            default_model: "",
            default_voice: "",
            default_format: "mp3",
            api_key_env: "",
            backend: false,
        },
        TtsProviderPreset {
            id: "browser",
            label: "浏览器 SpeechSynthesis（免 API）",
            base_url: "",
            default_model: "browser",
            default_voice: "",
            default_format: "mp3",
            api_key_env: "",
            backend: false,
        },
        TtsProviderPreset {
            id: "openai",
            label: "OpenAI TTS",
            base_url: "https://api.openai.com/v1",
            default_model: "gpt-4o-mini-tts",
            default_voice: "alloy",
            default_format: "mp3",
            api_key_env: "OPENAI_API_KEY",
            backend: true,
        },
        TtsProviderPreset {
            id: "mimo",
            label: "小米 MiMo V2.5 TTS",
            base_url: "https://api.xiaomimimo.com/v1",
            default_model: "mimo-v2.5-tts",
            default_voice: "冰糖",
            default_format: "wav",
            api_key_env: "MIMO_API_KEY",
            backend: true,
        },
        TtsProviderPreset {
            id: "custom-openai",
            label: "自定义 OpenAI-compatible TTS",
            base_url: "",
            default_model: "",
            default_voice: "",
            default_format: "mp3",
            api_key_env: "CUSTOM_TTS_API_KEY",
            backend: true,
        },
    ]
}

impl TtsSettings {
    pub fn resolve(&self) -> ResolvedTtsConfig {
        let provider = self.provider.trim().to_lowercase();
        let preset = tts_provider_presets()
            .into_iter()
            .find(|candidate| candidate.id == provider);
        let preset_value = |selector: fn(&TtsProviderPreset) -> &'static str| {
            preset.as_ref().map(selector).unwrap_or_default()
        };
        let pick = |configured: &str, fallback: &str| {
            if configured.trim().is_empty() {
                fallback.to_string()
            } else {
                configured.trim().to_string()
            }
        };
        let api_key_env = if self.api_key_env.trim().is_empty() {
            preset_value(|value| value.api_key_env)
        } else {
            self.api_key_env.trim()
        };
        let api_key = if !self.api_key.trim().is_empty() {
            self.api_key.trim().to_string()
        } else if api_key_env.is_empty() {
            String::new()
        } else {
            std::env::var(api_key_env).unwrap_or_default()
        };
        ResolvedTtsConfig {
            provider,
            base_url: pick(
                self.base_url.trim().trim_end_matches('/'),
                preset_value(|value| value.base_url),
            ),
            model: pick(&self.model, preset_value(|value| value.default_model)),
            voice: pick(&self.voice, preset_value(|value| value.default_voice)),
            api_key,
            response_format: if self.response_format.trim().is_empty() {
                preset_value(|value| value.default_format).to_string()
            } else {
                self.response_format.trim().to_lowercase()
            },
            speed: self.speed,
            style: self.style.trim().to_string(),
            auto_play: self.auto_play,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AsrSettings {
    pub provider: String,
}

impl Default for AsrSettings {
    fn default() -> Self {
        Self {
            provider: "browser".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub open_browser: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8760,
            open_browser: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub language: String,
    pub greet_on_connect: bool,
    pub welcome_back_hours: u32,
    pub user_bubble_bg: Option<String>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: "zh".to_string(),
            greet_on_connect: true,
            welcome_back_hours: 8,
            user_bubble_bg: Some("#B6EBEC".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct RuntimeOverrides {
    character_card: Option<String>,
    language: Option<String>,
    user_bubble_bg: Option<String>,
    llm_provider: Option<String>,
    llm_base_url: Option<String>,
    llm_model: Option<String>,
    llm_api_key: Option<String>,
    llm_temperature: Option<f32>,
    llm_max_tokens: Option<u32>,
    llm_thinking_mode: Option<String>,
    llm_example_dialogue_limit: Option<usize>,
    llm_mcp_enabled: Option<bool>,
    llm_mcp_server_url: Option<String>,
    llm_mcp_integration_id: Option<String>,
    blender_enabled: Option<bool>,
    blender_profile: Option<String>,
    blender_render_mode: Option<String>,
    blender_stream_fps: Option<u32>,
    blender_stream_width: Option<u32>,
    blender_stream_height: Option<u32>,
    blender_stream_size_mode: Option<String>,
    blender_playback_start_frame: Option<u32>,
    blender_action_start_frame: Option<u32>,
    blender_action_end_frame: Option<u32>,
    blender_playback_end_frame: Option<u32>,
    blender_transition_frames: Option<u32>,
    blender_control_analysis_enabled: Option<bool>,
    blender_control_analysis_model: Option<String>,
    blender_scene_proposals_enabled: Option<bool>,
    video_enabled: Option<bool>,
    video_source_type: Option<String>,
    video_source_url: Option<String>,
    video_autoplay: Option<bool>,
    video_loop_playback: Option<bool>,
    video_muted: Option<bool>,
    tts_provider: Option<String>,
    tts_base_url: Option<String>,
    tts_model: Option<String>,
    tts_voice: Option<String>,
    tts_api_key: Option<String>,
    tts_response_format: Option<String>,
    tts_speed: Option<f32>,
    tts_style: Option<String>,
    tts_auto_play: Option<bool>,
}

impl AppConfig {
    pub fn load() -> Result<Self, String> {
        Self::load_from_paths(DEFAULT_SETTINGS_PATH, DEFAULT_OVERRIDES_PATH)
    }

    pub fn load_from_paths<P: AsRef<Path>, Q: AsRef<Path>>(
        settings_path: P,
        overrides_path: Q,
    ) -> Result<Self, String> {
        let settings_path = settings_path.as_ref();
        let mut config = if settings_path.exists() {
            let content = fs::read_to_string(settings_path).map_err(|error| {
                format!(
                    "failed to read settings file {}: {error}",
                    settings_path.display()
                )
            })?;
            serde_yaml::from_str::<AppConfig>(&content).map_err(|error| {
                format!(
                    "invalid settings file {}: {error}",
                    settings_path.display()
                )
            })?
        } else {
            Self::default()
        };

        let overrides_path = overrides_path.as_ref();
        if overrides_path.exists() {
            let content = fs::read_to_string(overrides_path).map_err(|error| {
                format!(
                    "failed to read overrides file {}: {error}",
                    overrides_path.display()
                )
            })?;
            let overrides =
                serde_json::from_str::<RuntimeOverrides>(&content).map_err(|error| {
                    format!(
                        "invalid overrides file {}: {error}",
                        overrides_path.display()
                    )
                })?;
            config.apply_overrides(overrides);
        }

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        let provider = self.llm.provider.trim().to_lowercase();
        if !llm_provider_presets()
            .iter()
            .any(|candidate| candidate.id == provider)
        {
            return Err(format!("unsupported LLM provider: {}", self.llm.provider));
        }
        let resolved = self.llm.resolve();
        if !resolved.use_mock && resolved.base_url.trim().is_empty() {
            return Err("llm.base_url must not be empty".to_string());
        }
        if !resolved.use_mock && resolved.model.trim().is_empty() {
            return Err("llm.model must not be empty".to_string());
        }
        if !(0.0..=2.0).contains(&self.llm.temperature) {
            return Err("llm.temperature must be between 0.0 and 2.0".to_string());
        }
        if self.llm.max_tokens == 0 || self.llm.max_tokens > 32768 {
            return Err("llm.max_tokens must be between 1 and 32768".to_string());
        }
        if self.llm.request_timeout_seconds == 0 {
            return Err("llm.request_timeout_seconds must be positive".to_string());
        }
        if !matches!(
            self.llm.thinking_mode.trim().to_lowercase().as_str(),
            "auto" | "disabled" | "enabled"
        ) {
            return Err(
                "llm.thinking_mode must be auto, disabled, or enabled".to_string(),
            );
        }
        if self.llm.example_dialogue_limit > 20 {
            return Err("llm.example_dialogue_limit must not exceed 20".to_string());
        }
        if self.llm.mcp_enabled
            && self.llm.mcp_integration_id.is_empty()
            && (!self.llm.mcp_server_url.starts_with("http://")
                && !self.llm.mcp_server_url.starts_with("https://")
                || self.llm.mcp_server_url.chars().count() > 2048
                || self.llm.mcp_server_url.contains(['\r', '\n']))
        {
            return Err(
                "llm.mcp_server_url must be a bounded http(s) URL when MCP is enabled"
                    .to_string(),
            );
        }
        if !self.llm.mcp_integration_id.is_empty()
            && (!self.llm.mcp_integration_id.starts_with("mcp/")
                || self.llm.mcp_integration_id.len() > 128
                || !self
                    .llm
                    .mcp_integration_id
                    .chars()
                    .all(|character| {
                        character.is_ascii_alphanumeric()
                            || matches!(character, '/' | '-' | '_' | '.')
                    }))
        {
            return Err(
                "llm.mcp_integration_id must use the bounded form mcp/server-name"
                    .to_string(),
            );
        }
        if !matches!(self.ui.language.as_str(), "zh" | "en") {
            return Err("ui.language must be zh or en".to_string());
        }
        if self.memory.working_max_messages == 0
            || self.memory.working_max_chars == 0
            || self.memory.consolidate_keep >= self.memory.consolidate_after
        {
            return Err(
                "memory window sizes are invalid; consolidate_keep must be smaller than consolidate_after"
                    .to_string(),
            );
        }
        if !(0.0..=1.0).contains(&self.memory.dedupe_similarity) {
            return Err("memory.dedupe_similarity must be between 0 and 1".to_string());
        }
        if (self.memory.max_messages > 0
            && self.memory.max_messages < self.memory.working_max_messages)
            || (self.memory.max_facts > 0
                && self.memory.max_facts < self.memory.retrieve_facts)
            || (self.memory.max_episodes > 0
                && self.memory.max_episodes < self.memory.retrieve_episodes)
        {
            return Err(
                "non-zero memory retention limits must be at least as large as their retrieval windows"
                    .to_string(),
            );
        }
        if !(1..=30).contains(&self.blender.stream_fps)
            || !(128..=1024).contains(&self.blender.stream_size.0)
            || !(128..=1024).contains(&self.blender.stream_size.1)
        {
            return Err(
                "blender stream_fps must be 1..=30 and stream_size dimensions 128..=1024"
                    .to_string(),
            );
        }
        if !matches!(self.blender.stream_size_mode.as_str(), "camera" | "custom") {
            return Err(format!(
                "unsupported blender.stream_size_mode: {}; expected camera or custom",
                self.blender.stream_size_mode
            ));
        }
        if self.blender.playback_start_frame < 1
            || self.blender.playback_start_frame > self.blender.action_start_frame
            || self.blender.action_start_frame >= self.blender.action_end_frame
            || self.blender.action_end_frame > self.blender.playback_end_frame
            || self.blender.playback_end_frame > 1_000_000
        {
            return Err(
                "Blender frame ranges must satisfy playback_start <= action_start < action_end <= playback_end"
                    .to_string(),
            );
        }
        if self.blender.transition_frames > 120 {
            return Err("blender.transition_frames must be between 0 and 120".to_string());
        }
        if self.blender.action_start_frame - self.blender.playback_start_frame
            < self.blender.transition_frames
            || self.blender.playback_end_frame - self.blender.action_end_frame
                < self.blender.transition_frames
        {
            return Err(
                "Blender playback range must leave transition_frames before and after the action range"
                    .to_string(),
            );
        }
        if !matches!(
            self.blender.render_mode.as_str(),
            "off" | "pose"
        ) {
            return Err(format!(
                "unsupported blender.render_mode: {}; the current runtime supports only off and pose",
                self.blender.render_mode
            ));
        }
        if !matches!(self.video.source_type.as_str(), "video" | "mjpeg") {
            return Err(format!(
                "unsupported video.source_type: {}; expected video or mjpeg",
                self.video.source_type
            ));
        }
        let video_url = self.video.source_url.trim();
        if video_url.chars().count() > 2048 || video_url.contains(['\r', '\n']) {
            return Err(
                "video.source_url must not exceed 2048 characters or contain newlines"
                    .to_string(),
            );
        }
        if self.video.enabled
            && (!video_url.starts_with("http://")
                && !video_url.starts_with("https://"))
        {
            return Err(
                "enabled video.source_url must use an http(s) URL".to_string(),
            );
        }
        let tts_provider = self.voice.tts.provider.trim().to_lowercase();
        if !tts_provider_presets()
            .iter()
            .any(|candidate| candidate.id == tts_provider)
        {
            return Err(format!(
                "unsupported TTS provider: {}",
                self.voice.tts.provider
            ));
        }
        let resolved_tts = self.voice.tts.resolve();
        if !matches!(tts_provider.as_str(), "none" | "browser")
            && (resolved_tts.base_url.is_empty()
                || resolved_tts.model.is_empty()
                || resolved_tts.voice.is_empty())
        {
            return Err(
                "backend TTS requires non-empty base_url, model, and voice".to_string(),
            );
        }
        if !matches!(
            resolved_tts.response_format.as_str(),
            "mp3" | "wav" | "opus" | "aac" | "flac"
        ) {
            return Err(
                "voice.tts.response_format must be mp3, wav, opus, aac, or flac"
                    .to_string(),
            );
        }
        if !(0.25..=4.0).contains(&resolved_tts.speed) {
            return Err("voice.tts.speed must be between 0.25 and 4.0".to_string());
        }
        if resolved_tts.style.chars().count() > 500 {
            return Err("voice.tts.style must not exceed 500 characters".to_string());
        }
        Ok(())
    }

    pub fn save_runtime_overrides(&self) -> io::Result<()> {
        self.save_runtime_overrides_to(DEFAULT_OVERRIDES_PATH)
    }

    pub fn save_runtime_overrides_to<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let overrides = RuntimeOverrides {
            character_card: Some(self.character.card.clone()),
            language: Some(self.ui.language.clone()),
            user_bubble_bg: self.ui.user_bubble_bg.clone(),
            llm_provider: Some(self.llm.provider.clone()),
            llm_base_url: Some(self.llm.base_url.clone()),
            llm_model: Some(self.llm.model.clone()),
            llm_api_key: Some(self.llm.api_key.clone()),
            llm_temperature: Some(self.llm.temperature),
            llm_max_tokens: Some(self.llm.max_tokens),
            llm_thinking_mode: Some(self.llm.thinking_mode.clone()),
            llm_example_dialogue_limit: Some(self.llm.example_dialogue_limit),
            llm_mcp_enabled: Some(self.llm.mcp_enabled),
            llm_mcp_server_url: Some(self.llm.mcp_server_url.clone()),
            llm_mcp_integration_id: Some(self.llm.mcp_integration_id.clone()),
            blender_enabled: Some(self.blender.enabled),
            blender_profile: Some(self.blender.profile.clone()),
            blender_render_mode: Some(self.blender.render_mode.clone()),
            blender_stream_fps: Some(self.blender.stream_fps),
            blender_stream_width: Some(self.blender.stream_size.0),
            blender_stream_height: Some(self.blender.stream_size.1),
            blender_stream_size_mode: Some(self.blender.stream_size_mode.clone()),
            blender_playback_start_frame: Some(self.blender.playback_start_frame),
            blender_action_start_frame: Some(self.blender.action_start_frame),
            blender_action_end_frame: Some(self.blender.action_end_frame),
            blender_playback_end_frame: Some(self.blender.playback_end_frame),
            blender_transition_frames: Some(self.blender.transition_frames),
            blender_control_analysis_enabled: Some(self.blender.control_analysis_enabled),
            blender_control_analysis_model: Some(self.blender.control_analysis_model.clone()),
            blender_scene_proposals_enabled: Some(self.blender.scene_proposals_enabled),
            video_enabled: Some(self.video.enabled),
            video_source_type: Some(self.video.source_type.clone()),
            video_source_url: Some(self.video.source_url.clone()),
            video_autoplay: Some(self.video.autoplay),
            video_loop_playback: Some(self.video.loop_playback),
            video_muted: Some(self.video.muted),
            tts_provider: Some(self.voice.tts.provider.clone()),
            tts_base_url: Some(self.voice.tts.base_url.clone()),
            tts_model: Some(self.voice.tts.model.clone()),
            tts_voice: Some(self.voice.tts.voice.clone()),
            tts_api_key: Some(self.voice.tts.api_key.clone()),
            tts_response_format: Some(self.voice.tts.response_format.clone()),
            tts_speed: Some(self.voice.tts.speed),
            tts_style: Some(self.voice.tts.style.clone()),
            tts_auto_play: Some(self.voice.tts.auto_play),
        };
        let serialized = serde_json::to_string_pretty(&overrides)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        let temp_path = temporary_path(path);
        fs::write(&temp_path, serialized)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(temp_path, path)
    }

    fn apply_overrides(&mut self, overrides: RuntimeOverrides) {
        if let Some(value) = overrides.character_card {
            self.character.card = value;
        }
        if let Some(value) = overrides.language {
            self.ui.language = value;
        }
        if overrides.user_bubble_bg.is_some() {
            self.ui.user_bubble_bg = overrides.user_bubble_bg;
        }
        if let Some(value) = overrides.llm_provider {
            self.llm.provider = value;
        }
        if let Some(value) = overrides.llm_base_url {
            self.llm.base_url = value;
        }
        if let Some(value) = overrides.llm_model {
            self.llm.model = value;
        }
        if let Some(value) = overrides.llm_api_key {
            self.llm.api_key = value;
        }
        if let Some(value) = overrides.llm_temperature {
            self.llm.temperature = value;
        }
        if let Some(value) = overrides.llm_max_tokens {
            self.llm.max_tokens = value;
        }
        if let Some(value) = overrides.llm_thinking_mode {
            self.llm.thinking_mode = value;
        }
        if let Some(value) = overrides.llm_example_dialogue_limit {
            self.llm.example_dialogue_limit = value;
        }
        if let Some(value) = overrides.llm_mcp_enabled {
            self.llm.mcp_enabled = value;
        }
        if let Some(value) = overrides.llm_mcp_server_url {
            self.llm.mcp_server_url = value;
        }
        if let Some(value) = overrides.llm_mcp_integration_id {
            self.llm.mcp_integration_id = value;
        }
        if let Some(value) = overrides.blender_enabled {
            self.blender.enabled = value;
        }
        if let Some(value) = overrides.blender_profile {
            self.blender.profile = value;
        }
        if let Some(value) = overrides.blender_render_mode {
            self.blender.render_mode = value;
        }
        if let Some(value) = overrides.blender_stream_fps {
            self.blender.stream_fps = value;
        }
        if let Some(value) = overrides.blender_stream_width {
            self.blender.stream_size.0 = value;
        }
        if let Some(value) = overrides.blender_stream_height {
            self.blender.stream_size.1 = value;
        }
        if let Some(value) = overrides.blender_stream_size_mode {
            self.blender.stream_size_mode = value;
        }
        if let Some(value) = overrides.blender_playback_start_frame {
            self.blender.playback_start_frame = value;
        }
        if let Some(value) = overrides.blender_action_start_frame {
            self.blender.action_start_frame = value;
        }
        if let Some(value) = overrides.blender_action_end_frame {
            self.blender.action_end_frame = value;
        }
        if let Some(value) = overrides.blender_playback_end_frame {
            self.blender.playback_end_frame = value;
        }
        if let Some(value) = overrides.blender_transition_frames {
            self.blender.transition_frames = value;
        }
        if let Some(value) = overrides.blender_control_analysis_enabled {
            self.blender.control_analysis_enabled = value;
        }
        if let Some(value) = overrides.blender_control_analysis_model {
            self.blender.control_analysis_model = value;
        }
        if let Some(value) = overrides.blender_scene_proposals_enabled {
            self.blender.scene_proposals_enabled = value;
        }
        if let Some(value) = overrides.video_enabled {
            self.video.enabled = value;
        }
        if let Some(value) = overrides.video_source_type {
            self.video.source_type = value;
        }
        if let Some(value) = overrides.video_source_url {
            self.video.source_url = value;
        }
        if let Some(value) = overrides.video_autoplay {
            self.video.autoplay = value;
        }
        if let Some(value) = overrides.video_loop_playback {
            self.video.loop_playback = value;
        }
        if let Some(value) = overrides.video_muted {
            self.video.muted = value;
        }
        if let Some(value) = overrides.tts_provider {
            self.voice.tts.provider = value;
        }
        if let Some(value) = overrides.tts_base_url {
            self.voice.tts.base_url = value;
        }
        if let Some(value) = overrides.tts_model {
            self.voice.tts.model = value;
        }
        if let Some(value) = overrides.tts_voice {
            self.voice.tts.voice = value;
        }
        if let Some(value) = overrides.tts_api_key {
            self.voice.tts.api_key = value;
        }
        if let Some(value) = overrides.tts_response_format {
            self.voice.tts.response_format = value;
        }
        if let Some(value) = overrides.tts_speed {
            self.voice.tts.speed = value;
        }
        if let Some(value) = overrides.tts_style {
            self.voice.tts.style = value;
        }
        if let Some(value) = overrides.tts_auto_play {
            self.voice.tts.auto_play = value;
        }
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_else(|| "overrides.json".into());
    file_name.push(".tmp");
    path.with_file_name(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("everchara-{label}-{unique}"))
    }

    #[test]
    fn partial_yaml_uses_defaults() {
        let dir = test_dir("config-defaults");
        fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.yaml");
        fs::write(
            &settings,
            "llm:\n  provider: mock\nui:\n  language: en\n",
        )
        .unwrap();

        let config = AppConfig::load_from_paths(&settings, dir.join("none.json")).unwrap();
        assert_eq!(config.llm.provider, "mock");
        assert_eq!(config.llm.max_tokens, 2048);
        assert_eq!(config.llm.thinking_mode, "disabled");
        assert_eq!(config.ui.language, "en");
        assert_eq!(config.memory.directory, "data/memories");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lmstudio_root_url_is_normalized() {
        let config = LlmConfig {
            provider: "lmstudio".to_string(),
            base_url: "http://127.0.0.1:1234/".to_string(),
            model: "local-model".to_string(),
            ..LlmConfig::default()
        };
        let resolved = config.resolve();
        assert_eq!(resolved.base_url, "http://127.0.0.1:1234/v1");
        assert!(!resolved.use_mock);
    }

    #[test]
    fn provider_presets_are_unique_and_resolvable() {
        let presets = llm_provider_presets();
        assert!(presets.len() >= 10);
        let mut ids = presets.iter().map(|preset| preset.id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), presets.len());

        let config = LlmConfig {
            provider: "qwen".to_string(),
            base_url: String::new(),
            model: String::new(),
            ..LlmConfig::default()
        };
        let resolved = config.resolve();
        assert_eq!(
            resolved.base_url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(resolved.model, "qwen3.7-plus");

        let mimo = LlmConfig {
            provider: "mimo".to_string(),
            base_url: String::new(),
            model: String::new(),
            ..LlmConfig::default()
        }
        .resolve();
        assert_eq!(mimo.base_url, "https://api.xiaomimimo.com/v1");
        assert_eq!(mimo.model, "mimo-v2.5");
    }

    #[test]
    fn custom_provider_requires_explicit_url_and_model() {
        let mut config = AppConfig::default();
        config.llm.provider = "custom".to_string();
        config.llm.base_url.clear();
        config.llm.model.clear();
        assert!(config.validate().is_err());
        config.llm.base_url = "http://127.0.0.1:9000/v1".to_string();
        config.llm.model = "custom-model".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn external_video_requires_a_bounded_http_source() {
        let mut config = AppConfig::default();
        config.video.enabled = true;
        config.video.source_url = "file:///tmp/private.mp4".to_string();
        assert!(config.validate().is_err());
        config.video.source_url = "http://127.0.0.1:9001/live.mp4".to_string();
        assert!(config.validate().is_ok());
        config.video.source_type = "unknown".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn runtime_overrides_round_trip_with_local_api_keys() {
        let dir = test_dir("config-overrides");
        fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.yaml");
        let overrides = dir.join("overrides.json");
        fs::write(&settings, "{}").unwrap();

        let mut config = AppConfig::default();
        config.ui.language = "en".to_string();
        config.llm.model = "test-model".to_string();
        config.llm.api_key = "local-llm-key".to_string();
        config.voice.tts.api_key = "local-tts-key".to_string();
        config.video.enabled = true;
        config.video.source_url = "http://127.0.0.1:9001/live.mp4".to_string();
        config
            .save_runtime_overrides_to(&overrides)
            .unwrap();

        let persisted = fs::read_to_string(&overrides).unwrap();
        assert!(persisted.contains("local-llm-key"));
        assert!(persisted.contains("local-tts-key"));
        let loaded = AppConfig::load_from_paths(&settings, &overrides).unwrap();
        assert_eq!(loaded.ui.language, "en");
        assert_eq!(loaded.llm.model, "test-model");
        assert_eq!(loaded.llm.api_key, "local-llm-key");
        assert_eq!(loaded.voice.tts.api_key, "local-tts-key");
        assert!(loaded.video.enabled);
        assert_eq!(loaded.video.source_url, "http://127.0.0.1:9001/live.mp4");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tts_presets_resolve_and_validate() {
        let presets = tts_provider_presets();
        assert_eq!(presets.len(), 5);
        let mut config = AppConfig::default();
        config.voice.tts.provider = "mimo".to_string();
        config.voice.tts.base_url.clear();
        config.voice.tts.model.clear();
        config.voice.tts.voice.clear();
        config.voice.tts.response_format = "wav".to_string();
        let resolved = config.voice.tts.resolve();
        assert_eq!(resolved.base_url, "https://api.xiaomimimo.com/v1");
        assert_eq!(resolved.model, "mimo-v2.5-tts");
        assert_eq!(resolved.voice, "冰糖");
        assert!(config.validate().is_ok());
    }
}
