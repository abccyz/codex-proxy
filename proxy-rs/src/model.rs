/// Model vendor detection and capability flags.
/// Different vendors have different API requirements (e.g., reasoning_content handling).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelVendor {
    /// DeepSeek API - requires reasoning_content on every assistant message in thinking mode.
    DeepSeek,
    /// Qwen / DashScope - requires reasoning_content on every assistant message (even if empty)
    /// in thinking mode.
    Qwen,
    /// OpenAI - doesn't use reasoning_content field.
    OpenAI,
    /// Other/Unknown - conservative defaults.
    Other,
}

impl ModelVendor {
    /// Detect vendor from model name.
    pub fn from_model_name(model: &str) -> Self {
        let lower = model.to_lowercase();
        if lower.contains("deepseek") {
            Self::DeepSeek
        } else if lower.contains("qwen") || lower.contains("dashscope") {
            Self::Qwen
        } else if lower.contains("gpt") || lower.contains("openai") {
            Self::OpenAI
        } else {
            Self::Other
        }
    }

    /// Whether reasoning_content must be present on assistant messages.
    /// Both DeepSeek and Qwen require it in thinking mode.
    pub fn requires_reasoning_content(&self) -> bool {
        matches!(self, Self::DeepSeek | Self::Qwen)
    }

    /// Build reasoning_content value for output items (response back to Codex).
    /// Returns Some(content) if reasoning should be included, None otherwise.
    pub fn build_reasoning_output(&self, reasoning_content: &str) -> Option<String> {
        match self {
            Self::DeepSeek => {
                // DeepSeek: only include reasoning in output if non-empty
                if !reasoning_content.is_empty() {
                    Some(reasoning_content.to_string())
                } else {
                    None
                }
            }
            Self::Qwen => {
                // Qwen: always include (even if empty)
                Some(reasoning_content.to_string())
            }
            Self::OpenAI | Self::Other => None,
        }
    }

    /// Build reasoning_content value for input messages (sent to upstream API).
    /// Returns the value to use for the reasoning_content field.
    /// Both DeepSeek and Qwen require reasoning_content on assistant messages.
    pub fn build_reasoning_input(&self, reasoning_content: Option<&str>) -> Option<String> {
        match self {
            // Both DeepSeek and Qwen: always include reasoning_content (even if empty)
            Self::DeepSeek | Self::Qwen => {
                Some(reasoning_content.unwrap_or("").to_string())
            }
            Self::OpenAI | Self::Other => None,
        }
    }
}

/// Preset provider configuration for quick setup.
#[derive(Debug, Clone)]
pub struct ProviderPreset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub vendor: ModelVendor,
}

/// Get list of preset providers for quick config dropdown.
pub fn get_provider_presets() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            name: "openai",
            display_name: "OpenAI",
            base_url: "https://api.openai.com/v1",
            default_model: "gpt-4o",
            vendor: ModelVendor::OpenAI,
        },
        ProviderPreset {
            name: "deepseek",
            display_name: "DeepSeek",
            base_url: "https://api.deepseek.com/v1",
            default_model: "deepseek-chat",
            vendor: ModelVendor::DeepSeek,
        },
        ProviderPreset {
            name: "qwen",
            display_name: "Qwen (DashScope)",
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
            default_model: "qwen-plus",
            vendor: ModelVendor::Qwen,
        },
        ProviderPreset {
            name: "ollama",
            display_name: "Ollama (Local)",
            base_url: "http://localhost:11434/v1",
            default_model: "llama3",
            vendor: ModelVendor::Other,
        },
        ProviderPreset {
            name: "lmstudio",
            display_name: "LM Studio (Local)",
            base_url: "http://localhost:1234/v1",
            default_model: "local-model",
            vendor: ModelVendor::Other,
        },
    ]
}

/// Find preset by name.
pub fn find_preset(name: &str) -> Option<ProviderPreset> {
    get_provider_presets().into_iter().find(|p| p.name == name)
}
