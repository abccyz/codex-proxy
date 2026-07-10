use fernet::Fernet;
use parking_lot::Mutex;
use regex::Regex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

// ── Saved Config ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConfig {
    pub id: i64,
    pub name: String,
    pub model: String,
    pub provider: String,
    pub base_url: String,
    pub api_key_masked: String,
    pub created_at: f64,
    pub updated_at: f64,
}

#[derive(Debug, Clone)]
pub struct SavedConfigFull {
    pub model: String,
    #[allow(dead_code)]
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
}

// ── Secure Config Store ─────────────────────────────────────

pub struct SecureConfigStore {
    #[allow(dead_code)]
    db_path: PathBuf,
    lock: Mutex<Connection>,  // Cache a single connection with mutex protection
    fernet: Fernet,
}

impl SecureConfigStore {
    pub fn new(db_path: PathBuf, key_file: PathBuf) -> Self {
        let fernet = init_encryption(&key_file);
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        
        // Open and initialize database once
        let conn = Connection::open(&db_path).expect("Failed to open config DB");
        Self::init_db_schema(&conn);
        
        let store = Self {
            db_path,
            lock: Mutex::new(conn),
            fernet,
        };
        store
    }

    fn init_db_schema(conn: &Connection) {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS saved_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                model TEXT NOT NULL,
                provider TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_key_encrypted TEXT NOT NULL,
                created_at REAL NOT NULL,
                updated_at REAL NOT NULL
            )",
            [],
        )
        .expect("Failed to create saved_configs table");
    }

    fn encrypt(&self, plaintext: &str) -> String {
        self.fernet.encrypt(plaintext.as_bytes())
    }

    fn decrypt(&self, ciphertext: &str) -> String {
        let bytes = match self.fernet.decrypt(ciphertext) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to decrypt config value: {}", e);
                return String::new();
            }
        };
        String::from_utf8(bytes).unwrap_or_default()
    }

    pub fn save_config(
        &self,
        name: &str,
        model: &str,
        provider: &str,
        base_url: &str,
        api_key: &str,
    ) -> bool {
        let now = chrono::Utc::now().timestamp() as f64;
        let encrypted = self.encrypt(api_key);
        let conn = self.lock.lock();
        conn.execute(
            "INSERT INTO saved_configs (name, model, provider, base_url, api_key_encrypted, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(name) DO UPDATE SET
                model=excluded.model,
                provider=excluded.provider,
                base_url=excluded.base_url,
                api_key_encrypted=excluded.api_key_encrypted,
                updated_at=excluded.updated_at",
            rusqlite::params![name, model, provider, base_url, encrypted, now, now],
        )
        .is_ok()
    }

    pub fn get_config_full(&self, name: &str) -> Option<SavedConfigFull> {
        let conn = self.lock.lock();
        let mut stmt = conn
            .prepare("SELECT model, provider, base_url, api_key_encrypted FROM saved_configs WHERE name = ?1")
            .ok()?;
        let result = stmt.query_row(rusqlite::params![name], |row| {
            Ok(SavedConfigFull {
                model: row.get(0)?,
                provider: row.get(1)?,
                base_url: row.get(2)?,
                api_key: row.get(3)?,
            })
        });
        match result {
            Ok(cfg) => {
                let decrypted = self.decrypt(&cfg.api_key);
                Some(SavedConfigFull {
                    api_key: decrypted,
                    ..cfg
                })
            }
            Err(_) => None,
        }
    }

    pub fn list_configs(&self) -> Vec<SavedConfig> {
        let conn_guard = self.lock.lock();
        let conn = &*conn_guard;
        let mut stmt = match conn
            .prepare("SELECT id, name, model, provider, base_url, api_key_encrypted, created_at, updated_at FROM saved_configs ORDER BY updated_at DESC")
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to prepare list query: {}", e);
                return Vec::new();
            }
        };
        let rows = match stmt
            .query_map([], |row| {
                let encrypted: String = row.get(5)?;
                let masked = if encrypted.len() > 12 {
                    format!(
                        "{}...{}",
                        &encrypted[..8],
                        &encrypted[encrypted.len() - 4..]
                    )
                } else {
                    "***".to_string()
                };
                Ok(SavedConfig {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    model: row.get(2)?,
                    provider: row.get(3)?,
                    base_url: row.get(4)?,
                    api_key_masked: masked,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn delete_config(&self, name: &str) -> bool {
        let conn_guard = self.lock.lock();
        let conn = &*conn_guard;
        conn.execute("DELETE FROM saved_configs WHERE name = ?1", rusqlite::params![name])
            .is_ok()
    }
}

fn init_encryption(key_file: &PathBuf) -> Fernet {
    if key_file.exists() {
        let key = fs::read(key_file).expect("Failed to read encryption key");
        Fernet::new(&String::from_utf8(key).expect("Invalid key encoding"))
            .expect("Invalid Fernet key")
    } else {
        let key = Fernet::generate_key();
        if let Some(parent) = key_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(key_file, &key).expect("Failed to write encryption key");
        Fernet::new(&key).expect("Failed to create Fernet from new key")
    }
}

// ── Config Manager ──────────────────────────────────────────

pub struct ConfigManager {
    config_path: PathBuf,
    lock: Mutex<()>,
}

impl ConfigManager {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            lock: Mutex::new(()),
        }
    }

    #[allow(dead_code)]
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn config_path_display(&self) -> String {
        // Show ~ on macOS/Linux, %APPDATA% on Windows
        if let Some(home) = dirs::home_dir() {
            if let Ok(stripped) = self.config_path.strip_prefix(home) {
                if cfg!(windows) {
                    return format!("%USERPROFILE%\\{}", stripped.display().to_string().replace('/', "\\"));
                } else {
                    return format!("~/{}", stripped.display());
                }
            }
        }
        self.config_path.display().to_string()
    }

    pub fn read(&self) -> String {
        let _guard = self.lock.lock();
        fs::read_to_string(&self.config_path).unwrap_or_default()
    }

    pub fn write(&self, content: &str) -> bool {
        let _guard = self.lock.lock();
        if let Some(parent) = self.config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&self.config_path, content).is_ok()
    }

    pub fn get_current_model(&self) -> CurrentConfig {
        let content = self.read();
        let model_re = Regex::new(r#"^model\s*=\s*"([^"]+)""#).unwrap();
        let provider_re = Regex::new(r#"^model_provider\s*=\s*"([^"]+)""#).unwrap();

        let model = model_re
            .captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let provider = provider_re
            .captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        // Read base_url from the active provider's section, not just the first base_url in file
        let base_url = if !provider.is_empty() {
            let section_re = Regex::new(&format!(
                r#"(?s)\[model_providers\.{}\][^\[]*base_url\s*=\s*"([^"]+)""#,
                regex::escape(&provider)
            )).ok();
            section_re
                .and_then(|re| re.captures(&content))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

        CurrentConfig { model, provider, base_url }
    }

    pub fn apply_model(
        &self,
        model_name: &str,
        provider: &str,
        _base_url: &str,
        api_key: &str,
    ) -> bool {
        let _api_key = api_key; // API key stored in secure_store, not in TOML
        let content = self.read();
        let reserved: HashSet<&str> = ["openai", "ollama", "lmstudio"]
            .iter()
            .copied()
            .collect();

        let provider = if reserved.contains(provider.to_lowercase().as_str()) {
            format!("{}-custom", provider)
        } else {
            provider.to_string()
        };

        let mut new_content = if content.is_empty() {
            self.default_config()
        } else {
            content.clone()
        };

        // FIX: Ensure model and model_provider appear exactly once at the top
        // Strategy: Remove all existing top-level model/model_provider lines first,
        // then prepend the correct values

        // Split into header (before first [) and body (from first [ onwards)
        let first_section_pos = new_content.find('[');
        let (header, body) = match first_section_pos {
            Some(pos) => (new_content[..pos].to_string(), new_content[pos..].to_string()),
            None => (new_content.clone(), String::new()),
        };

        // Remove all existing model and model_provider lines from header
        let cleaned_header: String = header
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with("model_provider") && !trimmed.starts_with("model ")
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Build new header with model and model_provider at the top
        let mut new_header_parts = Vec::new();
        if !model_name.is_empty() {
            new_header_parts.push(format!(r#"model = "{}""#, model_name));
        }
        new_header_parts.push(format!(r#"model_provider = "{}""#, provider));
        
        // Add back any other lines from cleaned header (non-empty)
        for line in cleaned_header.lines() {
            if !line.trim().is_empty() {
                new_header_parts.push(line.to_string());
            }
        }

        let new_header = new_header_parts.join("\n");
        new_content = if body.is_empty() {
            new_header
        } else {
            format!("{}\n{}", new_header, body)
        };

        // Update or add provider section
        // FIX: Use PATH as env_key (always exists, Codex check will pass)
        // The actual API key is handled by the proxy, not by Codex directly
        let provider_section = format!(
            r#"
[model_providers.{}]
name = "{}"
base_url = "{}"
env_key = "PATH"
wire_api = "responses"
"#,
            provider, provider, _base_url
        );

        // Use capture group instead of lookahead (Rust regex doesn't support lookahead)
        let section_re = Regex::new(&format!(
            r"(?s)\[model_providers\.{}\].*?(\n\[|\z)",
            regex::escape(&provider)
        ))
        .unwrap();

        if section_re.is_match(&new_content) {
            let replacement = format!("{}\n$1", provider_section.trim());
            new_content = section_re
                .replace(&new_content, replacement)
                .to_string();
        } else {
            new_content.push('\n');
            new_content.push_str(&provider_section);
        }

        self.write(&new_content)
    }

    fn default_config(&self) -> String {
        r#"model_provider = "Default"
model = "qwen-plus"

[model_providers.Default]
name = "Default"
base_url = "http://127.0.0.1:8000/v1"
env_key = "PATH"
wire_api = "responses"
request_max_retries = 4
stream_max_retries = 5
stream_idle_timeout_ms = 300000

[features]
memories = true
"#
        .to_string()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrentConfig {
    pub model: String,
    pub provider: String,
    pub base_url: String,
}
