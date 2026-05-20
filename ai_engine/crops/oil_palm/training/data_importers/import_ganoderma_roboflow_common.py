"""Shared Roboflow CSV importer for Ganoderma risk classification datasets."""

from __future__ import annotations

import argparse
import csv
import json
import random
import shutil
from pathlib import Path
from typing import Any, Callable


ACTIVE_LABELS = ["healthy", "suspected_risk"]
STANDARD_LABELS = ["healthy", "suspected_risk", "other_stress_unknown"]
DEFAULT_OUTPUT_DIR = Path("datasets/oil_palm/ganoderma_risk/classification")
DEFAULT_SPLIT_RATIOS = {"train": 0.70, "val": 0.15, "test": 0.15}
IMAGE_SUFFIXES = {".jpg", ".jpeg", ".png", ".bmp", ".webp"}


class GanodermaRoboflowImporter:
    """Import one Roboflow CSV classification export into active class folders."""

    def __init__(
        self,
        source_name: str,
        raw_dir: str | Path,
        output_dir: str | Path = DEFAULT_OUTPUT_DIR,
        map_label: Callable[[dict[str, str]], str | None] | None = None,
        split_ratios: dict[str, float] | None = None,
        seed: int = 42,
        summary_file: str | Path | None = None,
        overwrite: bool = False,
    ) -> None:
        self.source_name = source_name
        self.raw_dir = Path(raw_dir)
        self.output_dir = Path(output_dir)
        self.map_label = map_label or (lambda _row: None)
        self.split_ratios = split_ratios or DEFAULT_SPLIT_RATIOS
        self.seed = seed
        self.summary_file = Path(summary_file) if summary_file else None
        self.overwrite = overwrite

    def convert(self) -> dict[str, Any]:
        if not self.raw_dir.exists():
            raise FileNotFoundError(f"Raw directory does not exist: {self.raw_dir}")
        if self.overwrite and self.output_dir.exists():
            shutil.rmtree(self.output_dir)

        csv_path = self._find_csv_path()
        image_dir = self.raw_dir / "train"
        items, skipped = self._read_items(csv_path, image_dir)
        splits = self._split_items(items)

        summary: dict[str, Any] = {
            "task": "ganoderma_risk",
            "source_name": self.source_name,
            "raw_dir": str(self.raw_dir),
            "output_dir": str(self.output_dir),
            "seed": self.seed,
            "split_ratios": self.split_ratios,
            "active_labels": ACTIVE_LABELS,
            "reserved_labels": ["other_stress_unknown"],
            "input_rows": len(items) + skipped["missing_filename"] + skipped["missing_image"] + skipped["unknown_label"],
            "skipped": skipped,
            "splits": {},
            "notes": [
                "The project label set reserves other_stress_unknown, but v1 importers only populate active labels.",
                "Run all Ganoderma source importers before training so both healthy and suspected_risk are represented.",
            ],
        }

        for split, split_items in splits.items():
            split_summary = self._copy_split(split, split_items, image_dir)
            summary["splits"][split] = split_summary

        if self.summary_file:
            self.summary_file.parent.mkdir(parents=True, exist_ok=True)
            self.summary_file.write_text(
                json.dumps(summary, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
        return summary

    def _find_csv_path(self) -> Path:
        train_dir = self.raw_dir / "train"
        for name in ["_classes.csv", "classes.csv"]:
            path = train_dir / name
            if path.exists():
                return path
        raise FileNotFoundError(f"Cannot find _classes.csv or classes.csv under {train_dir}")

    def _read_items(self, csv_path: Path, image_dir: Path) -> tuple[list[tuple[str, str]], dict[str, int]]:
        items: list[tuple[str, str]] = []
        skipped = {"missing_filename": 0, "missing_image": 0, "unknown_label": 0}

        with csv_path.open(newline="", encoding="utf-8") as source:
            reader = csv.DictReader(source)
            for row in reader:
                filename = (self._get_value(row, "filename") or "").strip()
                if not filename:
                    skipped["missing_filename"] += 1
                    continue
                if not (image_dir / filename).exists():
                    skipped["missing_image"] += 1
                    continue

                label = self.map_label(row)
                if label not in ACTIVE_LABELS:
                    skipped["unknown_label"] += 1
                    continue
                items.append((filename, label))

        return items, skipped

    def _split_items(self, items: list[tuple[str, str]]) -> dict[str, list[tuple[str, str]]]:
        rng = random.Random(self.seed)
        by_label: dict[str, list[str]] = {label: [] for label in ACTIVE_LABELS}
        for filename, label in items:
            by_label[label].append(filename)

        splits: dict[str, list[tuple[str, str]]] = {"train": [], "val": [], "test": []}
        for label, filenames in by_label.items():
            rng.shuffle(filenames)
            count = len(filenames)
            train_count = int(count * self.split_ratios["train"])
            val_count = int(count * self.split_ratios["val"])
            split_ranges = {
                "train": filenames[:train_count],
                "val": filenames[train_count:train_count + val_count],
                "test": filenames[train_count + val_count:],
            }
            for split, split_filenames in split_ranges.items():
                splits[split].extend((filename, label) for filename in split_filenames)
        return splits

    def _copy_split(
        self,
        split: str,
        items: list[tuple[str, str]],
        image_dir: Path,
    ) -> dict[str, Any]:
        summary = {
            "copied": 0,
            "per_label": {label: 0 for label in ACTIVE_LABELS},
            "collision_renamed": 0,
        }
        for label in ACTIVE_LABELS:
            (self.output_dir / split / label).mkdir(parents=True, exist_ok=True)

        for filename, label in items:
            source = image_dir / filename
            target = self._target_path(split, label, filename)
            if target.exists():
                summary["collision_renamed"] += 1
                target = self._deduplicated_target_path(split, label, filename)
            shutil.copy2(source, target)
            summary["copied"] += 1
            summary["per_label"][label] += 1
        return summary

    def _target_path(self, split: str, label: str, filename: str) -> Path:
        return self.output_dir / split / label / filename

    def _deduplicated_target_path(self, split: str, label: str, filename: str) -> Path:
        stem = Path(filename).stem
        suffix = Path(filename).suffix
        source_slug = self.source_name.lower().replace(" ", "_")
        index = 1
        while True:
            target = self.output_dir / split / label / f"{source_slug}_{index}_{stem}{suffix}"
            if not target.exists():
                return target
            index += 1

    @staticmethod
    def _get_value(row: dict[str, str], key: str) -> str | None:
        if key in row:
            return row[key]
        normalized = {name.strip().lower(): value for name, value in row.items()}
        return normalized.get(key.strip().lower())


def csv_int(row: dict[str, str], key: str) -> int:
    value = GanodermaRoboflowImporter._get_value(row, key)
    if value is None or str(value).strip() == "":
        return 0
    return int(float(str(value).strip()))


def build_arg_parser(default_raw_dir: str, default_summary_file: str) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Import a Ganoderma Roboflow CSV export.")
    parser.add_argument("--raw-dir", default=default_raw_dir)
    parser.add_argument("--output-dir", default=str(DEFAULT_OUTPUT_DIR))
    parser.add_argument("--summary-file", default=default_summary_file)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--overwrite", action="store_true")
    return parser
