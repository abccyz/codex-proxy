#!/usr/bin/env python3
"""
Responses API -> Chat Completions API proxy with monitoring dashboard and config center.
Translates OpenAI Responses API requests to Chat Completions format for DashScope.
"""

import json
import time
import uuid
import threading
import os
import re
import sqlite3
import base64
from collections import deque
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.request import Request, urlopen
from urllib.error import URLError, HTTPError
from pathlib import Path
from cryptography.fernet import Fernet
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC


class ClientDisconnected(Exception):
    """Raised when the client disconnects during SSE streaming."""
    pass


HOST = "127.0.0.1"
PORT = 8000
MONITOR_PORT = 8001
UPSTREAM = "https://coding.dashscope.aliyuncs.com/v1/chat/completions"
API_KEY = "sk-sp-9166e1c03e8b4c75b54fa1740a042ba0"
UPSTREAM_MODEL = "qwen3-coder-plus"  # Default model for DashScope Coding Plan
MAX_HISTORY = 200
CODEX_CONFIG_PATH = Path.home() / ".codex" / "config.toml"
SECURE_DB_PATH = Path.home() / ".codex" / "proxy_config.db"
ENCRYPTION_KEY_FILE = Path.home() / ".codex" / ".proxy_key"


# ── Secure Config Store ────────────────────────────────────────

class SecureConfigStore:
    """SQLite-based config storage with Fernet encryption for sensitive data."""
    
    def __init__(self, db_path, key_file):
        self.db_path = Path(db_path)
        self.key_file = Path(key_file)
        self.lock = threading.Lock()
        self.fernet = self._init_encryption()
        self._init_db()
    
    def _init_encryption(self):
        """Initialize or load encryption key."""
        if self.key_file.exists():
            key = self.key_file.read_bytes()
        else:
            key = Fernet.generate_key()
            self.key_file.parent.mkdir(parents=True, exist_ok=True)
            self.key_file.write_bytes(key)
            # Restrict permissions on key file (Unix only)
            try:
                os.chmod(self.key_file, 0o600)
            except Exception:
                pass
        return Fernet(key)
    
    def _init_db(self):
        """Initialize SQLite database with schema."""
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        with self.lock:
            conn = sqlite3.connect(self.db_path)
            conn.execute("""
                CREATE TABLE IF NOT EXISTS saved_configs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT UNIQUE NOT NULL,
                    model TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    base_url TEXT NOT NULL,
                    api_key_encrypted TEXT NOT NULL,
                    created_at REAL NOT NULL,
                    updated_at REAL NOT NULL
                )
            """)
            conn.commit()
            conn.close()
    
    def encrypt(self, plaintext):
        """Encrypt a string."""
        return self.fernet.encrypt(plaintext.encode()).decode()
    
    def decrypt(self, ciphertext):
        """Decrypt a string."""
        return self.fernet.decrypt(ciphertext.encode()).decode()
    
    def save_config(self, name, model, provider, base_url, api_key):
        """Save a new config or update existing one."""
        now = time.time()
        encrypted_key = self.encrypt(api_key)
        with self.lock:
            conn = sqlite3.connect(self.db_path)
            try:
                conn.execute("""
                    INSERT INTO saved_configs (name, model, provider, base_url, api_key_encrypted, created_at, updated_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(name) DO UPDATE SET
                        model=excluded.model,
                        provider=excluded.provider,
                        base_url=excluded.base_url,
                        api_key_encrypted=excluded.api_key_encrypted,
                        updated_at=excluded.updated_at
                """, (name, model, provider, base_url, encrypted_key, now, now))
                conn.commit()
                return True
            except Exception as e:
                print(f"Error saving config: {e}")
                return False
            finally:
                conn.close()
    
    def get_config(self, name):
        """Retrieve a config by name, decrypting the API key."""
        with self.lock:
            conn = sqlite3.connect(self.db_path)
            conn.row_factory = sqlite3.Row
            cursor = conn.execute("SELECT * FROM saved_configs WHERE name = ?", (name,))
            row = cursor.fetchone()
            conn.close()
            if row:
                return {
                    "id": row["id"],
                    "name": row["name"],
                    "model": row["model"],
                    "provider": row["provider"],
                    "base_url": row["base_url"],
                    "api_key": self.decrypt(row["api_key_encrypted"]),
                    "created_at": row["created_at"],
                    "updated_at": row["updated_at"],
                }
            return None
    
    def list_configs(self):
        """List all saved configs with masked API keys."""
        with self.lock:
            conn = sqlite3.connect(self.db_path)
            conn.row_factory = sqlite3.Row
            cursor = conn.execute("SELECT * FROM saved_configs ORDER BY updated_at DESC")
            rows = cursor.fetchall()
            conn.close()
            configs = []
            for row in rows:
                try:
                    decrypted_key = self.decrypt(row["api_key_encrypted"])
                    masked_key = decrypted_key[:8] + "..." + decrypted_key[-4:] if len(decrypted_key) > 12 else "***"
                except Exception:
                    masked_key = "***"
                configs.append({
                    "id": row["id"],
                    "name": row["name"],
                    "model": row["model"],
                    "provider": row["provider"],
                    "base_url": row["base_url"],
                    "api_key_masked": masked_key,
                    "created_at": row["created_at"],
                    "updated_at": row["updated_at"],
                })
            return configs
    
    def delete_config(self, name):
        """Delete a saved config."""
        with self.lock:
            conn = sqlite3.connect(self.db_path)
            conn.execute("DELETE FROM saved_configs WHERE name = ?", (name,))
            conn.commit()
            conn.close()
            return True


secure_store = SecureConfigStore(SECURE_DB_PATH, ENCRYPTION_KEY_FILE)


# ── Config Manager ─────────────────────────────────────────────

class ConfigManager:
    def __init__(self, config_path):
        self.config_path = Path(config_path)
        self.lock = threading.Lock()

    def read(self):
        with self.lock:
            if not self.config_path.exists():
                return ""
            return self.config_path.read_text(encoding="utf-8")

    def write(self, content):
        with self.lock:
            self.config_path.parent.mkdir(parents=True, exist_ok=True)
            self.config_path.write_text(content, encoding="utf-8")
            return True

    def get_current_model(self):
        """Extract current model and provider from config"""
        content = self.read()
        model_match = re.search(r'^model\s*=\s*"([^"]+)"', content, re.MULTILINE)
        provider_match = re.search(r'^model_provider\s*=\s*"([^"]+)"', content, re.MULTILINE)
        base_url_match = re.search(r'base_url\s*=\s*"([^"]+)"', content)
        return {
            "model": model_match.group(1) if model_match else "",
            "provider": provider_match.group(1) if provider_match else "",
            "base_url": base_url_match.group(1) if base_url_match else "",
        }

    # Codex reserved built-in provider IDs that cannot be overridden
    # According to official docs: openai, ollama, lmstudio
    RESERVED_PROVIDERS = {"openai", "ollama", "lmstudio"}

    def apply_model(self, model_name, provider, base_url, api_key):
        """Apply a model configuration to config.toml"""
        content = self.read()
        if not content:
            content = self._generate_default_config()

        # Avoid reserved built-in provider IDs
        if provider.lower() in self.RESERVED_PROVIDERS:
            provider = f"{provider}-custom"

        # Update model
        content = re.sub(
            r'^model\s*=.*$',
            f'model = "{model_name}"',
            content,
            flags=re.MULTILINE,
            count=1
        ) if re.search(r'^model\s*=', content, re.MULTILINE) else f'model = "{model_name}"\n' + content

        # Update or add model_provider section
        # base_url in config.toml must always point to local proxy,
        # the proxy handles actual upstream routing internally
        provider_section = f"""
[model_providers.{provider}]
name = "{provider}"
base_url = "http://127.0.0.1:8000/v1"
env_key = "PATH"
wire_api = "responses"
"""
        # Check if provider section exists
        if f"[model_providers.{provider}]" in content:
            # Update existing provider
            content = re.sub(
                rf'\[model_providers\.{re.escape(provider)}\][^\[]*',
                provider_section.strip() + "\n",
                content,
                flags=re.DOTALL
            )
        else:
            # Add new provider section
            content += "\n" + provider_section

        # Update model_provider reference
        content = re.sub(
            r'^model_provider\s*=.*$',
            f'model_provider = "{provider}"',
            content,
            flags=re.MULTILINE,
            count=1
        ) if re.search(r'^model_provider\s*=', content, re.MULTILINE) else f'model_provider = "{provider}"\n' + content

        self.write(content)
        return True

    def _generate_default_config(self):
        return '''model_provider = "Default"
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
'''


config_manager = ConfigManager(CODEX_CONFIG_PATH)


# ── Metrics Collector ──────────────────────────────────────────

class Metrics:
    def __init__(self):
        self._lock = threading.RLock()  # RLock to avoid deadlock when _notify_sse calls snapshot()
        self._sse_clients = []  # list of queue.Queue
        self.total = 0
        self.success = 0
        self.failed = 0
        self.total_input_tokens = 0
        self.total_output_tokens = 0
        self.total_latency = 0.0
        self.active_streams = 0
        self.history = deque(maxlen=MAX_HISTORY)
        self.throughput = deque(maxlen=60)
        self.latency_history = deque(maxlen=MAX_HISTORY)
        self.model_stats = {}
        self.start_time = time.time()

    def _notify_sse(self):
        """Push snapshot to all connected SSE clients. Must be called with _lock held."""
        if not self._sse_clients:
            return
        data = json.dumps(self.snapshot(), ensure_ascii=False)
        dead = []
        for q in self._sse_clients:
            try:
                q.put_nowait(data)
            except Exception:
                dead.append(q)
        for q in dead:
            try:
                self._sse_clients.remove(q)
            except ValueError:
                pass

    def subscribe_sse(self):
        """Register a new SSE client and return its queue."""
        import queue
        q = queue.Queue(maxsize=32)
        with self._lock:
            self._sse_clients.append(q)
        return q

    def unsubscribe_sse(self, q):
        """Remove an SSE client queue."""
        with self._lock:
            try:
                self._sse_clients.remove(q)
            except ValueError:
                pass

    def record_request(self, model, stream, status, latency, in_tok=0, out_tok=0, error="", preview="", req_id=None, input_detail=None):
        # 在锁外执行耗时的 JSON 序列化
        d = input_detail or {}
        tools_json = json.dumps(d.get("tools", []), ensure_ascii=False)[:2000]
        
        with self._lock:
            self.total += 1
            now = time.time()
            minute_ts = int(now // 60)
            if self.throughput and self.throughput[-1][0] == minute_ts:
                self.throughput[-1] = (minute_ts, self.throughput[-1][1] + 1)
            else:
                self.throughput.append((minute_ts, 1))
            if status == "success":
                self.success += 1
                self.total_input_tokens += in_tok
                self.total_output_tokens += out_tok
                self.total_latency += latency
            else:
                self.failed += 1
            self.latency_history.append({"t": now, "v": round(latency, 2)})
            self.model_stats[model] = self.model_stats.get(model, 0) + 1
            # Build input summary from messages
            msgs = d.get("messages", [])
            role_counts = {}
            for m in msgs:
                r = m.get("role", "unknown")
                role_counts[r] = role_counts.get(r, 0) + 1
            summary_parts = [f"{r}:{c}" for r, c in role_counts.items()]
            if d.get("tools"):
                summary_parts.append(f"tools:{len(d['tools'])}")
            input_summary = ", ".join(summary_parts) if summary_parts else ""
            # Last user message as input_preview
            last_user = ""
            for m in reversed(msgs):
                if m.get("role") == "user":
                    last_user = str(m.get("content", ""))[:120]
                    break
            # Build the entry dict once
            entry = {
                "time": time.strftime("%H:%M:%S", time.localtime(now)),
                "timestamp": now,
                "model": model,
                "stream": stream,
                "status": status,
                "latency": round(latency, 2),
                "input_tokens": in_tok,
                "output_tokens": out_tok,
                "error": error,
                "input_summary": input_summary,
                "input_preview": last_user,
                "input_detail": {
                    "instructions": (d.get("instructions") or "")[:2000],
                    "messages": [{"role": m.get("role",""), "content": str(m.get("content",""))[:2000], **({"tool_calls": m["tool_calls"]} if m.get("tool_calls") else {}), **({"tool_call_id": m["tool_call_id"]} if m.get("tool_call_id") else {})} for m in msgs[:50]],
                    "tools": tools_json,
                    "params": {},
                },
                "preview": preview[:120] if preview else "",
            }
            # If req_id provided, update the matching streaming placeholder entry
            if req_id:
                updated = False
                for i in range(len(self.history) - 1, -1, -1):
                    if self.history[i].get("req_id") == req_id:
                        self.history[i] = entry
                        updated = True
                        break
                if not updated:
                    self.history.append(entry)
            else:
                self.history.append(entry)
            self._notify_sse()

    def stream_start(self, req_id, model, input_detail=None):
        # 在锁外执行耗时的 JSON 序列化
        d = input_detail or {}
        tools_json = json.dumps(d.get("tools", []), ensure_ascii=False)[:2000]
        # Build input summary from messages
        msgs = d.get("messages", [])
        role_counts = {}
        for m in msgs:
            r = m.get("role", "unknown")
            role_counts[r] = role_counts.get(r, 0) + 1
        summary_parts = [f"{r}:{c}" for r, c in role_counts.items()]
        if d.get("tools"):
            summary_parts.append(f"tools:{len(d['tools'])}")
        input_summary = ", ".join(summary_parts) if summary_parts else ""
        # Last user message as input_preview
        last_user = ""
        for m in reversed(msgs):
            if m.get("role") == "user":
                last_user = str(m.get("content", ""))[:120]
                break
        with self._lock:
            self.active_streams += 1
            now = time.time()
            self.history.append({
                "req_id": req_id,
                "time": time.strftime("%H:%M:%S", time.localtime(now)),
                "timestamp": now,
                "model": model,
                "stream": True,
                "status": "streaming",
                "latency": 0,
                "input_tokens": 0,
                "output_tokens": 0,
                "error": "",
                "input_summary": input_summary,
                "input_preview": last_user,
                "input_detail": {
                    "instructions": (d.get("instructions") or "")[:2000],
                    "messages": [{"role": m.get("role",""), "content": str(m.get("content",""))[:2000], **({"tool_calls": m["tool_calls"]} if m.get("tool_calls") else {}), **({"tool_call_id": m["tool_call_id"]} if m.get("tool_call_id") else {})} for m in msgs[:50]],
                    "tools": tools_json,
                    "params": {},
                },
                "preview": "",
            })
            self._notify_sse()

    def stream_end(self):
        with self._lock:
            self.active_streams = max(0, self.active_streams - 1)
            self._notify_sse()

    def snapshot(self):
        with self._lock:
            uptime = time.time() - self.start_time
            avg_latency = (self.total_latency / self.success) if self.success else 0
            now = time.time()
            recent = [c for ts, c in self.throughput if now - ts * 60 < 300]
            rpm = sum(recent) / min(5, max(1, len(recent))) if recent else 0
            # 只返回最近 50 条 history，避免全量拷贝 200 条大记录
            recent_history = list(self.history)[-50:]
            return {
                "uptime": int(uptime),
                "total": self.total,
                "success": self.success,
                "failed": self.failed,
                "active_streams": self.active_streams,
                "avg_latency": round(avg_latency, 2),
                "rpm": round(rpm, 1),
                "total_input_tokens": self.total_input_tokens,
                "total_output_tokens": self.total_output_tokens,
                "total_tokens": self.total_input_tokens + self.total_output_tokens,
                "history": recent_history,
                "throughput": [{"t": ts, "c": c} for ts, c in self.throughput],
                "latency_history": list(self.latency_history),
                "model_stats": dict(self.model_stats),
            }


metrics = Metrics()


# ── Helpers ────────────────────────────────────────────────────

# DashScope doesn't support 'developer' role; map it to 'system'
ROLE_MAP = {"developer": "system"}


def extract_text_from_content(content):
    """Extract text from Responses API content format.
    
    Content can be:
    - A string
    - An array of content objects (input_text, input_image, input_file, etc.)
    """
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        texts = []
        for item in content:
            if isinstance(item, dict):
                # Handle different content types
                if item.get("type") == "input_text":
                    texts.append(item.get("text", ""))
                elif "text" in item:
                    texts.append(item["text"])
                elif "content" in item:
                    texts.append(str(item["content"]))
            else:
                texts.append(str(item))
        return "\n".join(texts)
    return str(content)


def convert_single_tool(tool):
    """Convert a single tool from Responses API format to Chat Completions API format."""
    if not isinstance(tool, dict):
        return tool
    
    # If already in Chat Completions format (has "function" key), keep as-is
    if "function" in tool:
        return tool
    
    tool_type = tool.get("type", "")
    
    # Handle namespace type - recursively convert nested tools
    if tool_type == "namespace":
        nested_tools = tool.get("tools", [])
        converted_nested = [convert_single_tool(t) for t in nested_tools if isinstance(t, dict)]
        # Flatten: return the nested tools directly (Chat Completions doesn't support namespace)
        return converted_nested
    
    # Handle web_search type - skip (DashScope doesn't support this)
    if tool_type == "web_search":
        return None
    
    # If has "name" and "parameters", treat as function tool and convert
    if "name" in tool and "parameters" in tool:
        return {
            "type": "function",
            "function": {
                "name": tool.get("name", ""),
                "description": tool.get("description", ""),
                "parameters": tool.get("parameters", {}),
            }
        }

    # If has "name" and "inputSchema" (MCP/plugin tools), convert
    if "name" in tool and "function" not in tool:
        has_input_schema = "inputSchema" in tool
        has_description = "description" in tool
        if has_input_schema or has_description:
            return {
                "type": "function",
                "function": {
                    "name": tool.get("name", ""),
                    "description": tool.get("description", ""),
                    "parameters": tool.get("inputSchema", {}),
                }
            }

    # Otherwise keep as-is
    return tool


def _extract_tool_name(tool):
    """Extract function name from a tool dict. Checks function.name first, then top-level name."""
    if not isinstance(tool, dict):
        return None
    name = tool.get("function", {}).get("name")
    if name:
        return name
    return tool.get("name")


def convert_tools_to_chat_format(tools):
    """Convert tools from Responses API format to Chat Completions API format.
    
    Handles:
    - function type tools
    - namespace type tools (flattens nested tools)
    - web_search type tools (skipped)
    - MCP/plugin tools with inputSchema
    
    Deduplicates tools by function name. DashScope requires unique tool names.
    """
    if not isinstance(tools, list):
        return tools
    
    import logging
    logger = logging.getLogger("proxy")
    
    # Log incoming tool names for debugging
    incoming_names = [_extract_tool_name(t) for t in tools if isinstance(t, dict)]
    logger.debug(f"[TOOLS] Received {len(tools)} tools, names: {[n for n in incoming_names if n]}")
    
    converted = []
    seen_names = set()
    for tool in tools:
        result = convert_single_tool(tool)
        if result is None:
            continue
        if isinstance(result, list):
            for t in result:
                name = _extract_tool_name(t)
                if name:
                    if name not in seen_names:
                        seen_names.add(name)
                        converted.append(t)
                    else:
                        logger.warning(f"[TOOLS] Duplicate namespace-nested tool '{name}' - skipped")
                else:
                    converted.append(t)
        elif isinstance(result, dict):
            name = _extract_tool_name(result)
            if name:
                if name not in seen_names:
                    seen_names.add(name)
                    converted.append(result)
                else:
                    logger.warning(f"[TOOLS] Duplicate tool '{name}' - skipped")
            else:
                # Fallback: check top-level "name" key
                fallback_name = result.get("name")
                if fallback_name:
                    if fallback_name not in seen_names:
                        seen_names.add(fallback_name)
                        converted.append(result)
                    else:
                        logger.warning(f"[TOOLS] Duplicate fallback-name tool '{fallback_name}' - skipped")
                else:
                    converted.append(result)
    
    # Post-conversion validation
    final_names = set()
    for t in converted:
        name = _extract_tool_name(t)
        if name:
            if name in final_names:
                logger.error(f"[TOOLS] BUG: duplicate '{name}' in final output - dedup failed!")
            final_names.add(name)
    
    logger.info(f"[TOOLS] Converted {len(tools)} -> {len(converted)} tools, names: {[n for n in final_names]}")
    
    return converted


def convert_input_to_messages(data_input):
    """Convert Responses API input to Chat Completions messages format.
    
    Handles all Responses API input types:
    - String input
    - Array of message objects with role/content
    - Array of content objects (input_text, input_image, etc.)
    - function_call_output objects
    - developer role (mapped to system for DashScope)
    """
    if isinstance(data_input, str):
        return [{"role": "user", "content": data_input}]
    
    if not isinstance(data_input, list):
        return [{"role": "user", "content": str(data_input)}]
    
    messages = []
    for item in data_input:
        if isinstance(item, dict):
            item_type = item.get("type", "")
            
            # Handle message type items
            if item_type == "message" or ("role" in item and "content" in item):
                role = item.get("role", "user")
                content = item.get("content", "")
                
                # Map developer role to system (DashScope doesn't support developer)
                role = ROLE_MAP.get(role, role)
                
                # Extract text from content (handles both string and array formats)
                text_content = extract_text_from_content(content)
                messages.append({"role": role, "content": text_content})
            
            # Handle assistant function calls (Responses API format)
            elif item_type == "function_call":
                # Convert to assistant message with tool_calls
                call_id = item.get("call_id", "") or item.get("id", "")
                name = item.get("name", "")
                arguments = item.get("arguments", "")
                messages.append({
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments
                        }
                    }]
                })
            
            # Handle function call outputs
            elif item_type == "function_call_output":
                # Convert to tool message format
                call_id = item.get("call_id", "")
                output = item.get("output", "")
                messages.append({
                    "role": "tool",
                    "content": str(output),
                    "tool_call_id": call_id
                })
            
            # Handle other content types (input_text, input_image, etc.)
            elif item_type in ["input_text", "input_image", "input_file"]:
                text = extract_text_from_content(item)
                messages.append({"role": "user", "content": text})
            
            # Fallback: convert unknown types to user message
            else:
                text = extract_text_from_content(item)
                if text:
                    messages.append({"role": "user", "content": text})
        else:
            # Non-dict items become user messages
            messages.append({"role": "user", "content": str(item)})
    
    messages = _merge_consecutive_assistant_messages(messages)
    return messages


def _merge_consecutive_assistant_messages(messages):
    """Merge consecutive assistant messages. Chat Completions API requires
    assistant text + tool_calls in a single message, not separate ones."""
    merged = []
    for msg in messages:
        if (merged and msg.get("role") == "assistant" 
                and merged[-1].get("role") == "assistant"):
            prev = merged[-1]
            # Merge content
            if msg.get("content") and prev.get("content"):
                prev["content"] = prev["content"] + "\n" + msg["content"]
            elif msg.get("content"):
                prev["content"] = msg["content"]
            # Merge tool_calls
            if msg.get("tool_calls"):
                prev.setdefault("tool_calls", []).extend(msg["tool_calls"])
        else:
            merged.append(dict(msg))  # copy to avoid mutating original
    merged = _deduplicate_tool_calls(merged)
    return merged


def _deduplicate_tool_calls(messages):
    """Remove duplicate tool calls from conversation history.
    DashScope rejects requests with identical tool calls (same name + arguments)."""
    seen_tool_calls = set()
    removed_call_ids = set()
    result = []

    for msg in messages:
        role = msg.get("role", "")

        if role == "assistant":
            tool_calls = msg.get("tool_calls")
            if tool_calls:
                unique_calls = []
                for tc in tool_calls:
                    func = tc.get("function", {})
                    name = func.get("name", "")
                    args = func.get("arguments", "")
                    call_id = tc.get("id", "")
                    key = f"{name}:{args}"

                    if key in seen_tool_calls:
                        removed_call_ids.add(call_id)
                    else:
                        seen_tool_calls.add(key)
                        unique_calls.append(tc)

                if not unique_calls:
                    continue

                new_msg = dict(msg)
                new_msg["tool_calls"] = unique_calls
                result.append(new_msg)
            else:
                result.append(msg)
        elif role == "tool":
            call_id = msg.get("tool_call_id", "")
            if call_id in removed_call_ids:
                continue
            result.append(msg)
        else:
            result.append(msg)

    return result


def build_sse(event, data):
    return f"event: {event}\ndata: {json.dumps(data)}\n\n"


def make_response_id():
    return f"resp_{uuid.uuid4().hex[:24]}"


def make_item_id():
    return f"item_{uuid.uuid4().hex[:24]}"


def make_req_id():
    return f"req_{uuid.uuid4().hex[:12]}"


# ── Proxy Handler ──────────────────────────────────────────────

class ProxyHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[{time.strftime('%H:%M:%S')}] {args[0]}")

    def _send(self, code, body, content_type="application/json"):
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.end_headers()
        if isinstance(body, dict):
            body = json.dumps(body).encode()
        elif isinstance(body, str):
            body = body.encode()
        self.wfile.write(body)

    def _read_body(self):
        length = int(self.headers.get("Content-Length", 0))
        return json.loads(self.rfile.read(length)) if length else {}

    def _forward(self, payload, stream):
        data = json.dumps(payload).encode()
        req = Request(UPSTREAM, data=data, method="POST")
        req.add_header("Content-Type", "application/json")
        req.add_header("Authorization", f"Bearer {API_KEY}")
        req.add_header("User-Agent", "codex-proxy/1.0")
        return urlopen(req, timeout=300)

    def do_POST(self):
        if self.path == "/v1/responses":
            self._handle_responses()
        else:
            self._send(404, {"error": f"Unknown endpoint: {self.path}"})

    def do_GET(self):
        if self.path == "/health":
            self._send(200, {"status": "ok"})
        else:
            self._send(404, {"error": "Not found"})

    def _handle_responses(self):
        body = self._read_body()
        model = body.get("model", "qwen-plus")
        
        # DEBUG: Log original request
        print(f"\n{'='*60}")
        print(f"[DEBUG] Received request for model: {model}")
        if "tools" in body:
            print(f"[DEBUG] Original tools count: {len(body['tools'])}")
            for i, tool in enumerate(body["tools"]):
                print(f"[DEBUG] tools[{i}]: {json.dumps(tool, ensure_ascii=False)[:200]}")
        print(f"{'='*60}\n")
        
        # Convert Responses API input to Chat Completions messages
        messages = convert_input_to_messages(body.get("input", ""))
        
        # Handle instructions (system prompt)
        instructions = body.get("instructions", "")
        if instructions:
            # Prepend system message with instructions
            messages.insert(0, {"role": "system", "content": instructions})
        
        stream = body.get("stream", False)

        # Model mapping: use configured model if set, otherwise use request model
        # This ensures Codex's deepseek-v4-pro gets replaced with the configured Qwen model
        upstream_model = UPSTREAM_MODEL if UPSTREAM_MODEL else model
        upstream_payload = {
            "model": upstream_model,
            "messages": messages,
            "stream": stream,
        }
        if UPSTREAM_MODEL:
            print(f"[DEBUG] Model mapping: {model} -> {upstream_model}")
        
        # DEBUG: Log key parameters
        print(f"[DEBUG] stop={body.get('stop')}, max_output_tokens={body.get('max_output_tokens')}, "
              f"max_tokens={body.get('max_tokens')}, temperature={body.get('temperature')}, "
              f"top_p={body.get('top_p')}")
        print(f"[DEBUG] messages count={len(messages)}, stream={stream}")
        if messages:
            first_msg = messages[0]
            print(f"[DEBUG] first message role={first_msg.get('role')}, "
                  f"content[:200]={str(first_msg.get('content', ''))[:200]}")
            # Log last 3 messages to understand context
            print(f"[DEBUG] Last {min(3, len(messages))} messages:")
            for i, msg in enumerate(messages[-3:]):
                role = msg.get('role', '?')
                content = str(msg.get('content', ''))[:150]
                tool_calls = msg.get('tool_calls')
                tool_call_id = msg.get('tool_call_id')
                print(f"[DEBUG]   [{len(messages)-3+i}] role={role}, content[:150]={content}")
                if tool_calls:
                    print(f"[DEBUG]       tool_calls={json.dumps(tool_calls, ensure_ascii=False)[:200]}")
                if tool_call_id:
                    print(f"[DEBUG]       tool_call_id={tool_call_id}")
        
        # Map Responses API parameters to Chat Completions API parameters
        param_mapping = {
            "temperature": "temperature",
            "max_tokens": "max_tokens",
            "max_output_tokens": "max_tokens",
            "top_p": "top_p",
            "frequency_penalty": "frequency_penalty",
            "presence_penalty": "presence_penalty",
            "stop": "stop",
            "seed": "seed",
            "logprobs": "logprobs",
            "top_logprobs": "top_logprobs",
        }
        
        for resp_key, chat_key in param_mapping.items():
            if resp_key in body:
                upstream_payload[chat_key] = body[resp_key]
        
        # Handle tools (convert Responses API format to Chat Completions format)
        has_tools = False
        if "tools" in body:
            has_tools = True
            converted_tools = convert_tools_to_chat_format(body["tools"])
            upstream_payload["tools"] = converted_tools
            
            # DEBUG: Log converted tools
            print(f"\n{'='*60}")
            print(f"[DEBUG] Converted tools count: {len(converted_tools)}")
            for i, tool in enumerate(converted_tools):
                print(f"[DEBUG] converted[{i}]: {json.dumps(tool, ensure_ascii=False)[:200]}")
            print(f"{'='*60}\n")
        
        # Handle tool_choice
        if "tool_choice" in body:
            upstream_payload["tool_choice"] = body["tool_choice"]

        # Ensure sufficient output tokens when tools are present
        # Without explicit max_tokens, model may stop before generating tool_calls
        if has_tools and "max_tokens" not in upstream_payload:
            upstream_payload["max_tokens"] = 32768
            print(f"[DEBUG] Auto-set max_tokens=32768 (tools present, no explicit limit)")

        # Disable thinking mode for Qwen3 models.
        # Qwen3 thinking consumes tokens internally, leaving no budget for tool_calls
        # in long contexts. Codex is already a reasoning framework, no need for model-level thinking.
        if has_tools:
            upstream_payload["enable_thinking"] = False
            print(f"[DEBUG] Disabled thinking mode (tools present)")

        # When tools are present, force non-streaming to upstream.
        # DashScope streaming sometimes drops tool_calls in long contexts.
        # Non-streaming returns reliable tool_calls + accurate usage.
        # We'll still send SSE events to Codex (simulated streaming).
        upstream_stream = False if has_tools else stream
        upstream_payload["stream"] = upstream_stream

        # Build structured input_detail for history recording
        input_detail = {
            "instructions": instructions,
            "messages": messages,
            "tools": body.get("tools", []),
            "params": {k: v for k, v in body.items() if k not in ("input", "instructions", "tools", "model", "stream")},
        }

        print(f"[DEBUG] Calling upstream: stream={upstream_stream}, messages={len(upstream_payload.get('messages',[]))}, tools={len(upstream_payload.get('tools',[]))}")
        t0 = time.time()
        try:
            resp = self._forward(upstream_payload, stream=upstream_stream)
            print(f"[DEBUG] Upstream responded after {time.time()-t0:.2f}s")
        except HTTPError as e:
            err = e.read().decode(errors="replace")
            latency = time.time() - t0
            print(f"[DEBUG] Upstream HTTPError: {e.code} - {err[:200]}")
            metrics.record_request(model, stream, "error", latency, error=f"HTTP {e.code}", input_detail=input_detail)
            self._send(e.code, {"error": {"message": err, "type": "upstream_error"}})
            return
        except URLError as e:
            latency = time.time() - t0
            print(f"[DEBUG] Upstream URLError: {e.reason}")
            metrics.record_request(model, stream, "error", latency, error=str(e.reason), input_detail=input_detail)
            self._send(502, {"error": {"message": str(e.reason), "type": "proxy_error"}})
            return

        if upstream_stream:
            self._stream_response(resp, model, t0, input_detail)
        elif stream:
            # Codex wants streaming but upstream returned non-streaming
            print(f"[DEBUG] Converting non-streaming to SSE for Codex")
            self._stream_from_non_streaming(resp, model, t0, input_detail)
        else:
            self._normal_response(resp, model, t0, input_detail)

    def _normal_response(self, resp, model, t0, input_detail=None):
        try:
            raw = resp.read().decode()
        finally:
            resp.close()
        try:
            chat_resp = json.loads(raw)
        except json.JSONDecodeError:
            latency = time.time() - t0
            metrics.record_request(model, False, "error", latency, error="Invalid JSON", input_detail=input_detail)
            self._send(502, {"error": {"message": "Invalid upstream response", "type": "proxy_error"}})
            return

        latency = time.time() - t0
        choice = chat_resp.get("choices", [{}])[0]
        msg = choice.get("message", {})
        content = msg.get("content", "")
        usage = chat_resp.get("usage", {})
        in_tok = usage.get("prompt_tokens", 0)
        out_tok = usage.get("completion_tokens", 0)

        metrics.record_request(model, False, "success", latency, in_tok, out_tok, preview=content, input_detail=input_detail)

        resp_id = make_response_id()
        item_id = make_item_id()
        result = {
            "id": resp_id,
            "object": "response",
            "created_at": int(time.time()),
            "model": chat_resp.get("model", model),
            "output": [{
                "type": "message", "id": item_id, "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": content, "annotations": []}],
            }],
            "status": "completed",
            "usage": {
                "input_tokens": in_tok,
                "output_tokens": out_tok,
                "total_tokens": in_tok + out_tok,
            },
        }
        self._send(200, result)

    def _stream_response(self, resp, model, t0, input_detail=None):
        resp_id = make_response_id()
        item_id = make_item_id()
        created = int(time.time())

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()

        req_id = make_req_id()
        metrics.stream_start(req_id, model, input_detail)
        full_text = []
        message_events_sent = False
        client_disconnected = False

        try:
            self._write_sse("response.created", {
                "type": "response.created", "response": {
                    "id": resp_id, "object": "response", "created_at": created,
                    "model": model, "output": [], "status": "in_progress",
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                }
            })
            self._write_sse("response.in_progress", {
                "type": "response.in_progress",
                "response": {"id": resp_id, "object": "response", "status": "in_progress"},
            })

            usage_info = {}
            tool_calls = {}  # index -> {id, name, arguments_parts}
            tc_chunk_count = 0
            last_chunks = []  # ring buffer for last 5 raw chunks
            try:
                for line in resp:
                    line = line.decode("utf-8", errors="replace").strip()
                    if not line.startswith("data: "):
                        continue
                    data_str = line[6:]
                    if data_str.strip() == "[DONE]":
                        break
                    try:
                        chunk = json.loads(data_str)
                    except json.JSONDecodeError:
                        continue
                    # Keep last 5 chunks for debugging
                    last_chunks.append(data_str[:500])
                    if len(last_chunks) > 5:
                        last_chunks.pop(0)
                    # DashScope sends "usage": null in every chunk; only capture non-null usage
                    if chunk.get("usage"):
                        usage_info = chunk["usage"]
                    choices = chunk.get("choices", [])
                    if not choices:
                        continue
                    delta = choices[0].get("delta", {})
                    text = delta.get("content", "")
                    if text:
                        full_text.append(text)
                        if not message_events_sent:
                            message_events_sent = True
                            self._write_sse("response.output_item.added", {
                                "type": "response.output_item.added", "output_index": 0,
                                "item": {"type": "message", "id": item_id, "role": "assistant",
                                         "status": "in_progress", "content": []},
                            })
                            self._write_sse("response.content_part.added", {
                                "type": "response.content_part.added",
                                "item_id": item_id, "output_index": 0, "content_index": 0,
                                "part": {"type": "output_text", "text": "", "annotations": []},
                            })
                        self._write_sse("response.output_text.delta", {
                            "type": "response.output_text.delta",
                            "item_id": item_id, "output_index": 0, "content_index": 0,
                            "delta": text,
                        })
                    # Handle streaming tool_calls (function calls)
                    tc_list = delta.get("tool_calls", [])
                    if tc_list:
                        tc_chunk_count += 1
                    for tc in tc_list:
                        idx = tc.get("index", 0)
                        if idx not in tool_calls:
                            tool_calls[idx] = {
                                "id": tc.get("id", ""),
                                "name": "",
                                "arguments_parts": [],
                            }
                        tc_info = tool_calls[idx]
                        if "id" in tc and tc["id"]:
                            tc_info["id"] = tc["id"]
                        func = tc.get("function", {})
                        if "name" in func and func["name"]:
                            tc_info["name"] = func["name"]
                        if "arguments" in func:
                            tc_info["arguments_parts"].append(func["arguments"])
            finally:
                resp.close()

            final_text = "".join(full_text)
            # Build function calls from accumulated tool_calls
            function_calls = []
            for idx in sorted(tool_calls.keys()):
                tc = tool_calls[idx]
                if tc["name"]:
                    function_calls.append({
                        "id": tc["id"] or f"call_{uuid.uuid4().hex[:12]}",
                        "name": tc["name"],
                        "arguments": "".join(tc["arguments_parts"]),
                    })
            print(f"[DEBUG] final_text_len={len(final_text)}, tc_chunk_count={tc_chunk_count}, "
                  f"tool_calls_indices={list(tool_calls.keys())}, function_calls_count={len(function_calls)}")
            if function_calls:
                for fc in function_calls:
                    print(f"[DEBUG]   function_call: {fc['name']}({fc['arguments'][:100]}...)")
            elif final_text and tc_chunk_count == 0:
                # No function calls but text exists - check if model was about to call tools
                text_lower = final_text.lower()
                if any(kw in text_lower for kw in ['implement', 'fix', 'starting', 'let me', 'now i\'ll', 'i\'ll']):
                    print(f"[DEBUG] WARNING: Text suggests tool intent but no tool_calls received from DashScope!")
                    print(f"[DEBUG] final_text[:300]={final_text[:300]}")
                    print(f"[DEBUG] Last {len(last_chunks)} raw chunks from DashScope:")
                    for i, c in enumerate(last_chunks):
                        print(f"[DEBUG]   chunk[{i}]: {c}")
            latency = time.time() - t0

            # If upstream didn't provide usage info, estimate from output text.
            # DashScope streaming responses may omit usage, causing Codex to see
            # 0 tokens and prematurely end the turn without auto-continue / tool calls.
            usage_info = usage_info or {}
            in_tok = usage_info.get("prompt_tokens", 0)
            out_tok = usage_info.get("completion_tokens", 0)
            if out_tok == 0 and (final_text or function_calls):
                # Rough estimation: CJK ~1.5 chars/token, ASCII ~4 chars/token.
                # Use a conservative blend: ~3 chars/token.
                total_chars = len(final_text) + sum(len(fc["arguments"]) for fc in function_calls)
                out_tok = max(1, total_chars // 3)
                in_tok = in_tok or 0

            # Build output items for the response
            output_items = []
            if final_text:
                output_items.append({
                    "type": "message", "id": item_id, "role": "assistant",
                    "status": "completed",
                    "content": [{"type": "output_text", "text": final_text, "annotations": []}],
                })
            for fc in function_calls:
                output_items.append({
                    "type": "function_call",
                    "id": fc["id"],
                    "call_id": fc["id"],
                    "name": fc["name"],
                    "arguments": fc["arguments"],
                })

            # Determine output_index offset for function calls
            next_output_index = 0
            if final_text:
                # Text message occupies output_index 0
                self._write_sse("response.content_part.done", {
                    "type": "response.content_part.done",
                    "item_id": item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": final_text, "annotations": []},
                })
                self._write_sse("response.output_item.done", {
                    "type": "response.output_item.done", "output_index": 0,
                    "item": {"type": "message", "id": item_id, "role": "assistant",
                             "status": "completed",
                             "content": [{"type": "output_text", "text": final_text, "annotations": []}]},
                })
                next_output_index = 1

            # Emit SSE events for each function_call so Codex can see them
            for fc_i, fc in enumerate(function_calls):
                oi = next_output_index + fc_i
                fc_item_id = fc["id"]
                fc_item = {
                    "type": "function_call",
                    "id": fc_item_id,
                    "call_id": fc_item_id,
                    "name": fc["name"],
                    "arguments": fc["arguments"],
                    "status": "completed",
                }
                self._write_sse("response.output_item.added", {
                    "type": "response.output_item.added",
                    "output_index": oi,
                    "item": {**fc_item, "status": "in_progress"},
                })
                self._write_sse("response.output_item.done", {
                    "type": "response.output_item.done",
                    "output_index": oi,
                    "item": fc_item,
                })

            self._write_sse("response.completed", {
                "type": "response.completed",
                "response": {
                    "id": resp_id, "object": "response", "created_at": created,
                    "model": model, "output": output_items or [{
                        "type": "message", "id": item_id, "role": "assistant",
                        "status": "completed",
                        "content": [{"type": "output_text", "text": final_text, "annotations": []}],
                    }],
                    "status": "completed",
                    "usage": {"input_tokens": in_tok, "output_tokens": out_tok,
                              "total_tokens": in_tok + out_tok},
                },
            })
            # Send [DONE] to signal end of SSE stream
            self.wfile.write(b"data: [DONE]\n\n")
            self.wfile.flush()

            metrics.stream_end()
            metrics.record_request(model, True, "success", latency, in_tok, out_tok, preview=final_text, req_id=req_id, input_detail=input_detail)
        except ClientDisconnected:
            client_disconnected = True
            # Client disconnected during streaming - still record metrics
            final_text = "".join(full_text)
            latency = time.time() - t0
            metrics.stream_end()
            metrics.record_request(model, True, "client_disconnect", latency, preview=final_text, req_id=req_id, input_detail=input_detail)

    def _stream_from_non_streaming(self, resp, model, t0, input_detail=None):
        """Convert a non-streaming Chat Completions response to SSE events for Codex.
        Used when tools are present to avoid streaming tool_calls loss in DashScope."""
        try:
            raw = resp.read().decode()
        finally:
            resp.close()
        try:
            chat_resp = json.loads(raw)
        except json.JSONDecodeError:
            latency = time.time() - t0
            metrics.record_request(model, False, "error", latency, error="Invalid JSON", input_detail=input_detail)
            self._send(502, {"error": {"message": "Invalid upstream response", "type": "proxy_error"}})
            return

        resp_id = make_response_id()
        item_id = make_item_id()
        created = int(time.time())

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()

        req_id = make_req_id()
        metrics.stream_start(req_id, model, input_detail)

        try:
            self._write_sse("response.created", {
                "type": "response.created", "response": {
                    "id": resp_id, "object": "response", "created_at": created,
                    "model": model, "output": [], "status": "in_progress",
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                }
            })
            self._write_sse("response.in_progress", {
                "type": "response.in_progress",
                "response": {"id": resp_id, "object": "response", "status": "in_progress"},
            })

            choice = chat_resp.get("choices", [{}])[0]
            msg = choice.get("message", {})
            content = msg.get("content", "") or ""
            usage = chat_resp.get("usage", {})
            in_tok = usage.get("prompt_tokens", 0)
            out_tok = usage.get("completion_tokens", 0)
            finish_reason = choice.get("finish_reason", "")

            # Extract tool_calls from non-streaming response
            raw_tool_calls = msg.get("tool_calls") or []
            function_calls = []
            for tc in raw_tool_calls:
                func = tc.get("function", {})
                function_calls.append({
                    "id": tc.get("id", f"call_{uuid.uuid4().hex[:12]}"),
                    "name": func.get("name", ""),
                    "arguments": func.get("arguments", ""),
                })

            print(f"[DEBUG] non-streaming: finish_reason={finish_reason}, "
                  f"text_len={len(content)}, function_calls_count={len(function_calls)}, "
                  f"in_tok={in_tok}, out_tok={out_tok}")
            if function_calls:
                for fc in function_calls:
                    print(f"[DEBUG]   function_call: {fc['name']}({fc['arguments'][:100]}...)")

            # Emit text message events
            if content:
                self._write_sse("response.output_item.added", {
                    "type": "response.output_item.added", "output_index": 0,
                    "item": {"type": "message", "id": item_id, "role": "assistant",
                             "status": "in_progress", "content": []},
                })
                self._write_sse("response.content_part.added", {
                    "type": "response.content_part.added",
                    "item_id": item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": "", "annotations": []},
                })
                # Send text as a single delta (simulated streaming)
                self._write_sse("response.output_text.delta", {
                    "type": "response.output_text.delta",
                    "item_id": item_id, "output_index": 0, "content_index": 0,
                    "delta": content,
                })

            # Build output items
            output_items = []
            next_output_index = 0
            if content:
                output_items.append({
                    "type": "message", "id": item_id, "role": "assistant",
                    "status": "completed",
                    "content": [{"type": "output_text", "text": content, "annotations": []}],
                })
                self._write_sse("response.content_part.done", {
                    "type": "response.content_part.done",
                    "item_id": item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": content, "annotations": []},
                })
                self._write_sse("response.output_item.done", {
                    "type": "response.output_item.done", "output_index": 0,
                    "item": {"type": "message", "id": item_id, "role": "assistant",
                             "status": "completed",
                             "content": [{"type": "output_text", "text": content, "annotations": []}]},
                })
                next_output_index = 1

            # Emit SSE events for each function_call
            for fc_i, fc in enumerate(function_calls):
                oi = next_output_index + fc_i
                fc_item_id = fc["id"]
                fc_item = {
                    "type": "function_call",
                    "id": fc_item_id,
                    "call_id": fc_item_id,
                    "name": fc["name"],
                    "arguments": fc["arguments"],
                    "status": "completed",
                }
                self._write_sse("response.output_item.added", {
                    "type": "response.output_item.added",
                    "output_index": oi,
                    "item": {**fc_item, "status": "in_progress"},
                })
                self._write_sse("response.output_item.done", {
                    "type": "response.output_item.done",
                    "output_index": oi,
                    "item": fc_item,
                })
                output_items.append(fc_item)

            # Fallback token estimation if usage is missing
            if out_tok == 0 and (content or function_calls):
                total_chars = len(content) + sum(len(fc["arguments"]) for fc in function_calls)
                out_tok = max(1, total_chars // 3)
                in_tok = in_tok or 0

            self._write_sse("response.completed", {
                "type": "response.completed",
                "response": {
                    "id": resp_id, "object": "response", "created_at": created,
                    "model": model, "output": output_items or [{
                        "type": "message", "id": item_id, "role": "assistant",
                        "status": "completed",
                        "content": [{"type": "output_text", "text": content, "annotations": []}],
                    }],
                    "status": "completed",
                    "usage": {"input_tokens": in_tok, "output_tokens": out_tok,
                              "total_tokens": in_tok + out_tok},
                },
            })
            # Send [DONE] to signal end of SSE stream
            self.wfile.write(b"data: [DONE]\n\n")
            self.wfile.flush()

            latency = time.time() - t0
            metrics.stream_end()
            metrics.record_request(model, True, "success", latency, in_tok, out_tok, preview=content, req_id=req_id, input_detail=input_detail)
        except ClientDisconnected:
            latency = time.time() - t0
            metrics.stream_end()
            metrics.record_request(model, True, "client_disconnect", latency, preview=content, req_id=req_id, input_detail=input_detail)

    def _write_sse(self, event, data):
        try:
            self.wfile.write(build_sse(event, data).encode())
            self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            raise ClientDisconnected()
        except OSError:
            raise ClientDisconnected()


# ── Monitor Handler ────────────────────────────────────────────

DASHBOARD_PATH = Path(__file__).parent / "index.html"


class MonitorHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def do_GET(self):
        if self.path == "/api/stats":
            self._send_json(metrics.snapshot())
        elif self.path == "/api/events":
            self._handle_sse()
        elif self.path == "/api/config":
            self._send_json({"content": config_manager.read()})
        elif self.path == "/api/config/current":
            self._send_json(config_manager.get_current_model())
        elif self.path == "/api/config/saved":
            self._send_json({"configs": secure_store.list_configs()})
        elif self.path in ("/", "/index.html"):
            self._serve_dashboard()
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if self.path == "/api/config":
            self._handle_save_config()
        elif self.path == "/api/config/quick":
            self._handle_quick_config()
        elif self.path == "/api/config/load":
            self._handle_load_config()
        else:
            self.send_response(404)
            self.end_headers()

    def do_DELETE(self):
        if self.path.startswith("/api/config/"):
            config_name = self.path.split("/")[-1]
            if config_name:
                secure_store.delete_config(config_name)
                self._send_json({"success": True})
            else:
                self._send_json({"error": "Config name required"}, 400)
        else:
            self.send_response(404)
            self.end_headers()

    def _handle_save_config(self):
        try:
            length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(length))
            content = body.get("content", "")
            config_manager.write(content)
            self._send_json({"success": True})
        except Exception as e:
            self._send_json({"success": False, "error": str(e)}, 500)

    def _handle_quick_config(self):
        global UPSTREAM_MODEL, UPSTREAM, API_KEY
        try:
            length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(length))
            config_name = body.get("name", "").strip()
            model = body.get("model", "")
            base_url = body.get("base_url", "")
            api_key = body.get("api_key", "")
            provider = body.get("provider", "Custom")
            config_manager.apply_model(model, provider, base_url, api_key)
            
            # Save to secure store if name provided
            saved_to_db = False
            if config_name and api_key:
                saved_to_db = secure_store.save_config(
                    name=config_name,
                    model=model,
                    provider=provider,
                    base_url=base_url,
                    api_key=api_key
                )
            
            # Update runtime variables for proxy forwarding
            UPSTREAM_MODEL = model
            UPSTREAM = f"{base_url.rstrip('/')}/chat/completions"
            API_KEY = api_key
            print(f"[CONFIG] Applied config: model={UPSTREAM_MODEL}, upstream={UPSTREAM}")
            
            self._send_json({"success": True, "saved_to_db": saved_to_db})
        except Exception as e:
            self._send_json({"success": False, "error": str(e)}, 500)

    def _handle_load_config(self):
        """Load a saved config by name and apply it."""
        global UPSTREAM_MODEL, UPSTREAM, API_KEY
        try:
            length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(length))
            config_name = body.get("name", "").strip()
            
            if not config_name:
                self._send_json({"success": False, "error": "Config name required"}, 400)
                return
            
            config = secure_store.get_config(config_name)
            if not config:
                self._send_json({"success": False, "error": "Config not found"}, 404)
                return
            
            # Apply the loaded config to codex config.toml
            config_manager.apply_model(
                config["model"],
                config["provider"],
                config["base_url"],
                config["api_key"]
            )
            
            # Update runtime variables for proxy forwarding
            UPSTREAM_MODEL = config["model"]
            UPSTREAM = f"{config['base_url'].rstrip('/')}/chat/completions"
            API_KEY = config["api_key"]
            print(f"[CONFIG] Loaded config '{config_name}': model={UPSTREAM_MODEL}, upstream={UPSTREAM}")
            
            self._send_json({
                "success": True,
                "config": {
                    "name": config["name"],
                    "model": config["model"],
                    "provider": config["provider"],
                    "base_url": config["base_url"],
                }
            })
        except Exception as e:
            self._send_json({"success": False, "error": str(e)}, 500)

    def _send_json(self, data, code=200):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def _serve_dashboard(self):
        try:
            html = DASHBOARD_PATH.read_text(encoding="utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(html.encode())
        except Exception as e:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(f"Error loading dashboard: {e}".encode())

    def _handle_sse(self):
        """SSE endpoint: pushes metrics snapshot on every change."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        # Send initial snapshot immediately
        try:
            initial = json.dumps(metrics.snapshot(), ensure_ascii=False)
            self.wfile.write(f"data: {initial}\n\n".encode())
            self.wfile.flush()
        except Exception:
            return
        # Subscribe for updates
        q = metrics.subscribe_sse()
        try:
            while True:
                try:
                    data = q.get(timeout=30)
                    self.wfile.write(f"data: {data}\n\n".encode())
                    self.wfile.flush()
                except Exception:
                    break
        finally:
            metrics.unsubscribe_sse(q)


# ── Threaded Server ────────────────────────────────────────────

class ThreadedHTTPServer(HTTPServer):
    allow_reuse_address = True

    def process_request(self, request, client_address):
        t = threading.Thread(target=self._handle, args=(request, client_address))
        t.daemon = True
        t.start()

    def _handle(self, request, client_address):
        try:
            self.finish_request(request, client_address)
        except Exception:
            import traceback
            traceback.print_exc()
            self.handle_error(request, client_address)
        finally:
            self.shutdown_request(request)


# ── Main ───────────────────────────────────────────────────────

if __name__ == "__main__":
    # Note: We use hardcoded defaults for UPSTREAM and API_KEY.
    # Saved configs can be loaded via the admin UI (http://127.0.0.1:8001)
    # The model mapping (UPSTREAM_MODEL) will be set when user selects a config in UI.
    print(f"Using default upstream: {UPSTREAM}")
    
    proxy_server = ThreadedHTTPServer((HOST, PORT), ProxyHandler)
    monitor_server = ThreadedHTTPServer((HOST, MONITOR_PORT), MonitorHandler)

    monitor_thread = threading.Thread(target=monitor_server.serve_forever)
    monitor_thread.daemon = True
    monitor_thread.start()

    print(f"Proxy:    http://{HOST}:{PORT}")
    print(f"Monitor:  http://{HOST}:{MONITOR_PORT}")
    print(f"Upstream: {UPSTREAM}")
    print()
    try:
        proxy_server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down.")
        proxy_server.shutdown()
        monitor_server.shutdown()
