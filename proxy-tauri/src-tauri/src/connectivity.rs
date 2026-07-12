use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectivityResult {
    pub success: bool,
    pub models: Vec<String>,
    pub error_message: Option<String>,
    pub latency_ms: u64,
}

pub async fn test_connectivity(client: &Client, base_url: &str, api_key: &str) -> ConnectivityResult {
    let start = std::time::Instant::now();
    let models_url = if base_url.ends_with('/') { format!("{}models", base_url) } else { format!("{}/models", base_url) };
    let request = client.get(&models_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(10));
    match request.send().await {
        Ok(response) => {
            let latency = start.elapsed().as_millis() as u64;
            if response.status().is_success() {
                match response.json::<Value>().await {
                    Ok(json) => {
                        let models = extract_models_from_response(&json);
                        ConnectivityResult { success: true, models, error_message: None, latency_ms: latency }
                    }
                    Err(e) => ConnectivityResult { success: false, models: vec![], error_message: Some(format!("Failed to parse response: {}", e)), latency_ms: latency },
                }
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                ConnectivityResult { success: false, models: vec![], error_message: Some(format!("HTTP {}: {}", status, error_text)), latency_ms: latency }
            }
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            ConnectivityResult { success: false, models: vec![], error_message: Some(format!("Connection failed: {}", e)), latency_ms: latency }
        }
    }
}

fn extract_models_from_response(json: &Value) -> Vec<String> {
    let mut models = vec![];
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        for item in data {
            if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            }
        }
    }
    models.sort();
    models.truncate(20);
    models
}
