#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import statistics
import subprocess
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from typing import Any


DEFAULT_ENDPOINT = "http://127.0.0.1:8088/api/v1/image/upload"
DEFAULT_TIMEOUT_SEC = 20.0


def utc_now_rfc3339() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def encode_multipart(field_name: str, filename: str, body: bytes, content_type: str) -> tuple[bytes, str]:
    boundary = f"----video-frame-poc-{int(time.time() * 1000)}"
    head = (
        f"--{boundary}\r\n"
        f"Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
        f"Content-Type: {content_type}\r\n\r\n"
    ).encode("utf-8")
    tail = f"\r\n--{boundary}--\r\n".encode("utf-8")
    return head + body + tail, boundary


def post_image(
    endpoint: str,
    device_id: str,
    location: str,
    crop_type: str,
    farm_note: str,
    image_bytes: bytes,
    image_name: str,
    timeout_sec: float,
) -> tuple[bool, float, str]:
    ts = utc_now_rfc3339()
    query = urllib.parse.urlencode(
        {
            "device_id": device_id,
            "ts": ts,
            "location": location,
            "crop_type": crop_type,
            "farm_note": farm_note,
        }
    )
    url = endpoint + ("&" if "?" in endpoint else "?") + query
    body, boundary = encode_multipart("file", image_name, image_bytes, "image/png")
    req = urllib.request.Request(url=url, data=body, method="POST")
    req.add_header("Content-Type", f"multipart/form-data; boundary={boundary}")
    req.add_header("Content-Length", str(len(body)))
    start = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=timeout_sec) as resp:
            payload = resp.read().decode("utf-8", errors="replace")
            ok = resp.status == 200 and '"status":"success"' in payload.replace(" ", "")
            return ok, (time.perf_counter() - start) * 1000.0, payload[:240]
    except Exception as exc:  # noqa: BLE001
        return False, (time.perf_counter() - start) * 1000.0, f"error: {exc}"


def ensure_ffmpeg() -> None:
    if shutil.which("ffmpeg") is None or shutil.which("ffprobe") is None:
        raise RuntimeError("ffmpeg/ffprobe not found. Please install ffmpeg first.")


def probe_duration(video_path: str) -> float:
    cmd = [
        "ffprobe",
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "json",
        video_path,
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"ffprobe failed: {proc.stderr.strip()}")
    data = json.loads(proc.stdout or "{}")
    duration = float(data.get("format", {}).get("duration", 0.0))
    if duration <= 0:
        raise RuntimeError("unable to probe video duration")
    return duration


def extract_frame_png(video_path: str, sec: float) -> bytes:
    cmd = [
        "ffmpeg",
        "-loglevel",
        "error",
        "-ss",
        f"{sec:.3f}",
        "-i",
        video_path,
        "-frames:v",
        "1",
        "-f",
        "image2pipe",
        "-vcodec",
        "png",
        "-",
    ]
    proc = subprocess.run(cmd, capture_output=True, check=False)
    if proc.returncode != 0 or not proc.stdout:
        err = proc.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"ffmpeg extract failed at {sec:.3f}s: {err}")
    return proc.stdout


def pct(values: list[float], q: float) -> float:
    if not values:
        return 0.0
    idx = int((len(values) - 1) * q)
    return sorted(values)[idx]


def run(args: argparse.Namespace) -> int:
    ensure_ffmpeg()
    if not os.path.isfile(args.video_path):
        raise RuntimeError(f"video not found: {args.video_path}")
    if args.frame_interval_sec <= 0:
        raise RuntimeError("--frame-interval-sec must be > 0")
    duration = probe_duration(args.video_path)
    total_planned = int(math.floor(duration / args.frame_interval_sec)) + 1
    if args.max_frames and args.max_frames > 0:
        total_planned = min(total_planned, args.max_frames)

    print(
        f"[video-poc] start video={args.video_path} duration={duration:.2f}s interval={args.frame_interval_sec:.2f}s planned_frames={total_planned}"
    )
    print(f"[video-poc] endpoint={args.endpoint} device_id={args.device_id}")

    upload_lat_ms: list[float] = []
    extract_lat_ms: list[float] = []
    ok_count = 0
    fail_count = 0
    t0 = time.perf_counter()

    for i in range(total_planned):
        sec = min(i * args.frame_interval_sec, max(duration - 0.01, 0))
        x0 = time.perf_counter()
        try:
            png = extract_frame_png(args.video_path, sec)
        except Exception as exc:  # noqa: BLE001
            fail_count += 1
            print(f"[{i+1}/{total_planned}] extract_fail t={sec:.2f}s err={exc}")
            continue
        x_ms = (time.perf_counter() - x0) * 1000.0
        extract_lat_ms.append(x_ms)

        ok, up_ms, detail = post_image(
            endpoint=args.endpoint,
            device_id=args.device_id,
            location=args.location,
            crop_type=args.crop_type,
            farm_note=args.farm_note,
            image_bytes=png,
            image_name=f"frame_{i:04d}.png",
            timeout_sec=args.timeout_sec,
        )
        upload_lat_ms.append(up_ms)
        if ok:
            ok_count += 1
            print(f"[{i+1}/{total_planned}] ok t={sec:.2f}s extract={x_ms:.1f}ms upload={up_ms:.1f}ms")
        else:
            fail_count += 1
            print(f"[{i+1}/{total_planned}] upload_fail t={sec:.2f}s upload={up_ms:.1f}ms detail={detail}")

    elapsed = max(time.perf_counter() - t0, 1e-9)
    sent = ok_count + fail_count
    fps = sent / elapsed
    summary: dict[str, Any] = {
        "status": "done",
        "video_path": os.path.abspath(args.video_path),
        "duration_sec": round(duration, 3),
        "frame_interval_sec": args.frame_interval_sec,
        "planned_frames": total_planned,
        "sent_frames": sent,
        "success": ok_count,
        "failed": fail_count,
        "success_rate": round((ok_count / sent) if sent else 0.0, 4),
        "elapsed_sec": round(elapsed, 3),
        "throughput_fps": round(fps, 3),
        "extract_ms": {
            "avg": round(statistics.mean(extract_lat_ms), 2) if extract_lat_ms else 0.0,
            "p50": round(pct(extract_lat_ms, 0.50), 2) if extract_lat_ms else 0.0,
            "p95": round(pct(extract_lat_ms, 0.95), 2) if extract_lat_ms else 0.0,
            "p99": round(pct(extract_lat_ms, 0.99), 2) if extract_lat_ms else 0.0,
        },
        "upload_ms": {
            "avg": round(statistics.mean(upload_lat_ms), 2) if upload_lat_ms else 0.0,
            "p50": round(pct(upload_lat_ms, 0.50), 2) if upload_lat_ms else 0.0,
            "p95": round(pct(upload_lat_ms, 0.95), 2) if upload_lat_ms else 0.0,
            "p99": round(pct(upload_lat_ms, 0.99), 2) if upload_lat_ms else 0.0,
        },
    }
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    if args.report_file:
        parent = os.path.dirname(os.path.abspath(args.report_file))
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(args.report_file, "w", encoding="utf-8") as f:
            json.dump(summary, f, ensure_ascii=False, indent=2)
        print(f"[video-poc] report saved to {os.path.abspath(args.report_file)}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Video frame upload PoC CLI")
    p.add_argument("--video-path", required=True, help="Input video file path")
    p.add_argument("--endpoint", default=DEFAULT_ENDPOINT)
    p.add_argument("--device-id", default="dev_video_poc_01")
    p.add_argument("--location", default="video_lab")
    p.add_argument("--crop-type", default="rice")
    p.add_argument("--farm-note", default="video frame poc")
    p.add_argument("--frame-interval-sec", type=float, default=2.0)
    p.add_argument("--max-frames", type=int, default=0, help="0 means no cap")
    p.add_argument("--timeout-sec", type=float, default=DEFAULT_TIMEOUT_SEC)
    p.add_argument("--report-file", default="")
    return p


def main() -> int:
    args = build_parser().parse_args()
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
