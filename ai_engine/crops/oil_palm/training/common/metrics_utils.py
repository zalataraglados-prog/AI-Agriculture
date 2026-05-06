"""Metrics utilities for oil palm model training and evaluation.

Provides helpers to save, load, and compare model metrics in a
standardized JSON format consistent with metrics.example.json templates.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


def save_metrics(metrics_dict: dict[str, Any], output_path: str | Path) -> Path:
    """Save metrics dictionary to a JSON file.

    Args:
        metrics_dict: Metrics to save. Should follow the structure in metrics.example.json.
        output_path: Destination file path.

    Returns:
        The Path object of the saved file.
    """
    out = Path(output_path)
    out.parent.mkdir(parents=True, exist_ok=True)
    with open(out, "w", encoding="utf-8") as f:
        json.dump(metrics_dict, f, indent=2, ensure_ascii=False)
    logger.info("Metrics saved to %s", out)
    return out


def load_metrics(path: str | Path) -> dict[str, Any]:
    """Load metrics from a JSON file.

    Args:
        path: Path to the metrics JSON file.

    Returns:
        Parsed metrics dictionary.

    Raises:
        FileNotFoundError: If the metrics file does not exist.
    """
    metrics_path = Path(path)
    if not metrics_path.exists():
        raise FileNotFoundError(f"Metrics file not found: {metrics_path}")

    with open(metrics_path, "r", encoding="utf-8") as f:
        return json.load(f)


def compare_metrics(
    old_path: str | Path,
    new_path: str | Path,
) -> dict[str, Any]:
    """Compare two metrics files and report differences.

    Args:
        old_path: Path to the baseline metrics JSON.
        new_path: Path to the new metrics JSON.

    Returns:
        Dictionary containing the comparison results with keys:
        - old_version: model_version from old metrics
        - new_version: model_version from new metrics
        - changes: dict of metric_name -> {old, new, delta} for numeric metrics
    """
    old = load_metrics(old_path)
    new = load_metrics(new_path)

    result: dict[str, Any] = {
        "old_version": old.get("model_version", "unknown"),
        "new_version": new.get("model_version", "unknown"),
        "changes": {},
    }

    # Compare top-level numeric metrics
    old_metrics = old.get("metrics", {})
    new_metrics = new.get("metrics", {})

    for key in sorted(set(old_metrics.keys()) | set(new_metrics.keys())):
        old_val = old_metrics.get(key)
        new_val = new_metrics.get(key)
        if isinstance(old_val, (int, float)) and isinstance(new_val, (int, float)):
            delta = round(new_val - old_val, 6)
            result["changes"][key] = {
                "old": old_val,
                "new": new_val,
                "delta": delta,
                "improved": delta > 0,
            }

    return result
