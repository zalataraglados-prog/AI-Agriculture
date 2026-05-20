"""Roboflow YOLO importer for oil palm FFB maturity datasets.

This importer normalizes one known Roboflow export into the project-level
FFB YOLO label order:

    flower, unripe, underripe, ripe, overripe, abnormal

The source-to-target mapping below was recorded from the teammate handoff and
must still be confirmed against the exact exported dataset version before the
metrics are treated as production evidence.
"""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any


DEFAULT_RAW_DIR = Path("datasets/oil_palm/ffb_maturity/raw/roboflow_v1")
DEFAULT_OUTPUT_DIR = Path("datasets/oil_palm/ffb_maturity/yolo")
DEFAULT_SUMMARY_FILE = Path("datasets/oil_palm/ffb_maturity/splits/roboflow_v1_summary.json")

STANDARD_LABELS = ["flower", "unripe", "underripe", "ripe", "overripe", "abnormal"]

# Source Roboflow class id -> internal YOLO class id.
SOURCE_TO_INTERNAL_CLASS = {
    "0": "4",  # overripe
    "1": "3",  # ripe
    "2": "2",  # underripe
    "3": "1",  # unripe
}

IMAGE_SUFFIXES = {".jpg", ".jpeg", ".png", ".bmp", ".webp"}


class RoboflowYoloImporter:
    """Convert a Roboflow YOLO export into the project FFB YOLO layout."""

    def __init__(
        self,
        raw_dir: str | Path = DEFAULT_RAW_DIR,
        output_dir: str | Path = DEFAULT_OUTPUT_DIR,
        summary_file: str | Path | None = DEFAULT_SUMMARY_FILE,
        overwrite: bool = False,
        label_map: dict[str, str] | None = None,
    ) -> None:
        self.raw_dir = Path(raw_dir)
        self.output_dir = Path(output_dir)
        self.summary_file = Path(summary_file) if summary_file else None
        self.overwrite = overwrite
        self.label_map = label_map or SOURCE_TO_INTERNAL_CLASS

    def convert(self) -> dict[str, Any]:
        """Run conversion and return a summary dictionary."""
        if not self.raw_dir.exists():
            raise FileNotFoundError(f"Raw directory does not exist: {self.raw_dir}")

        self._prepare_output_dir()

        summary: dict[str, Any] = {
            "task": "ffb_maturity",
            "source_name": "roboflow_v1",
            "raw_dir": str(self.raw_dir),
            "output_dir": str(self.output_dir),
            "label_map": self.label_map,
            "labels": STANDARD_LABELS,
            "splits": {},
            "notes": [
                "Source class mapping must be confirmed against the exact Roboflow export.",
                "flower and abnormal are internal standard labels but are not present in this source map.",
            ],
        }

        for split in ["train", "valid", "test"]:
            split_summary = self._convert_split(split)
            if split_summary["source_exists"]:
                summary["splits"][split] = split_summary

        self._write_data_yaml()
        if self.summary_file:
            self.summary_file.parent.mkdir(parents=True, exist_ok=True)
            self.summary_file.write_text(
                json.dumps(summary, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
        return summary

    def _prepare_output_dir(self) -> None:
        if self.output_dir.exists() and self._has_real_contents(self.output_dir):
            if not self.overwrite:
                raise FileExistsError(
                    f"Output directory already has data: {self.output_dir}. "
                    "Pass --overwrite to replace it."
                )
            shutil.rmtree(self.output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

    @staticmethod
    def _has_real_contents(path: Path) -> bool:
        return any(child.name != ".gitkeep" for child in path.iterdir())

    def _convert_split(self, split: str) -> dict[str, Any]:
        split_dir = self.raw_dir / split
        split_summary = {
            "source_exists": split_dir.exists(),
            "images_copied": 0,
            "label_files_seen": 0,
            "annotations_kept": 0,
            "annotations_dropped_unknown_label": 0,
            "annotations_dropped_malformed": 0,
            "annotations_dropped_invalid_bbox": 0,
            "unknown_source_labels": {},
        }
        if not split_dir.exists():
            return split_summary

        out_images = self.output_dir / split / "images"
        out_labels = self.output_dir / split / "labels"
        out_images.mkdir(parents=True, exist_ok=True)
        out_labels.mkdir(parents=True, exist_ok=True)

        raw_images = split_dir / "images"
        if raw_images.exists():
            for image_path in sorted(raw_images.iterdir()):
                if image_path.is_file() and image_path.suffix.lower() in IMAGE_SUFFIXES:
                    shutil.copy2(image_path, out_images / image_path.name)
                    split_summary["images_copied"] += 1

        raw_labels = split_dir / "labels"
        if raw_labels.exists():
            for label_path in sorted(raw_labels.glob("*.txt")):
                file_summary = self._convert_label_file(label_path, out_labels / label_path.name)
                split_summary["label_files_seen"] += 1
                for key in [
                    "annotations_kept",
                    "annotations_dropped_unknown_label",
                    "annotations_dropped_malformed",
                    "annotations_dropped_invalid_bbox",
                ]:
                    split_summary[key] += file_summary[key]
                for label, count in file_summary["unknown_source_labels"].items():
                    split_summary["unknown_source_labels"][label] = (
                        split_summary["unknown_source_labels"].get(label, 0) + count
                    )

        return split_summary

    def _convert_label_file(self, source_path: Path, target_path: Path) -> dict[str, Any]:
        summary = {
            "annotations_kept": 0,
            "annotations_dropped_unknown_label": 0,
            "annotations_dropped_malformed": 0,
            "annotations_dropped_invalid_bbox": 0,
            "unknown_source_labels": {},
        }
        output_lines: list[str] = []

        with source_path.open("r", encoding="utf-8") as source:
            for line in source:
                parts = line.strip().split()
                if not parts:
                    continue
                if len(parts) != 5:
                    summary["annotations_dropped_malformed"] += 1
                    continue

                source_class = parts[0]
                internal_class = self.label_map.get(source_class)
                if internal_class is None:
                    summary["annotations_dropped_unknown_label"] += 1
                    summary["unknown_source_labels"][source_class] = (
                        summary["unknown_source_labels"].get(source_class, 0) + 1
                    )
                    continue

                if not self._is_valid_yolo_bbox(parts[1:]):
                    summary["annotations_dropped_invalid_bbox"] += 1
                    continue

                output_lines.append(" ".join([internal_class, *parts[1:]]) + "\n")
                summary["annotations_kept"] += 1

        target_path.write_text("".join(output_lines), encoding="utf-8")
        return summary

    @staticmethod
    def _is_valid_yolo_bbox(values: list[str]) -> bool:
        try:
            x_center, y_center, width, height = [float(value) for value in values]
        except ValueError:
            return False

        return (
            0.0 <= x_center <= 1.0
            and 0.0 <= y_center <= 1.0
            and 0.0 < width <= 1.0
            and 0.0 < height <= 1.0
        )

    def _write_data_yaml(self) -> None:
        names = ", ".join(f"'{label}'" for label in STANDARD_LABELS)
        yaml_content = (
            "train: ./train/images\n"
            "val: ./valid/images\n"
            "test: ./test/images\n\n"
            f"nc: {len(STANDARD_LABELS)}\n"
            f"names: [{names}]\n"
        )
        (self.output_dir / "data.yaml").write_text(yaml_content, encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Convert Roboflow FFB YOLO labels.")
    parser.add_argument("--raw-dir", default=str(DEFAULT_RAW_DIR))
    parser.add_argument("--output-dir", default=str(DEFAULT_OUTPUT_DIR))
    parser.add_argument("--summary-file", default=str(DEFAULT_SUMMARY_FILE))
    parser.add_argument("--overwrite", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    importer = RoboflowYoloImporter(
        raw_dir=args.raw_dir,
        output_dir=args.output_dir,
        summary_file=args.summary_file,
        overwrite=args.overwrite,
    )
    summary = importer.convert()
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
