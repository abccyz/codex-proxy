---
name: toml-config-editor
description: Safely edit TOML config files by splitting header/sections to avoid duplicate key insertion when replacing top-level keys
source: auto-skill
extracted_at: '2026-07-10T12:25:02.539Z'
---

# TOML Config Editor

## Problem

When using regex to replace top-level keys in a TOML config file (e.g., `model = "xxx"`, `model_provider = "xxx"`), a naive global regex match like `^model\s*=.*$` will also match:
- Keys inside section headers (e.g., `[model_providers.xxx]`)
- Keys that happen to start with the same prefix

This causes **duplicate entries** instead of replacement:

```toml
# Before apply_model()
model_provider = "Old"
model = "old-model"

[model_providers.Old]
name = "Old"

# After naive regex replace (WRONG - duplicates!)
model_provider = "New"
model = "new-model"
model_provider = "New"
model = "new-model"

[model_providers.Old]
name = "New"  # This also got replaced!
```

## Solution

**Split the TOML into header (before first `[`) and body (from first `[` onwards). Remove all existing matching key lines from the header first, then build a new header with the correct values prepended.**

### Why "remove-then-prepend" is better than "regex-replace"

The simpler approach (regex-replace in header, prepend if no match) has a subtle bug:
- If the regex fails to match (e.g., whitespace differences, encoding issues), it prepends a new line
- On repeated calls, this accumulates duplicates
- The "remove-then-prepend" approach is **idempotent** — calling it N times produces the same result

### Rust Implementation (Remove-Then-Prepend)

```rust
fn update_toml_top_level_keys(content: &str, keys: &[(&str, &str)]) -> String {
    // Split at first section header
    let first_section_pos = content.find('[');
    let (header, body) = match first_section_pos {
        Some(pos) => (content[..pos].to_string(), content[pos..].to_string()),
        None => (content.to_string(), String::new()),
    };

    // Remove ALL existing lines matching any of the keys
    let cleaned_header: String = header
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !keys.iter().any(|(key, _)| trimmed.starts_with(key))
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Build new header: new key-value pairs first, then remaining lines
    let mut new_header_parts: Vec<String> = Vec::new();
    for (key, value) in keys {
        new_header_parts.push(format!(r#"{} = "{}""#, key, value));
    }
    for line in cleaned_header.lines() {
        if !line.trim().is_empty() {
            new_header_parts.push(line.to_string());
        }
    }

    let new_header = new_header_parts.join("\n");
    if body.is_empty() {
        new_header
    } else {
        format!("{}\n{}", new_header, body)
    }
}
```

### Key Rules

1. **Always split at first `[`** — everything before is top-level config, everything after is section-based
2. **Remove all existing matching key lines from header first** — prevents duplicates from accumulating
3. **Prepend new values at the top** — ensures correct ordering
4. **Section-level updates** (e.g., `[model_providers.xxx]`) use a separate regex that targets the full section block
5. **This approach is idempotent** — calling it multiple times produces the same result

### When This Applies

- Editing `config.toml`, `Cargo.toml`, or any TOML-based config
- When top-level keys share names or prefixes with section-level keys
- When using regex-based text replacement (not a TOML parser)
- When the same config write may be called multiple times (e.g., on app restart + user action)

### When This Doesn't Apply

- If you're using a proper TOML parser library (e.g., `toml` crate with serde)
- For adding new sections (use section-level regex or append)
- For simple configs with no sections

## Example: Full Config Update for Codex

```rust
pub fn apply_model(&self, model_name: &str, provider: &str, base_url: &str) -> bool {
    let content = self.read();
    let mut new_content = if content.is_empty() {
        self.default_config()
    } else {
        content.clone()
    };

    // ── Step 1: Update top-level keys (header only, remove-then-prepend) ──
    let first_section_pos = new_content.find('[');
    let (header, body) = match first_section_pos {
        Some(pos) => (new_content[..pos].to_string(), new_content[pos..].to_string()),
        None => (new_content.clone(), String::new()),
    };

    // Remove all existing model_provider and model lines
    let cleaned_header: String = header
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("model_provider") && !trimmed.starts_with("model ")
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Build new header with correct values at top
    let mut new_header_parts = Vec::new();
    if !model_name.is_empty() {
        new_header_parts.push(format!(r#"model = "{}""#, model_name));
    }
    new_header_parts.push(format!(r#"model_provider = "{}""#, provider));

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

    // ── Step 2: Update or add provider section ──
    let env_key = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));
    let provider_section = format!(
        r#"
[model_providers.{}]
name = "{}"
base_url = "{}"
env_key = "{}"
wire_api = "responses"
"#,
        provider, provider, base_url, env_key
    );

    let section_re = Regex::new(&format!(
        r"(?s)\[model_providers\.{}\].*?(\n\[|\z)",
        regex::escape(&provider)
    )).unwrap();

    if section_re.is_match(&new_content) {
        let replacement = format!("{}\n$1", provider_section.trim());
        new_content = section_re.replace(&new_content, replacement).to_string();
    } else {
        new_content.push('\n');
        new_content.push_str(&provider_section);
    }

    self.write(&new_content)
}
```

## Codex-Specific Config Notes

- **`wire_api` must be `"responses"`** — Codex no longer supports `"chat"` (as of 2026)
- **`env_key` must be a real env var name** like `MODEL_STUDIO_API_KEY`, NOT `"PATH"` or any system variable
- **`base_url`** should point to the proxy (`http://127.0.0.1:8000/v1`) when using a proxy, or directly to the upstream service

## Testing

Verify no duplicates after edit:

```bash
# Should output 1
grep -c "^model_provider" ~/.codex/config.toml

# Should output 1
grep "^model " ~/.codex/config.toml | wc -l
```

## Related Issues

- Regex lookahead not supported in Rust's `regex` crate — use capture groups instead
- TOML is not a regular language — regex-based editing has limits; use a parser for complex cases
- The "replace-if-match, else-prepend" approach is NOT idempotent — prefer "remove-all-then-prepend"
