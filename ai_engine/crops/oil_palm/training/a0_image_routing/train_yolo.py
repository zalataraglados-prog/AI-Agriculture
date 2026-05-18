"""Train the A0 YOLO structure detector.

This module keeps heavy ML imports inside the execution path so repository
tests can import the module without installing the full training stack.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import yaml

PROJECT_ROOT = Path(__file__).resolve().parents[5]
DEFAULT_CONFIG = PROJECT_ROOT / "models" / "oil_palm" / "a0_image_routing" / "training_config.example.yaml"

TRAIN_ARG_KEYS = {
    "data",
    "epochs",
    "imgsz",
    "batch",
    "device",
    "workers",
    "patience",
    "seed",
    "optimizer",
    "cache",
    "project",
    "name",
    "exist_ok",
    "pretrained",
}


def load_training_config(config_path: str | Path) -> dict[str, Any]:
    path = Path(config_path)
    if not path.is_absolute():
        path = PROJECT_ROOT / path
    if not path.exists():
        raise FileNotFoundError(f"Training config not found: {path}")
    with path.open("r", encoding="utf-8") as f:
        config = yaml.safe_load(f) or {}
    if not isinstance(config, dict):
        raise ValueError(f"Training config must be a mapping: {path}")
    return config


def build_train_args(
    config: dict[str, Any],
    *,
    data_yaml: str | None = None,
    run_name: str | None = None,
) -> dict[str, Any]:
    train = dict(config.get("train", {}))
    augment = dict(config.get("augment", {}))

    if data_yaml:
        train["data"] = data_yaml
    if run_name:
        train["name"] = run_name

    if "data" not in train:
        raise ValueError("Training config must define train.data")

    args: dict[str, Any] = {}
    for key, value in train.items():
        if key not in TRAIN_ARG_KEYS or value is None:
            continue
        args[key] = _resolve_path_value(key, value)

    for key, value in augment.items():
        if value is not None:
            args[key] = value

    return args


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Train the oil palm A0 YOLO detector.")
    parser.add_argument(
        "--config",
        default=str(DEFAULT_CONFIG),
        help="Training YAML config.",
    )
    parser.add_argument(
        "--data-yaml",
        default=None,
        help="Override train.data from the config.",
    )
    parser.add_argument(
        "--run-name",
        default=None,
        help="Override the Ultralytics run name.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate config and print train args without starting training.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    config = load_training_config(args.config)
    train_args = build_train_args(
        config,
        data_yaml=args.data_yaml,
        run_name=args.run_name,
    )
    model_name = config.get("model", "yolov8n.pt")

    data_path = Path(str(train_args["data"]))
    data_exists = data_path.exists()
    payload = {
        "task": config.get("task", "a0_image_routing"),
        "model": model_name,
        "data_yaml": str(data_path),
        "data_yaml_exists": data_exists,
        "train_args": train_args,
    }

    if args.dry_run:
        print(json.dumps({"status": "dry_run", **payload}, indent=2, ensure_ascii=False))
        return 0

    if not data_exists:
        raise FileNotFoundError(
            f"YOLO data.yaml not found: {data_path}. Run prepare_dataset.py first."
        )

    try:
        from ultralytics import YOLO
    except ImportError as exc:
        raise RuntimeError(
            "Ultralytics is not installed. Install training dependencies with "
            "`pip install -r requirements/oil-palm-training.txt`."
        ) from exc

    model = YOLO(model_name)
    result = model.train(**train_args)
    print(json.dumps({"status": "ok", **payload, "result": str(result)}, indent=2, ensure_ascii=False))
    return 0


def _resolve_path_value(key: str, value: Any) -> Any:
    if key not in {"data", "project"}:
        return value
    path = Path(str(value))
    if not path.is_absolute():
        path = PROJECT_ROOT / path
    return str(path)


if __name__ == "__main__":
    raise SystemExit(main())
