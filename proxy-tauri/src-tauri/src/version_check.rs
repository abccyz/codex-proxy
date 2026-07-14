use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
}

const RELEASES_URL: &str = "https://github.com/abccyz/codex-proxy/releases";
const TAG_PREFIX: &str = "/abccyz/codex-proxy/releases/tag/";

pub async fn check_latest_release() -> Option<VersionInfo> {
    let current_version = get_app_version();

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    let response = client
        .get(RELEASES_URL)
        .header("User-Agent", "ProxyTauri-UpdateChecker/1.0")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let html = response.text().await.ok()?;

    let latest_version = extract_latest_tag(&html)?;
    let release_url = format!("{}/tag/v{}", RELEASES_URL, latest_version);

    let has_update = compare_versions(&latest_version, &current_version) > 0;

    eprintln!("[version_check] current={}, latest={}, has_update={}", current_version, latest_version, has_update);

    Some(VersionInfo {
        current_version,
        latest_version,
        has_update,
        release_url,
    })
}

fn extract_latest_tag(html: &str) -> Option<String> {
    let mut search_from = 0;

    loop {
        let pos = html[search_from..].find(TAG_PREFIX)?;
        let abs_pos = search_from + pos;
        let tag_start = abs_pos + TAG_PREFIX.len();

        let remaining = &html[tag_start..];
        let tag_end = remaining
            .find(|c: char| c == '"' || c == '\'' || c == '<')
            .unwrap_or(remaining.len());

        let tag = &remaining[..tag_end];
        let version = tag.strip_prefix('v').unwrap_or(tag);

        if !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Some(version.to_string());
        }

        search_from = tag_start + tag_end;
    }
}

fn get_app_version() -> String {
    let config = include_str!("../tauri.conf.json");
    let value: serde_json::Value = serde_json::from_str(config).unwrap_or_default();
    value.get("version")
        .and_then(|v| v.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string()
}

fn compare_versions(a: &str, b: &str) -> i32 {
    let a_parts: Vec<u64> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let b_parts: Vec<u64> = b.split('.').filter_map(|s| s.parse().ok()).collect();

    let max_len = a_parts.len().max(b_parts.len());

    for i in 0..max_len {
        let a_val = a_parts.get(i).copied().unwrap_or(0);
        let b_val = b_parts.get(i).copied().unwrap_or(0);

        if a_val > b_val {
            return 1;
        } else if a_val < b_val {
            return -1;
        }
    }

    0
}
