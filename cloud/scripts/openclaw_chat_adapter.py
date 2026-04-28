#!/usr/bin/env python3
"""OpenClaw HTTP chat adapter.

Exposes POST /api/v1/chat with JSON {message, context?}
and proxies to OpenClaw CLI local agent.

Optimization goals:
- keep adapter process hot (no Python cold start per request)
- bounded worker pool for CLI calls
- optional warmup on startup to reduce first-request latency
"""

from __future__ import annotations

import argparse
import json
import os
import queue
import subprocess
import threading
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

SYSTEM_HINT = (
    "You are operating on the cloud host as root-level operator. "
    "Prefer AI-ag whitelist commands for status checks. "
    "Do not confuse gateway registration with sensor telemetry registration: "
    "gateway/device registration is at device level, sensor_id is telemetry payload dimension."
)


class OpenClawAdapter:
    def __init__(self, timeout_sec: float, workers: int, cwd: str) -> None:
        self.timeout_sec = timeout_sec
        self.cwd = cwd
        self.workers = max(1, workers)
        self.env = dict(os.environ)
        self.env.setdefault("HOME", "/root")
        self.jobs: "queue.Queue[tuple[list[str], queue.Queue[tuple[bool, Any]]]]" = queue.Queue()
        self._threads: list[threading.Thread] = []

        for idx in range(self.workers):
            t = threading.Thread(
                target=self._worker_loop,
                name=f"openclaw-worker-{idx}",
                daemon=True,
            )
            t.start()
            self._threads.append(t)

    def _run_once(self, cmd: list[str]) -> str:
        try:
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=self.timeout_sec,
                check=False,
                cwd=self.cwd,
                env=self.env,
            )
        except subprocess.TimeoutExpired as exc:
            raise TimeoutError("openclaw request timeout") from exc
        except Exception as exc:  # noqa: BLE001
            raise RuntimeError(f"openclaw exec failed: {exc}") from exc

        if proc.returncode != 0:
            err = (proc.stderr or proc.stdout or "openclaw failed").strip()
            raise RuntimeError(err[:1000])

        text = (proc.stdout or "").strip()
        if "{" not in text:
            text = (proc.stderr or "").strip()
        start = text.find("{")
        end = text.rfind("}")
        if start < 0 or end < start:
            raise RuntimeError("invalid openclaw response")

        try:
            result = json.loads(text[start : end + 1])
            payloads = result.get("payloads") or []
            reply = None
            for item in reversed(payloads):
                if isinstance(item, dict) and isinstance(item.get("text"), str) and item["text"].strip():
                    reply = item["text"].strip()
                    break
        except Exception as exc:  # noqa: BLE001
            raise RuntimeError(f"parse error: {exc}") from exc

        if not reply:
            raise RuntimeError("openclaw response missing reply")
        return reply

    def _worker_loop(self) -> None:
        while True:
            cmd, result_q = self.jobs.get()
            try:
                result_q.put((True, self._run_once(cmd)))
            except Exception as exc:  # noqa: BLE001
                result_q.put((False, exc))
            finally:
                self.jobs.task_done()

    def call_openclaw(self, cmd: list[str]) -> str:
        result_q: "queue.Queue[tuple[bool, Any]]" = queue.Queue(maxsize=1)
        self.jobs.put((cmd, result_q))
        try:
            ok, payload = result_q.get(timeout=self.timeout_sec + 2.0)
        except queue.Empty as exc:
            raise TimeoutError("openclaw queue timeout") from exc

        if ok:
            return payload
        if isinstance(payload, TimeoutError):
            raise payload
        if isinstance(payload, RuntimeError):
            raise payload
        raise RuntimeError(str(payload))

    def warmup(self) -> None:
        warmup_cmd = [
            "openclaw",
            "agent",
            "--local",
            "--agent",
            "main",
            "--message",
            "[system]\\nWarmup ping, reply with short ok.",
            "--json",
        ]
        start = time.time()
        try:
            self.call_openclaw(warmup_cmd)
            print(f"[openclaw-chat-adapter] warmup ok in {(time.time() - start):.2f}s")
        except Exception as exc:  # noqa: BLE001
            print(f"[openclaw-chat-adapter] warmup skipped: {exc}")


class ChatHandler(BaseHTTPRequestHandler):
    server_version = "openclaw-chat-adapter/0.2"

    def _write_json(self, status: int, payload: dict) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args):  # noqa: A003
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

        prompt = f"[system]\\n{SYSTEM_HINT}\\n\\n[user]\\n{message}"
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
            reply = self.server.adapter.call_openclaw(cmd)  # type: ignore[attr-defined]
        except TimeoutError:
            self._write_json(HTTPStatus.GATEWAY_TIMEOUT, {"status": "error", "message": "openclaw request timeout"})
            return
        except RuntimeError as exc:
            self._write_json(HTTPStatus.SERVICE_UNAVAILABLE, {"status": "error", "message": str(exc)})
            return

        self._write_json(HTTPStatus.OK, {"reply": reply})


def main() -> None:
    parser = argparse.ArgumentParser(description="OpenClaw /api/v1/chat adapter")
    parser.add_argument("--host", default=os.getenv("CHAT_ADAPTER_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.getenv("CHAT_ADAPTER_PORT", "3000")))
    parser.add_argument("--timeout-sec", type=float, default=float(os.getenv("CHAT_ADAPTER_TIMEOUT_SEC", "180")))
    parser.add_argument("--workers", type=int, default=int(os.getenv("CHAT_ADAPTER_WORKERS", "2")))
    parser.add_argument("--cwd", default=os.getenv("CHAT_ADAPTER_CWD", "/opt/ai-agriculture/cloud"))
    parser.add_argument("--no-warmup", action="store_true", help="Disable startup warmup request.")
    args = parser.parse_args()

    server = ThreadingHTTPServer((args.host, args.port), ChatHandler)
    server.adapter = OpenClawAdapter(
        timeout_sec=max(5.0, args.timeout_sec),
        workers=max(1, args.workers),
        cwd=args.cwd,
    )
    print(f"[openclaw-chat-adapter] listening on http://{args.host}:{args.port}")
    print(
        f"[openclaw-chat-adapter] workers={max(1, args.workers)} timeout_sec={max(5.0, args.timeout_sec)} cwd={args.cwd}"
    )
    if not args.no_warmup:
        server.adapter.warmup()
    server.serve_forever()


if __name__ == "__main__":
    main()
