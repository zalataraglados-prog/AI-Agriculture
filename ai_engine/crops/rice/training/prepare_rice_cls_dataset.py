from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any


DEFAULT_CLASS_NAMES = [
    "Bacterial_Leaf_Blight",
    "Brown_Spot",
    "HealthyLeaf",
    "Leaf_Blast",
]


def collect_classification_samples(
    dataset_root: str | Path,
    class_names: list[str],
    healthy_class_name: str = "HealthyLeaf",
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    """Convert YOLO detection labels into image-level classification samples."""

    dataset_root = Path(dataset_root)
    images_dir = dataset_root / "train" / "images"
    labels_dir = dataset_root / "train" / "labels"

    if not images_dir.exists():
        raise FileNotFoundError(f"images directory not found: {images_dir}")
    if not labels_dir.exists():
        raise FileNotFoundError(f"labels directory not found: {labels_dir}")
    if healthy_class_name not in class_names:
        raise ValueError(f"healthy class not found in class_names: {healthy_class_name}")

    healthy_class_idx = class_names.index(healthy_class_name)
    image_paths: list[Path] = []
    for pattern in ("*.jpg", "*.jpeg", "*.png", "*.bmp", "*.webp"):
        image_paths.extend(images_dir.glob(pattern))

    stats = {
        "total_images_found": 0,
        "missing_label_files": 0,
        "empty_label_as_healthy": 0,
        "multi_class_images": 0,
        "invalid_label_lines": 0,
        "kept_samples": 0,
    }
    samples: list[dict[str, Any]] = []

    for image_path in sorted(image_paths):
        stats["total_images_found"] += 1
        label_path = labels_dir / f"{image_path.stem}.txt"
        if not label_path.exists():
            stats["missing_label_files"] += 1
            continue

        lines = [line.strip() for line in label_path.read_text(encoding="utf-8").splitlines() if line.strip()]
        if not lines:
            label_idx = healthy_class_idx
            stats["empty_label_as_healthy"] += 1
        else:
            label_ids: list[int] = []
            for line in lines:
                parts = line.split()
                if len(parts) < 5:
                    stats["invalid_label_lines"] += 1
                    continue
                try:
                    class_id = int(parts[0])
                except ValueError:
                    stats["invalid_label_lines"] += 1
                    continue
                if class_id < 0 or class_id >= len(class_names):
                    stats["invalid_label_lines"] += 1
                    continue
                label_ids.append(class_id)

            if not label_ids:
                continue
            if len(set(label_ids)) > 1:
                stats["multi_class_images"] += 1
            label_idx = Counter(label_ids).most_common(1)[0][0]

        samples.append(
            {
                "image_path": str(image_path),
                "label": label_idx,
                "label_name": class_names[label_idx],
            }
        )

    stats["kept_samples"] = len(samples)
    return samples, stats


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Collect rice classification samples from a YOLO-style train/images + train/labels dataset.",
    )
    parser.add_argument("--dataset-root", required=True, help="Dataset root containing train/images and train/labels.")
    parser.add_argument("--healthy-class-name", default="HealthyLeaf")
    parser.add_argument("--class-names", nargs="*", default=DEFAULT_CLASS_NAMES)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    samples, stats = collect_classification_samples(
        dataset_root=args.dataset_root,
        class_names=list(args.class_names),
        healthy_class_name=args.healthy_class_name,
    )
    print(json.dumps({"status": "ok", "stats": stats, "sample_count": len(samples)}, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
