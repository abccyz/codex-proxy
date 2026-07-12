/// Model vendor detection and capability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModelVendor {
    DeepSeek, Qwen, OpenAI, Other,
}

impl ModelVendor {
    pub fn from_model_name(model: &str) -> Self {
        let lower = model.to_lowercase();
        if lower.contains("deepseek") { Self::DeepSeek }
        else if lower.contains("qwen") || lower.contains("dashscope") { Self::Qwen }
        else if lower.contains("gpt") || lower.contains("openai") { Self::OpenAI }
        else { Self::Other }
    }

    pub fn requires_reasoning_content(&self) -> bool {
        matches!(self, Self::DeepSeek | Self::Qwen)
    }

    pub fn build_reasoning_output(&self, reasoning_content: &str) -> Option<String> {
        match self {
            Self::DeepSeek => {
                if !reasoning_content.is_empty() { Some(reasoning_content.to_string()) } else { None }
            }
            Self::Qwen => Some(reasoning_content.to_string()),
            Self::OpenAI | Self::Other => None,
        }
    }

    pub fn build_reasoning_input(&self, reasoning_content: Option<&str>) -> Option<String> {
        match self {
            Self::DeepSeek | Self::Qwen => Some(reasoning_content.unwrap_or("").to_string()),
            Self::OpenAI | Self::Other => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderPreset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub vendor: ModelVendor,
}

pub fn get_provider_presets() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset { name: "openai", display_name: "OpenAI", base_url: "https://api.openai.com/v1", default_model: "gpt-4o", vendor: ModelVendor::OpenAI },
        ProviderPreset { name: "deepseek", display_name: "DeepSeek", base_url: "https://api.deepseek.com/v1", default_model: "deepseek-chat", vendor: ModelVendor::DeepSeek },
        ProviderPreset { name: "qwen", display_name: "Qwen (DashScope)", base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1", default_model: "qwen-plus", vendor: ModelVendor::Qwen },
        ProviderPreset { name: "ollama", display_name: "Ollama (Local)", base_url: "http://localhost:11434/v1", default_model: "llama3", vendor: ModelVendor::Other },
        ProviderPreset { name: "lmstudio", display_name: "LM Studio (Local)", base_url: "http://localhost:1234/v1", default_model: "local-model", vendor: ModelVendor::Other },
    ]
}

pub fn find_preset(name: &str) -> Option<ProviderPreset> {
    get_provider_presets().into_iter().find(|p| p.name == name)
}
