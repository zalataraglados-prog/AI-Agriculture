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
import re
import subprocess
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

SYSTEM_HINT = (
    "You are operating on the cloud host as root-level operator. "
    "Prefer AI-ag whitelist commands for status checks. "
    "When tool_context is provided, treat it as authoritative structured agriculture data. "
    "Do not confuse gateway registration with sensor telemetry registration: "
    "gateway/device registration is at device level, sensor_id is telemetry payload dimension."
)

TREE_CODE_RE = re.compile(r"\bOP-\d{3,}\b", re.IGNORECASE)
PLANTATION_ID_RE = re.compile(
    r"(?:plantation[_\s-]*id|plantation|种植园|园区)\s*[=:：#]?\s*(\d+)",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class ToolRequest:
    name: str
    path: str
    params: dict[str, Any]


class OpenClawAdapter:
    def __init__(
        self,
        timeout_sec: float,
        workers: int,
        cwd: str,
        cloud_tool_base_url: str,
        tool_timeout_sec: float,
        max_tool_context_chars: int,
        default_plantation_id: str | None,
    ) -> None:
        self.timeout_sec = timeout_sec
        self.cwd = cwd
        self.workers = max(1, workers)
        self.cloud_tool_base_url = cloud_tool_base_url.rstrip("/")
        self.tool_timeout_sec = max(1.0, tool_timeout_sec)
        self.max_tool_context_chars = max(1000, max_tool_context_chars)
        self.default_plantation_id = default_plantation_id
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

    def build_tool_context(self, message: str, context: Any) -> dict[str, Any] | None:
        requests = select_tool_requests(message, context, self.default_plantation_id)
        if not requests:
            return None

        results = []
        for item in requests:
            try:
                results.append(
                    {
                        "request": {
                            "name": item.name,
                            "path": item.path,
                            "params": item.params,
                        },
                        "response": self.fetch_tool(item),
                    }
                )
            except Exception as exc:  # noqa: BLE001
                results.append(
                    {
                        "request": {
                            "name": item.name,
                            "path": item.path,
                            "params": item.params,
                        },
                        "response": {
                            "status": "error",
                            "message": str(exc),
                        },
                    }
                )

        return {
            "source": "cloud_openclaw_tools",
            "read_only": True,
            "results": results,
        }

    def fetch_tool(self, item: ToolRequest) -> dict[str, Any]:
        query = urllib.parse.urlencode(item.params)
        url = f"{self.cloud_tool_base_url}{item.path}"
        if query:
            url = f"{url}?{query}"

        req = urllib.request.Request(
            url,
            method="GET",
            headers={"Accept": "application/json"},
        )
        try:
            with urllib.request.urlopen(req, timeout=self.tool_timeout_sec) as resp:  # noqa: S310
                body = resp.read()
                status = resp.status
        except urllib.error.HTTPError as exc:
            body = exc.read()
            status = exc.code
        except urllib.error.URLError as exc:
            raise RuntimeError(f"tool request failed: {exc.reason}") from exc

        try:
            payload = json.loads(body.decode("utf-8") if body else "{}")
        except Exception as exc:  # noqa: BLE001
            raise RuntimeError(f"tool returned invalid json: {exc}") from exc

        if status >= 400:
            message = payload.get("message") if isinstance(payload, dict) else None
            raise RuntimeError(message or f"tool returned HTTP {status}")
        if not isinstance(payload, dict):
            raise RuntimeError("tool returned non-object json")
        return payload


class ChatHandler(BaseHTTPRequestHandler):
    server_version = "openclaw-chat-adapter/0.3"

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
        tool_context = self.server.adapter.build_tool_context(message, context)  # type: ignore[attr-defined]
        if tool_context:
            prompt += "\n\n[tool_context]\n" + compact_tool_context(
                tool_context,
                self.server.adapter.max_tool_context_chars,  # type: ignore[attr-defined]
            )

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


def select_tool_requests(message: str, context: Any, default_plantation_id: str | None = None) -> list[ToolRequest]:
    text = message or ""
    lowered = text.lower()
    tree_code = extract_tree_code(text)
    plantation_id = extract_plantation_id(text, context, default_plantation_id)
    requests: list[ToolRequest] = []

    if tree_code:
        if has_any(lowered, ["缺", "证据", "missing", "evidence", "补拍", "补充"]):
            requests.append(ToolRequest("query_missing_evidence", "/missing-evidence", {"tree_code": tree_code}))
        elif has_any(lowered, ["timeline", "时间线", "历史", "坐标历史"]):
            requests.append(ToolRequest("query_tree_timeline", "/tree-timeline", {"tree_code": tree_code, "limit": 20}))
        else:
            requests.append(ToolRequest("query_tree_profile", "/tree-profile", {"tree_code": tree_code, "limit": 10}))

    if has_any(lowered, ["巡检", "patrol", "优先", "priority"]):
        if plantation_id:
            requests.append(
                ToolRequest("generate_patrol_report", "/patrol-report", {"plantation_id": plantation_id, "limit": 50})
            )
    elif has_any(lowered, ["plantation", "种植园", "园区", "dashboard", "报告", "report"]):
        if plantation_id:
            requests.append(
                ToolRequest(
                    "query_plantation_report",
                    "/plantation-report",
                    {"plantation_id": plantation_id, "limit": 50},
                )
            )

    return dedupe_tool_requests(requests)


def extract_tree_code(text: str) -> str | None:
    match = TREE_CODE_RE.search(text or "")
    return match.group(0).upper() if match else None


def extract_plantation_id(text: str, context: Any, default_plantation_id: str | None = None) -> str | None:
    match = PLANTATION_ID_RE.search(text or "")
    if match:
        return match.group(1)
    if isinstance(context, dict):
        for key in ["plantation_id", "plantationId"]:
            value = context.get(key)
            if isinstance(value, int) or (isinstance(value, str) and value.isdigit()):
                return str(value)
    if default_plantation_id and default_plantation_id.isdigit():
        return default_plantation_id
    return None


def has_any(text: str, terms: list[str]) -> bool:
    return any(term.lower() in text for term in terms)


def dedupe_tool_requests(items: list[ToolRequest]) -> list[ToolRequest]:
    seen: set[tuple[str, tuple[tuple[str, str], ...]]] = set()
    out: list[ToolRequest] = []
    for item in items:
        key = (item.name, tuple(sorted((k, str(v)) for k, v in item.params.items())))
        if key in seen:
            continue
        seen.add(key)
        out.append(item)
    return out


def compact_tool_context(payload: dict[str, Any], max_chars: int) -> str:
    text = json.dumps(payload, ensure_ascii=False)
    if len(text) <= max_chars:
        return text
    return json.dumps(
        {
            "source": payload.get("source", "cloud_openclaw_tools"),
            "read_only": True,
            "truncated": True,
            "content": text[: max(0, max_chars - 200)],
        },
        ensure_ascii=False,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="OpenClaw /api/v1/chat adapter")
    parser.add_argument("--host", default=os.getenv("CHAT_ADAPTER_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.getenv("CHAT_ADAPTER_PORT", "3000")))
    parser.add_argument("--timeout-sec", type=float, default=float(os.getenv("CHAT_ADAPTER_TIMEOUT_SEC", "180")))
    parser.add_argument("--workers", type=int, default=int(os.getenv("CHAT_ADAPTER_WORKERS", "2")))
    parser.add_argument("--cwd", default=os.getenv("CHAT_ADAPTER_CWD", "/opt/ai-agriculture/cloud"))
    parser.add_argument(
        "--cloud-tool-base-url",
        default=os.getenv("CLOUD_TOOL_BASE_URL", "http://127.0.0.1:8088/api/v1/openclaw/tools"),
    )
    parser.add_argument("--tool-timeout-sec", type=float, default=float(os.getenv("CLOUD_TOOL_TIMEOUT_SEC", "5")))
    parser.add_argument(
        "--max-tool-context-chars",
        type=int,
        default=int(os.getenv("CLOUD_TOOL_CONTEXT_MAX_CHARS", "12000")),
    )
    parser.add_argument("--default-plantation-id", default=os.getenv("OPENCLAW_DEFAULT_PLANTATION_ID"))
    parser.add_argument("--no-warmup", action="store_true", help="Disable startup warmup request.")
    args = parser.parse_args()

    server = ThreadingHTTPServer((args.host, args.port), ChatHandler)
    server.adapter = OpenClawAdapter(
        timeout_sec=max(5.0, args.timeout_sec),
        workers=max(1, args.workers),
        cwd=args.cwd,
        cloud_tool_base_url=args.cloud_tool_base_url,
        tool_timeout_sec=max(1.0, args.tool_timeout_sec),
        max_tool_context_chars=max(1000, args.max_tool_context_chars),
        default_plantation_id=args.default_plantation_id,
    )
    print(f"[openclaw-chat-adapter] listening on http://{args.host}:{args.port}")
    print(
        f"[openclaw-chat-adapter] workers={max(1, args.workers)} timeout_sec={max(5.0, args.timeout_sec)} cwd={args.cwd}"
    )
    print(
        "[openclaw-chat-adapter] "
        f"cloud_tool_base_url={args.cloud_tool_base_url} tool_timeout_sec={max(1.0, args.tool_timeout_sec)}"
    )
    if not args.no_warmup:
        server.adapter.warmup()
    server.serve_forever()


if __name__ == "__main__":
    main()
