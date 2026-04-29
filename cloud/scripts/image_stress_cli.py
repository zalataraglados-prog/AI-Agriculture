#!/usr/bin/env python3
import argparse
import json
import os
import random
import sys
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from typing import Dict, Tuple

DEFAULT_ENDPOINT = "http://127.0.0.1:8088/api/v1/image/upload"
DEFAULT_CONTROL_FILE = ".image_stress_control.json"
DEFAULT_INTERVAL_SEC = 5.0
DEFAULT_TIMEOUT_SEC = 15.0

# 1x1 png
DEFAULT_IMAGE_BYTES = bytes([
    0x89,0x50,0x4E,0x47,0x0D,0x0A,0x1A,0x0A,0x00,0x00,0x00,0x0D,0x49,0x48,0x44,0x52,
    0x00,0x00,0x00,0x01,0x00,0x00,0x00,0x01,0x08,0x04,0x00,0x00,0x00,0xB5,0x1C,0x0C,
    0x02,0x00,0x00,0x00,0x0B,0x49,0x44,0x41,0x54,0x78,0xDA,0x63,0xFC,0xFF,0x1F,0x00,
    0x03,0x03,0x02,0x00,0xEF,0xA2,0x63,0xDB,0x00,0x00,0x00,0x00,0x49,0x45,0x4E,0x44,
    0xAE,0x42,0x60,0x82,
])


def utc_now_rfc3339() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load_control(path: str) -> Dict:
    if not os.path.exists(path):
        return {}
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def save_control(path: str, payload: Dict) -> None:
    parent = os.path.dirname(os.path.abspath(path))
    if parent and not os.path.exists(parent):
        os.makedirs(parent, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(payload, f, ensure_ascii=False, indent=2)


def resolve_interval(args_interval: float, args_hz: float) -> float:
    if args_hz is not None:
        if args_hz <= 0:
            raise ValueError("--hz must be > 0")
        return 1.0 / args_hz
    if args_interval is None:
        return DEFAULT_INTERVAL_SEC
    if args_interval <= 0:
        raise ValueError("--interval-sec must be > 0")
    return args_interval


def encode_multipart(field_name: str, filename: str, body: bytes, content_type: str) -> Tuple[bytes, str]:
    boundary = "----image-stress-{}".format(random.randint(100000, 999999))
    head = (
        "--{b}\r\n"
        "Content-Disposition: form-data; name=\"{n}\"; filename=\"{f}\"\r\n"
        "Content-Type: {ct}\r\n\r\n"
    ).format(b=boundary, n=field_name, f=filename, ct=content_type).encode("utf-8")
    tail = "\r\n--{}--\r\n".format(boundary).encode("utf-8")
    return head + body + tail, boundary


def post_image(endpoint: str, device_id: str, location: str, crop_type: str, farm_note: str,
               image_bytes: bytes, image_name: str, timeout_sec: float) -> Tuple[bool, str]:
    ts = utc_now_rfc3339()
    query = urllib.parse.urlencode({
        "device_id": device_id,
        "ts": ts,
        "location": location,
        "crop_type": crop_type,
        "farm_note": farm_note,
    })
    url = endpoint + ("&" if "?" in endpoint else "?") + query

    body, boundary = encode_multipart("file", image_name, image_bytes, "image/png")
    req = urllib.request.Request(url=url, data=body, method="POST")
    req.add_header("Content-Type", "multipart/form-data; boundary={}".format(boundary))
    req.add_header("Content-Length", str(len(body)))

    try:
        with urllib.request.urlopen(req, timeout=timeout_sec) as resp:
            payload = resp.read().decode("utf-8", errors="replace")
            ok = resp.status == 200 and '"status":"success"' in payload.replace(" ", "")
            return ok, payload
    except Exception as exc:
        return False, "error: {}".format(exc)


def cmd_set_rate(args: argparse.Namespace) -> int:
    interval = resolve_interval(args.interval_sec, args.hz)
    payload = load_control(args.control_file)
    payload.setdefault("stopped", False)
    payload["interval_sec"] = interval
    payload["updated_at"] = utc_now_rfc3339()
    save_control(args.control_file, payload)
    print("interval_sec={:.6f}".format(interval))
    return 0


def cmd_show_rate(args: argparse.Namespace) -> int:
    payload = load_control(args.control_file)
    interval = float(payload.get("interval_sec", DEFAULT_INTERVAL_SEC))
    stopped = bool(payload.get("stopped", False))
    hz = (1.0 / interval) if interval > 0 else 0.0
    print(json.dumps({
        "control_file": os.path.abspath(args.control_file),
        "interval_sec": interval,
        "hz": hz,
        "stopped": stopped,
        "updated_at": payload.get("updated_at"),
    }, ensure_ascii=False, indent=2))
    return 0


def cmd_stop(args: argparse.Namespace) -> int:
    payload = load_control(args.control_file)
    payload["stopped"] = True
    payload["updated_at"] = utc_now_rfc3339()
    save_control(args.control_file, payload)
    print("stopped=true")
    return 0


def cmd_run(args: argparse.Namespace) -> int:
    try:
        start_interval = resolve_interval(args.interval_sec, args.hz)
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2

    if args.image_path:
        with open(args.image_path, "rb") as f:
            image_bytes = f.read()
        image_name = os.path.basename(args.image_path) or "upload.png"
    else:
        image_bytes = DEFAULT_IMAGE_BYTES
        image_name = "stress.png"

    control = load_control(args.control_file)
    control["interval_sec"] = float(control.get("interval_sec", start_interval))
    control["stopped"] = False
    control["updated_at"] = utc_now_rfc3339()
    save_control(args.control_file, control)

    total = 0
    ok_count = 0
    fail_count = 0

    print("[image-stress] start endpoint={} device_id={} control_file={}".format(
        args.endpoint, args.device_id, os.path.abspath(args.control_file)
    ))
    print("[image-stress] initial interval_sec={:.6f} (hz={:.4f})".format(
        control["interval_sec"], 1.0 / control["interval_sec"]
    ))

    while True:
        control = load_control(args.control_file)
        if control.get("stopped", False):
            print("[image-stress] stopped by control file")
            break

        interval_sec = float(control.get("interval_sec", start_interval))
        if interval_sec <= 0:
            interval_sec = start_interval

        t0 = time.time()
        ok, msg = post_image(
            endpoint=args.endpoint,
            device_id=args.device_id,
            location=args.location,
            crop_type=args.crop_type,
            farm_note=args.farm_note,
            image_bytes=image_bytes,
            image_name=image_name,
            timeout_sec=args.timeout_sec,
        )

        total += 1
        if ok:
            ok_count += 1
            state = "ok"
        else:
            fail_count += 1
            state = "fail"

        if len(msg) > 180:
            msg = msg[:180] + "..."

        print("[image-stress] #{} {} interval={:.3f}s ok={} fail={} {}".format(
            total, state, interval_sec, ok_count, fail_count, msg
        ))

        if args.max_uploads is not None and total >= args.max_uploads:
            print("[image-stress] reached max_uploads={}".format(args.max_uploads))
            break

        elapsed = time.time() - t0
        sleep_for = max(0.0, interval_sec - elapsed)
        time.sleep(sleep_for)

    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="image-stress",
        description="Image upload stress tester with hot rate changes.",
    )
    sub = p.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="start stress loop")
    run.add_argument("--endpoint", default=DEFAULT_ENDPOINT)
    run.add_argument("--device-id", required=True)
    run.add_argument("--location", default="stress_lab")
    run.add_argument("--crop-type", default="rice")
    run.add_argument("--farm-note", default="stress")
    run.add_argument("--image-path", default="")
    run.add_argument("--interval-sec", type=float, default=DEFAULT_INTERVAL_SEC)
    run.add_argument("--hz", type=float, default=None)
    run.add_argument("--timeout-sec", type=float, default=DEFAULT_TIMEOUT_SEC)
    run.add_argument("--max-uploads", type=int, default=None)
    run.add_argument("--control-file", default=DEFAULT_CONTROL_FILE)
    run.set_defaults(func=cmd_run)

    set_rate = sub.add_parser("set-rate", help="hot change frequency")
    set_rate.add_argument("--interval-sec", type=float, default=None)
    set_rate.add_argument("--hz", type=float, default=None)
    set_rate.add_argument("--control-file", default=DEFAULT_CONTROL_FILE)
    set_rate.set_defaults(func=cmd_set_rate)

    show = sub.add_parser("show-rate", help="show current control state")
    show.add_argument("--control-file", default=DEFAULT_CONTROL_FILE)
    show.set_defaults(func=cmd_show_rate)

    stop = sub.add_parser("stop", help="request running loop to stop")
    stop.add_argument("--control-file", default=DEFAULT_CONTROL_FILE)
    stop.set_defaults(func=cmd_stop)

    return p


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
