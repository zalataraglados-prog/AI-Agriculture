#!/usr/bin/env python3
"""Chat stress CLI with hot frequency change.

Features:
- Continuous POST /api/v1/chat pressure
- Shared control file for live rate change
- Stair mode: every N seconds decrease interval by delta, never stop
"""

from __future__ import annotations

import argparse
import json
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path


def load_control(path: Path, default_interval: float) -> dict:
    if path.exists():
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except Exception:
            pass
    state = {"interval_sec": default_interval, "stop": False, "message": "health check"}
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")
    return state


def save_control(path: Path, state: dict) -> None:
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


def post_chat(url: str, message: str, timeout: float) -> tuple[bool, float, str]:
    body = json.dumps({"message": message, "context": {"source": "chat_stress_cli"}}).encode("utf-8")
    req = urllib.request.Request(url, data=body, method="POST")
    req.add_header("Content-Type", "application/json")
    start = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            payload = resp.read().decode("utf-8", errors="replace")
            elapsed_ms = (time.perf_counter() - start) * 1000.0
            return True, elapsed_ms, payload[:160]
    except urllib.error.HTTPError as exc:
        elapsed_ms = (time.perf_counter() - start) * 1000.0
        return False, elapsed_ms, f"HTTP {exc.code}"
    except Exception as exc:
        elapsed_ms = (time.perf_counter() - start) * 1000.0
        return False, elapsed_ms, str(exc)


def run(args: argparse.Namespace) -> None:
    control_file = Path(args.control_file)
    state = load_control(control_file, args.interval_sec)

    if args.stair_enable:
        def stair_loop() -> None:
            while True:
                time.sleep(args.stair_every_sec)
                s = load_control(control_file, args.interval_sec)
                if s.get("stop"):
                    return
                cur = float(s.get("interval_sec", args.interval_sec))
                nxt = max(args.min_interval_sec, cur - args.stair_delta_sec)
                s["interval_sec"] = nxt
                save_control(control_file, s)
                print(f"[stair] interval_sec {cur:.3f} -> {nxt:.3f}")

        threading.Thread(target=stair_loop, daemon=True).start()

    n = 0
    ok = 0
    fail = 0
    while True:
        s = load_control(control_file, args.interval_sec)
        if s.get("stop"):
            print("[run] stop=true, exit")
            return

        interval = max(args.min_interval_sec, float(s.get("interval_sec", args.interval_sec)))
        message = str(s.get("message", args.message))

        succ, ms, detail = post_chat(args.url, message, args.timeout_sec)
        n += 1
        if succ:
            ok += 1
            print(f"[{n}] ok {ms:.1f}ms interval={interval:.3f}s {detail}")
        else:
            fail += 1
            print(f"[{n}] fail {ms:.1f}ms interval={interval:.3f}s {detail}")

        if args.report_every > 0 and n % args.report_every == 0:
            rate = (ok / n) * 100.0
            print(f"[report] total={n} ok={ok} fail={fail} success_rate={rate:.1f}%")

        time.sleep(interval)


def set_rate(args: argparse.Namespace) -> None:
    p = Path(args.control_file)
    s = load_control(p, args.interval_sec if args.interval_sec else 1.0)
    if args.interval_sec is not None:
        s["interval_sec"] = max(0.05, args.interval_sec)
    if args.message is not None:
        s["message"] = args.message
    save_control(p, s)
    print(json.dumps(s, ensure_ascii=False))


def stop(args: argparse.Namespace) -> None:
    p = Path(args.control_file)
    s = load_control(p, 1.0)
    s["stop"] = True
    save_control(p, s)
    print("stopped")


def show(args: argparse.Namespace) -> None:
    p = Path(args.control_file)
    s = load_control(p, 1.0)
    print(json.dumps(s, ensure_ascii=False, indent=2))


def reset(args: argparse.Namespace) -> None:
    p = Path(args.control_file)
    s = {"interval_sec": args.interval_sec, "stop": False, "message": args.message}
    save_control(p, s)
    print(json.dumps(s, ensure_ascii=False, indent=2))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Chat stress CLI")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_run = sub.add_parser("run", help="run stress loop")
    p_run.add_argument("--url", default="http://127.0.0.1:9001/api/v1/chat")
    p_run.add_argument("--message", default="health check")
    p_run.add_argument("--timeout-sec", type=float, default=60.0)
    p_run.add_argument("--interval-sec", type=float, default=1.0)
    p_run.add_argument("--min-interval-sec", type=float, default=0.05)
    p_run.add_argument("--control-file", default="/tmp/chat_stress_control.json")
    p_run.add_argument("--report-every", type=int, default=20)
    p_run.add_argument("--stair-enable", action="store_true")
    p_run.add_argument("--stair-every-sec", type=float, default=5.0)
    p_run.add_argument("--stair-delta-sec", type=float, default=0.1)
    p_run.set_defaults(func=run)

    p_set = sub.add_parser("set-rate", help="hot change interval/message")
    p_set.add_argument("--control-file", default="/tmp/chat_stress_control.json")
    p_set.add_argument("--interval-sec", type=float)
    p_set.add_argument("--message")
    p_set.set_defaults(func=set_rate)

    p_stop = sub.add_parser("stop", help="stop running loop")
    p_stop.add_argument("--control-file", default="/tmp/chat_stress_control.json")
    p_stop.set_defaults(func=stop)

    p_show = sub.add_parser("show", help="show current control state")
    p_show.add_argument("--control-file", default="/tmp/chat_stress_control.json")
    p_show.set_defaults(func=show)

    p_reset = sub.add_parser("reset", help="reset control state")
    p_reset.add_argument("--control-file", default="/tmp/chat_stress_control.json")
    p_reset.add_argument("--interval-sec", type=float, default=1.0)
    p_reset.add_argument("--message", default="health check")
    p_reset.set_defaults(func=reset)

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
