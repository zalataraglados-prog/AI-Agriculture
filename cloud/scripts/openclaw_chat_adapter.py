#!/usr/bin/env python3
"""OpenClaw HTTP chat adapter.

Exposes POST /api/v1/chat with JSON {message, context?}
and proxies to OpenClaw CLI local agent.
"""

from __future__ import annotations

import argparse
import json
import subprocess
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class ChatHandler(BaseHTTPRequestHandler):
    server_version = "openclaw-chat-adapter/0.1"

    def _write_json(self, status: int, payload: dict) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args):  # noqa: A003
        # Keep stdout clean; systemd journal still has status via access logs if needed.
        return

    def do_POST(self):  # noqa: N802
        if self.path != "/api/v1/chat":
            self._write_json(HTTPStatus.NOT_FOUND, {"status": "error", "message": "not found"})
            return

        try:
            content_length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            content_length = 0
        body = self.rfile.read(content_length)

        try:
            data = json.loads(body.decode("utf-8") if body else "{}")
        except Exception as exc:  # noqa: BLE001
            self._write_json(HTTPStatus.BAD_REQUEST, {"status": "error", "message": f"invalid json: {exc}"})
            return

        message = str(data.get("message", "")).strip()
        context = data.get("context", {})
        if not message:
            self._write_json(HTTPStatus.BAD_REQUEST, {"status": "error", "message": "message must not be empty"})
            return

        prompt = message
        if context:
            try:
                prompt += "\n\n[context]\n" + json.dumps(context, ensure_ascii=False)
            except Exception:  # noqa: BLE001
                pass

        cmd = [
            "openclaw",
            "agent",
            "--local",
            "--agent",
            "main",
            "--message",
            prompt,
            "--json",
        ]

        try:
            proc = subprocess.run(cmd, capture_output=True, text=True, timeout=120, check=False)
        except subprocess.TimeoutExpired:
            self._write_json(HTTPStatus.GATEWAY_TIMEOUT, {"status": "error", "message": "openclaw request timeout"})
            return
        except Exception as exc:  # noqa: BLE001
            self._write_json(HTTPStatus.SERVICE_UNAVAILABLE, {"status": "error", "message": f"openclaw exec failed: {exc}"})
            return

        if proc.returncode != 0:
            err = (proc.stderr or proc.stdout or "openclaw failed").strip()
            self._write_json(HTTPStatus.SERVICE_UNAVAILABLE, {"status": "error", "message": err[:1000]})
            return

        text = proc.stdout.strip()
        start = text.find("{")
        end = text.rfind("}")
        if start < 0 or end < start:
            self._write_json(HTTPStatus.SERVICE_UNAVAILABLE, {"status": "error", "message": "invalid openclaw response"})
            return

        try:
            result = json.loads(text[start : end + 1])
            payloads = result.get("payloads") or []
            reply = None
            for item in reversed(payloads):
                if isinstance(item, dict) and isinstance(item.get("text"), str) and item["text"].strip():
                    reply = item["text"].strip()
                    break
        except Exception as exc:  # noqa: BLE001
            self._write_json(HTTPStatus.SERVICE_UNAVAILABLE, {"status": "error", "message": f"parse error: {exc}"})
            return

        if not reply:
            self._write_json(HTTPStatus.SERVICE_UNAVAILABLE, {"status": "error", "message": "openclaw response missing reply"})
            return

        self._write_json(HTTPStatus.OK, {"reply": reply})


def main() -> None:
    parser = argparse.ArgumentParser(description="OpenClaw /api/v1/chat adapter")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3000)
    args = parser.parse_args()

    server = ThreadingHTTPServer((args.host, args.port), ChatHandler)
    print(f"[openclaw-chat-adapter] listening on http://{args.host}:{args.port}")
    server.serve_forever()


if __name__ == "__main__":
    main()
