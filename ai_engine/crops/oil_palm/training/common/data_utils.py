"""Data utilities for oil palm training pipelines.

Core principle: raw layer accepts everything, processed/train layer unifies everything.
These utilities operate on the unified processed/train layer, not on raw data directly.
Raw-to-processed conversion is handled by importers in data_importers/.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


def load_manifest(path: str | Path) -> dict[str, Any]:
    """Load and validate a dataset manifest JSON file.

    Args:
        path: Path to the manifest JSON file.

    Returns:
        Parsed manifest dictionary.

    Raises:
        FileNotFoundError: If the manifest file does not exist.
        ValueError: If the manifest is missing required fields.
    """
    manifest_path = Path(path)
    if not manifest_path.exists():
        raise FileNotFoundError(f"Manifest not found: {manifest_path}")

    with open(manifest_path, "r", encoding="utf-8") as f:
        manifest = json.load(f)

    required_fields = ["task", "version", "label_map", "split_strategy"]
    missing = [field for field in required_fields if field not in manifest]
    if missing:
        raise ValueError(f"Manifest {manifest_path.name} missing required fields: {missing}")

    return manifest


def load_labels(path: str | Path) -> list[str]:
    """Load a labels.json file.

    Args:
        path: Path to labels.json.

    Returns:
        List of label strings.

    Raises:
        FileNotFoundError: If the labels file does not exist.
        ValueError: If the labels file is empty or not a list.
    """
    labels_path = Path(path)
    if not labels_path.exists():
        raise FileNotFoundError(f"Labels file not found: {labels_path}")

    with open(labels_path, "r", encoding="utf-8") as f:
        labels = json.load(f)

    if not isinstance(labels, list) or len(labels) == 0:
        raise ValueError(f"Labels file {labels_path.name} must be a non-empty list")

    return labels


def validate_label_map(manifest: dict[str, Any], labels_json_path: str | Path) -> bool:
    """Check that manifest label_map values match labels.json entries.

    Args:
        manifest: Parsed manifest dictionary (must contain 'label_map').
        labels_json_path: Path to the corresponding labels.json file.

    Returns:
        True if labels match.

    Raises:
        ValueError: If there is a mismatch between manifest and labels.json.
    """
    labels = load_labels(labels_json_path)
    manifest_labels = sorted(manifest["label_map"].values())
    file_labels = sorted(labels)

    if manifest_labels != file_labels:
        raise ValueError(
            f"Label mismatch:\n"
            f"  manifest: {manifest_labels}\n"
            f"  labels.json: {file_labels}"
        )

    return True


def verify_split_integrity(split_dir: str | Path) -> dict[str, Any]:
    """Verify that train/val/test splits have no overlap.

    Checks for .txt index files (image paths) in the split directory.
    Each line in a split file should be a relative image path.

    Args:
        split_dir: Path to the splits directory containing train.txt, val.txt, test.txt.

    Returns:
        Dictionary with split statistics and overlap check results.
    """
    split_path = Path(split_dir)
    result: dict[str, Any] = {"valid": True, "splits": {}, "overlaps": {}}

    split_names = ["train", "val", "test"]
    split_sets: dict[str, set[str]] = {}

    for name in split_names:
        txt_file = split_path / f"{name}.txt"
        if txt_file.exists():
            with open(txt_file, "r", encoding="utf-8") as f:
                entries = {line.strip() for line in f if line.strip()}
            split_sets[name] = entries
            result["splits"][name] = len(entries)
        else:
            split_sets[name] = set()
            result["splits"][name] = 0

    # Check pairwise overlaps
    for i, name_a in enumerate(split_names):
        for name_b in split_names[i + 1:]:
            overlap = split_sets[name_a] & split_sets[name_b]
            if overlap:
                result["valid"] = False
                result["overlaps"][f"{name_a}_vs_{name_b}"] = len(overlap)
                logger.warning(
                    "Split overlap detected: %s vs %s — %d entries",
                    name_a, name_b, len(overlap),
                )

    return result


def print_dataset_summary(manifest: dict[str, Any]) -> None:
    """Print a human-readable summary of a dataset manifest.

    Args:
        manifest: Parsed manifest dictionary.
    """
    task = manifest.get("task", "unknown")
    version = manifest.get("version", "unknown")
    sources = manifest.get("sources", [])
    label_map = manifest.get("label_map", {})
    split = manifest.get("split_strategy", {})
    counts = manifest.get("expected_counts", {})

    print(f"\n{'=' * 60}")
    print(f"Dataset Manifest: {task} (v{version})")
    print(f"{'=' * 60}")
    print(f"Labels ({len(label_map)}):")
    for idx, label in sorted(label_map.items(), key=lambda x: int(x[0])):
        print(f"  [{idx}] {label}")
    print(f"\nSources ({len(sources)}):")
    for src in sources:
        print(f"  - {src.get('name', 'unnamed')}")
        if src.get("importer"):
            print(f"    importer: {src['importer']}")
    print(f"\nSplit Strategy: {split.get('method', 'unspecified')}")
    print(f"  Ratios: {split.get('ratios', {})}")
    print(f"  Constraint: {split.get('constraint', 'none')}")
    total = counts.get("total")
    print(f"\nExpected Total: {total if total else 'not yet counted'}")
    print(f"{'=' * 60}\n")
