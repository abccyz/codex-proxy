use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// ── Frontend-facing types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub providers: Vec<CatalogProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogProvider {
    pub id: String,
    pub name: String,
    pub api: Option<String>,
    pub model_count: usize,
    pub npm: Option<String>,
    pub models: Vec<CatalogModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub family: Option<String>,
    pub tool_call: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub attachment: bool,
    pub context: Option<i64>,
    pub output: Option<i64>,
    pub release_date: Option<String>,
    pub open_weights: Option<bool>,
    pub cost_input: Option<f64>,
    pub cost_output: Option<f64>,
}

// ── Raw models.dev JSON types ──

#[derive(Debug, Deserialize)]
struct RawCatalog {
    providers: Option<HashMap<String, RawProvider>>,
    models: HashMap<String, RawModel>,
}

#[derive(Debug, Deserialize)]
struct RawProvider {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    api: Option<String>,
    #[serde(default)]
    npm: Option<String>,
    #[serde(default)]
    models: Option<HashMap<String, RawProviderModel>>,
}

#[derive(Debug, Deserialize)]
struct RawProviderModel {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    tool_call: bool,
    #[serde(default)]
    reasoning: bool,
    #[serde(default)]
    attachment: bool,
    limit: Option<RawModelLimit>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    open_weights: Option<bool>,
    #[serde(default)]
    cost: Option<RawModelCost>,
}

#[derive(Debug, Deserialize)]
struct RawModel {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    tool_call: bool,
    #[serde(default)]
    reasoning: bool,
    #[serde(default)]
    attachment: bool,
    limit: Option<RawModelLimit>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    open_weights: Option<bool>,
    #[serde(default)]
    cost: Option<RawModelCost>,
}

#[derive(Debug, Deserialize)]
struct RawModelLimit {
    context: Option<i64>,
    output: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawModelCost {
    input: Option<f64>,
    output: Option<f64>,
}

// ── Known base URLs for native providers (no `api` field in catalog) ──

const KNOWN_NATIVE_URLS: &[(&str, &str)] = &[
    ("openai", "https://api.openai.com/v1"),
    ("anthropic", "https://api.anthropic.com/v1"),
    ("google", "https://generativelanguage.googleapis.com/v1beta"),
    ("xai", "https://api.x.ai/v1"),
    ("cohere", "https://api.cohere.com/v1"),
    ("mistral", "https://api.mistral.ai/v1"),
    ("perplexity", "https://api.perplexity.com"),
    ("meta", "https://api.meta.ai/v1"),
];

fn known_native_url(provider_id: &str) -> Option<&'static str> {
    KNOWN_NATIVE_URLS
        .iter()
        .find(|(id, _)| *id == provider_id)
        .map(|(_, url)| *url)
}

fn get_catalog_cache_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".codex").join("proxy").join("model_catalog.json")
}

// ── Download & cache ──

pub async fn refresh_catalog() -> Result<ModelCatalog, String> {
    let url = "https://models.dev/catalog.json";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .get(url)
        .header("User-Agent", "codex-proxy/0.1")
        .send()
        .await
        .map_err(|e| format!("Failed to download catalog: {e}"))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    let cache_path = get_catalog_cache_path();
    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&cache_path, &text);

    parse_catalog(&text)
}

fn parse_catalog(json: &str) -> Result<ModelCatalog, String> {
    let raw: RawCatalog =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse catalog: {e}"))?;

    let mut providers: Vec<CatalogProvider> = Vec::new();

    // Iterate over all providers
    if let Some(raw_providers) = &raw.providers {
        for (prov_id, raw_prov) in raw_providers {
            // Determine API endpoint
            let api = {
                if let Some(api) = &raw_prov.api {
                    if !api.is_empty() {
                        Some(api.clone())
                    } else {
                        known_native_url(prov_id).map(|s| s.to_string())
                    }
                } else {
                    known_native_url(prov_id).map(|s| s.to_string())
                }
            };

            // Only include if we have a usable API endpoint
            let api = match api {
                Some(a) => a,
                None => continue,
            };

            // Build model list for this provider
            let mut models: Vec<CatalogModel> = Vec::new();

            // Option 1: Use provider's own models field if available
            if let Some(prov_models) = &raw_prov.models {
                for (short_id, prov_model) in prov_models {
                    let full_id = format!("{}/{}", prov_id, short_id);
                    models.push(CatalogModel {
                        id: full_id,
                        name: prov_model.name.clone(),
                        description: prov_model.description.clone(),
                        family: prov_model.family.clone(),
                        tool_call: prov_model.tool_call,
                        reasoning: prov_model.reasoning,
                        attachment: prov_model.attachment,
                        context: prov_model.limit.as_ref().and_then(|l| l.context),
                        output: prov_model.limit.as_ref().and_then(|l| l.output),
                        release_date: prov_model.release_date.clone(),
                        open_weights: prov_model.open_weights,
                        cost_input: prov_model.cost.as_ref().and_then(|c| c.input),
                        cost_output: prov_model.cost.as_ref().and_then(|c| c.output),
                    });
                }
            } else {
                // Option 2: Use global models with matching prefix
                for (model_id, raw_model) in &raw.models {
                    if let Some((prefix, _)) = model_id.split_once('/') {
                        if prefix == prov_id {
                            models.push(CatalogModel {
                                id: model_id.clone(),
                                name: raw_model.name.clone(),
                                description: raw_model.description.clone(),
                                family: raw_model.family.clone(),
                                tool_call: raw_model.tool_call,
                                reasoning: raw_model.reasoning,
                                attachment: raw_model.attachment,
                                context: raw_model.limit.as_ref().and_then(|l| l.context),
                                output: raw_model.limit.as_ref().and_then(|l| l.output),
                                release_date: raw_model.release_date.clone(),
                                open_weights: raw_model.open_weights,
                                cost_input: raw_model.cost.as_ref().and_then(|c| c.input),
                                cost_output: raw_model.cost.as_ref().and_then(|c| c.output),
                            });
                        }
                    }
                }
            }

            // Skip providers with no models
            if models.is_empty() {
                continue;
            }

            // Sort models: tool_call first, then by context desc
            models.sort_by(|a, b| {
                b.tool_call.cmp(&a.tool_call)
                    .then(b.context.unwrap_or(0).cmp(&a.context.unwrap_or(0)))
            });

            providers.push(CatalogProvider {
                id: prov_id.clone(),
                name: raw_prov.name.clone(),
                api: Some(api),
                model_count: models.len(),
                npm: raw_prov.npm.clone(),
                models,
            });
        }
    }

    // Sort: providers with most models first
    providers.sort_by(|a, b| b.model_count.cmp(&a.model_count));

    Ok(ModelCatalog { providers })
}

// ── Read cached catalog ──

pub fn get_cached_catalog() -> Option<ModelCatalog> {
    let path = get_catalog_cache_path();
    if !path.exists() {
        return None;
    }
    let json = fs::read_to_string(&path).ok()?;
    parse_catalog(&json).ok()
}
