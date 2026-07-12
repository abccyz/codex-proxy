//! Connectivity testing module for validating upstream API connections.

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

/// Result of a connectivity test.
#[derive(Debug, Clone)]
pub struct ConnectivityResult {
    pub success: bool,
    pub models: Vec<String>,
    pub error_message: Option<String>,
    pub latency_ms: u64,
}

/// Test connectivity to an upstream API endpoint.
/// Sends GET /models and returns available models or error.
pub async fn test_connectivity(
    client: &Client,
    base_url: &str,
    api_key: &str,
) -> ConnectivityResult {
    let start = std::time::Instant::now();
    
    // Build URL: ensure it ends with /models
    let models_url = if base_url.ends_with('/') {
        format!("{}models", base_url)
    } else {
        format!("{}/models", base_url)
    };
    
    // Build request with timeout
    let request = client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(10));
    
    match request.send().await {
        Ok(response) => {
            let latency = start.elapsed().as_millis() as u64;
            
            if response.status().is_success() {
                match response.json::<Value>().await {
                    Ok(json) => {
                        // Extract model list from response
                        let models = extract_models_from_response(&json);
                        ConnectivityResult {
                            success: true,
                            models,
                            error_message: None,
                            latency_ms: latency,
                        }
                    }
                    Err(e) => ConnectivityResult {
                        success: false,
                        models: vec![],
                        error_message: Some(format!("Failed to parse response: {}", e)),
                        latency_ms: latency,
                    },
                }
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                ConnectivityResult {
                    success: false,
                    models: vec![],
                    error_message: Some(format!("HTTP {}: {}", status, error_text)),
                    latency_ms: latency,
                }
            }
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            ConnectivityResult {
                success: false,
                models: vec![],
                error_message: Some(format!("Connection failed: {}", e)),
                latency_ms: latency,
            }
        }
    }
}

/// Extract model names from OpenAI-compatible /models response.
fn extract_models_from_response(json: &Value) -> Vec<String> {
    let mut models = vec![];
    
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        for item in data {
            if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            }
        }
    }
    
    // Sort and limit to first 20 for display
    models.sort();
    models.truncate(20);
    models
}
