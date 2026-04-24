
#!/usr/bin/env python3
"""

JSON contract:
  Input:  {"message": "...", "context": {"current_vwc": 25.5, ...}, "session_id": "..."}
  Output: {"reply": "..."}
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import threading
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# ── Config ──────────────────────────────────────────────────────────────
CLI_TIMEOUT = 120
AGENT_ID = "main"
AGRI_SKILL_PATH = "/opt/ai-agriculture/cloud/frontend_v2_premium/AI-ag-agent-skill.md"

# ── Knowledge base ──────────────────────────────────────────────────────
AGRI_SYSTEM_PROMPT = ""


def load_agri_knowledge() -> str:
    try:
        with open(AGRI_SKILL_PATH) as f:
            content = f.read()
        return (
            "你是专业农业AI助手，拥有以下知识库：\n\n"
            f"{content}\n\n"
            "请根据传感器数据给出中文农业建议。简明扼要，直接回答问题。"
        )
    except FileNotFoundError:
        return (
            "你是专业农业AI助手。根据传感器数据给出中文农业建议。"
            "简明扼要，直接回答问题。"
        )


# ── Multi-turn session store ────────────────────────────────────────────
# session_id -> list of messages (for injection into prompt)
_sessions: dict[str, list[dict]] = {}
_sessions_lock = threading.Lock()
MAX_HISTORY = 20  # exchanges


def get_history(sid: str) -> list[dict]:
    with _sessions_lock:
        if sid not in _sessions:
            _sessions[sid] = []
        return _sessions[sid]


def append_history(sid: str, user: str, reply: str):
    with _sessions_lock:
        h = _sessions.get(sid, [])
        h.append({"role": "user", "content": user})
        h.append({"role": "assistant", "content": reply})
        if len(h) > MAX_HISTORY * 2:
            _sessions[sid] = h[-MAX_HISTORY * 2:]


# ── Core: build prompt + call CLI ───────────────────────────────────────

def build_full_prompt(message: str, context: dict, session_id: str) -> str:
    """Build the full prompt with system hint, history, context, and new query."""
    parts = [f"[system]\n{AGRI_SYSTEM_PROMPT}\n"]

    # Context block
    ctx_lines = []
    if context.get("current_vwc") is not None:
        ctx_lines.append(f"土壤水分(VWC): {context['current_vwc']}%")
    if context.get("temperature") is not None:
        ctx_lines.append(f"温度: {context['temperature']}°C")
    if context.get("humidity") is not None:
        ctx_lines.append(f"湿度: {context['humidity']}%")
    if context.get("crop_type"):
        ctx_lines.append(f"作物类型: {context['crop_type']}")
    if context.get("ec") is not None:
        ctx_lines.append(f"EC值: {context['ec']} mS/cm")
    if context.get("ph") is not None:
        ctx_lines.append(f"pH值: {context['ph']}")

    if ctx_lines:
        parts.append("[context]\n当前环境数据：\n" + "\n".join(ctx_lines) + "\n")

    # History
    history = get_history(session_id)
    if history:
        parts.append("[history]")
        for h in history:
            role_label = "user" if h["role"] == "user" else "assistant"
            parts.append(f"{role_label}: {h['content']}")
        parts.append("")

    # Current query
    parts.append(f"[user]\n{message}")

    return "\n".join(parts)


def call_openclaw_cli(full_prompt: str) -> str:
    """Run openclaw agent --local, parse JSON reply."""
    cmd = [
        "openclaw", "agent", "--local", "--agent", AGENT_ID,
        "--message", full_prompt,
        "--json", "--thinking", "off",
        "--timeout", str(CLI_TIMEOUT),
    ]
    env = dict(os.environ)
    env.setdefault("HOME", "/root")
    proc = subprocess.run(
        cmd, capture_output=True, text=True, timeout=CLI_TIMEOUT, check=False,
        cwd="/opt/ai-agriculture/cloud", env=env,
    )

    if proc.returncode != 0:
        raise RuntimeError((proc.stderr or proc.stdout or "CLI failed").strip()[:500])

    text = proc.stdout or ""
    if "{" not in text:
        text = proc.stderr or ""
    start = text.find("{")
    end = text.rfind("}")
    if start < 0 or end < start:
        raise RuntimeError("invalid CLI response (no JSON)")

    result = json.loads(text[start:end + 1])
    for item in reversed(result.get("payloads") or []):
        if isinstance(item, dict) and isinstance(item.get("text"), str) and item["text"].strip():
            return item["text"].strip()

    raise RuntimeError("empty reply from CLI")


# ── HTTP Handler ─────────────────────────────────────────────────────────

class ChatHandler(BaseHTTPRequestHandler):
    server_version = "agri-adapter/2.0"

    def _write_json(self, status: int, payload: dict):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt, *args):
        pass

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def _parse(self):
        try:
            cl = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            cl = 0
        body = self.rfile.read(cl)
        try:
            data = json.loads(body.decode("utf-8") if body else "{}")
        except Exception as e:
            self._write_json(400, {"status": "error", "message": f"invalid json: {e}"})
            return None
        msg = str(data.get("message", "")).strip()
        ctx = data.get("context", {})
        sid = str(data.get("session_id", "default")).strip() or "default"
        if not msg:
            self._write_json(400, {"status": "error", "message": "message required"})
            return None
        return msg, ctx, sid

    def do_POST(self):
        is_stream = self.path == "/api/v1/chat/stream"
        if self.path not in ("/api/v1/chat", "/api/v1/chat/stream"):
            self._write_json(404, {"status": "error", "message": "not found"})
            return

        parsed = self._parse()
        if parsed is None:
            return
        message, context, session_id = parsed

        # Build prompt with history + context
        prompt = build_full_prompt(message, context, session_id)

        try:
            reply = call_openclaw_cli(prompt)
        except Exception as e:
            self._write_json(503, {"status": "error", "message": str(e)[:500]})
            return

        # Save to history
        append_history(session_id, message, reply)

        if is_stream:
            self._send_stream(reply)
        else:
            self._write_json(200, {"reply": reply})

    def _send_stream(self, reply: str):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("X-Accel-Buffering", "no")
        self.end_headers()
        self.wfile.flush()

        for ch in reply:
            try:
                self.wfile.write(
                    f"data: {json.dumps({'text': ch}, ensure_ascii=False)}\n\n".encode()
                )
                self.wfile.flush()
                time.sleep(0.015)
            except Exception:
                return

        try:
            self.wfile.write(f"data: {json.dumps({'done': True})}\n\n".encode())
            self.wfile.flush()
        except Exception:
            pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3000)
    args = parser.parse_args()

    global AGRI_SYSTEM_PROMPT
    AGRI_SYSTEM_PROMPT = load_agri_knowledge()
    print(f"[adapter v2] Knowledge base loaded ({len(AGRI_SYSTEM_PROMPT)} chars)")
    print(f"[adapter v2] Serving on http://{args.host}:{args.port}")

    server = ThreadingHTTPServer((args.host, args.port), ChatHandler)
    server.serve_forever()


if __name__ == "__main__":
    main()
