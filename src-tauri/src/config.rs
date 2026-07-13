use fernet::Fernet;
use parking_lot::Mutex;
use regex::Regex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

#[derive(Debug, Clone, serde::Serialize)]
pub struct SavedConfigFull {
    pub model: String,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
}

pub struct SecureConfigStore {
    db_path: PathBuf,
    lock: Mutex<Connection>,
    fernet: Fernet,
}

fn init_encryption(key_file: &Path) -> Fernet {
    if key_file.exists() {
        let key = fs::read(key_file).expect("Failed to read encryption key");
        Fernet::new(&String::from_utf8(key).expect("Invalid key file")).expect("Invalid Fernet key")
    } else {
        let key = Fernet::generate_key();
        if let Some(parent) = key_file.parent() { let _ = fs::create_dir_all(parent); }
        fs::write(key_file, &key).expect("Failed to write encryption key");
        #[cfg(unix)] { let _ = std::fs::set_permissions(key_file, std::fs::Permissions::from_mode(0o600)); }
        Fernet::new(&key).expect("Invalid Fernet key")
    }
}

impl SecureConfigStore {
    pub fn new(db_path: PathBuf, key_file: PathBuf) -> Self {
        let fernet = init_encryption(&key_file);
        if let Some(parent) = db_path.parent() { let _ = fs::create_dir_all(parent); }
        let conn = Connection::open(&db_path).expect("Failed to open config DB");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS saved_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL, model TEXT NOT NULL,
                provider TEXT NOT NULL, base_url TEXT NOT NULL, api_key_encrypted TEXT NOT NULL,
                created_at REAL NOT NULL, updated_at REAL NOT NULL
            )", [],
        ).expect("Failed to create saved_configs table");
        Self { db_path, lock: Mutex::new(conn), fernet }
    }

    fn encrypt(&self, plaintext: &str) -> String { self.fernet.encrypt(plaintext.as_bytes()) }
    fn decrypt(&self, ciphertext: &str) -> String {
        self.fernet.decrypt(ciphertext).map(|b| String::from_utf8(b).unwrap_or_default()).unwrap_or_default()
    }

    pub fn save_config(&self, name: &str, model: &str, provider: &str, base_url: &str, api_key: &str) -> bool {
        let now = chrono::Utc::now().timestamp() as f64;
        let encrypted = self.encrypt(api_key);
        let conn = self.lock.lock();
        conn.execute(
            "INSERT INTO saved_configs (name, model, provider, base_url, api_key_encrypted, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(name) DO UPDATE SET model=excluded.model, provider=excluded.provider,
             base_url=excluded.base_url, api_key_encrypted=excluded.api_key_encrypted, updated_at=excluded.updated_at",
            rusqlite::params![name, model, provider, base_url, encrypted, now, now],
        ).is_ok()
    }

    pub fn get_config_full(&self, name: &str) -> Option<SavedConfigFull> {
        let conn = self.lock.lock();
        let mut stmt = conn.prepare("SELECT model, provider, base_url, api_key_encrypted FROM saved_configs WHERE name = ?1").ok()?;
        let result = stmt.query_row(rusqlite::params![name], |row| Ok(SavedConfigFull {
            model: row.get(0)?, provider: row.get(1)?, base_url: row.get(2)?, api_key: row.get(3)?,
        }));
        result.ok().map(|cfg| SavedConfigFull { api_key: self.decrypt(&cfg.api_key), ..cfg })
    }

    pub fn list_configs(&self) -> Vec<SavedConfig> {
        let conn = self.lock.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, name, model, provider, base_url, api_key_encrypted, created_at, updated_at FROM saved_configs ORDER BY updated_at DESC"
        ) { Ok(s) => s, Err(_) => return vec![] };
        let rows = stmt.query_map([], |row| {
            let encrypted: String = row.get(5)?;
            let decrypted = self.decrypt(&encrypted);
            let masked = if decrypted.len() > 12 { format!("{}...{}", &decrypted[..8], &decrypted[decrypted.len()-4..]) } else { "***".to_string() };
            Ok(SavedConfig {
                id: row.get(0)?, name: row.get(1)?, model: row.get(2)?, provider: row.get(3)?,
                base_url: row.get(4)?, api_key_masked: masked, created_at: row.get(6)?, updated_at: row.get(7)?,
            })
        }).ok();
        match rows {
            Some(iter) => iter.filter_map(|r| r.ok()).collect(),
            None => vec![]
        }
    }

    pub fn delete_config(&self, name: &str) -> bool {
        let conn = self.lock.lock();
        conn.execute("DELETE FROM saved_configs WHERE name = ?1", rusqlite::params![name]).is_ok()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrentConfig {
    pub model: String,
    pub provider: String,
    pub base_url: String,
}

pub struct ConfigManager {
    config_path: PathBuf,
    lock: Mutex<()>,
}

impl ConfigManager {
    pub fn new(config_path: PathBuf) -> Self { Self { config_path, lock: Mutex::new(()) } }

    fn read(&self) -> String {
        if !self.config_path.exists() { return String::new(); }
        fs::read_to_string(&self.config_path).unwrap_or_default()
    }

    fn write(&self, content: &str) -> bool {
        let _guard = self.lock.lock();
        if let Some(parent) = self.config_path.parent() { let _ = fs::create_dir_all(parent); }
        fs::write(&self.config_path, content).is_ok()
    }

    pub fn get_current_model(&self) -> CurrentConfig {
        let content = self.read();
        let model_re = Regex::new(r#"model\s*=\s*"([^"]+)""#).unwrap();
        let provider_re = Regex::new(r#"model_provider\s*=\s*"([^"]+)""#).unwrap();
        let model = model_re.captures(&content).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default();
        let provider = provider_re.captures(&content).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default();
        let base_url = if !provider.is_empty() {
            let section_pattern = format!(
                r#"(?s)\[model_providers\.{}\].*?base_url\s*=\s*"([^"]+)""#,
                regex::escape(&provider)
            );
            Regex::new(&section_pattern)
                .ok().and_then(|re| re.captures(&content)).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default()
        } else { String::new() };
        CurrentConfig { model, provider, base_url }
    }

    pub fn config_path_display(&self) -> String {
        self.config_path.display().to_string()
    }

    pub fn apply_model(&self, model_name: &str, provider: &str, base_url: &str, _api_key: &str) -> bool {
        let content = self.read();
        let reserved: HashSet<&str> = ["openai", "ollama", "lmstudio"].iter().copied().collect();
        let provider_key = if reserved.contains(provider.to_lowercase().as_str()) {
            format!("{}-custom", provider)
        } else {
            provider.to_string()
        };
        let first_section_pos = content.find('[');
        let (header, body) = match first_section_pos {
            Some(pos) => {
                let h = &content[..pos];
                let b = &content[pos..];
                (h.to_string(), b.to_string())
            }
            None => {
                let h = content.clone();
                let b = String::new();
                (h, b)
            }
        };
        let cleaned_header: String = header.lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with("model_provider") && !trimmed.starts_with("model ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut new_header_parts: Vec<String> = Vec::new();
        if !model_name.is_empty() {
            new_header_parts.push(format!(r#"model = "{}""#, model_name));
        }
        new_header_parts.push(format!(r#"model_provider = "{}""#, provider_key));
        for line in cleaned_header.lines() {
            if !line.trim().is_empty() {
                new_header_parts.push(line.to_string());
            }
        }
        let new_header = new_header_parts.join("\n");
        let mut new_content = if body.is_empty() {
            new_header
        } else {
            format!("{}\n{}", new_header, body)
        };
        let provider_section = format!(
            "\n[model_providers.{}]\nname = \"{}\"\nbase_url = \"{}\"\nenv_key = \"PATH\"\nwire_api = \"responses\"\n",
            provider_key, provider_key, base_url
        );
        let section_pattern = format!(
            r"(?s)\[model_providers\.{}\].*?(\n\[|\z)",
            regex::escape(&provider_key)
        );
        let section_re = Regex::new(&section_pattern).unwrap();
        if section_re.is_match(&new_content) {
            let replacement = format!("{}\n$1", provider_section.trim());
            new_content = section_re.replace(&new_content, replacement.as_str()).to_string();
        } else {
            new_content.push('\n');
            new_content.push_str(&provider_section);
        }
        self.write(&new_content)
    }
}

impl Drop for SecureConfigStore {
    fn drop(&mut self) {
        let _ = self.lock.lock().execute("PRAGMA optimize", []);
    }
}
