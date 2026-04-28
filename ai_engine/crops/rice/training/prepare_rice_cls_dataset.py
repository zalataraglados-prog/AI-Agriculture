import argparse
import json
from collections import Counter
from pathlib import Path
from typing import List, Dict, Tuple

import numpy as np
from sklearn.model_selection import train_test_split


def load_class_names(labels_file: str) -> List[str]:
    labels_path = Path(labels_file)
    if not labels_path.exists():
        raise FileNotFoundError(f"labels file not found: {labels_path}")

    with labels_path.open("r", encoding="utf-8") as f:
        class_names = json.load(f)

    if not isinstance(class_names, list) or not class_names:
        raise ValueError("labels.json must be a non-empty JSON list")

    return class_names


def resolve_dataset_root(dataset_root: str) -> Path:
    root = Path(dataset_root)
    images_dir = root / "train" / "images"
    labels_dir = root / "train" / "labels"

    if not images_dir.exists() or not labels_dir.exists():
        raise FileNotFoundError(
            f"dataset root must contain train/images and train/labels, got: {root}"
        )
    return root


def collect_classification_samples(
    dataset_root: str,
    class_names: List[str],
    healthy_class_name: str = "HealthyLeaf"
) -> Tuple[List[Dict], Dict]:
    """
    Convert detection-style annotations into image-level classification labels.

    Rules:
    1. Empty txt -> HealthyLeaf
    2. One or more boxes but same class -> that class
    3. Multiple classes in one image -> use majority class
    """
    dataset_root = resolve_dataset_root(dataset_root)
    images_dir = dataset_root / "train" / "images"
    labels_dir = dataset_root / "train" / "labels"

    if healthy_class_name not in class_names:
        raise ValueError(f"{healthy_class_name} not found in class names")

    healthy_idx = class_names.index(healthy_class_name)

    image_paths = []
    for ext in ["*.jpg", "*.jpeg", "*.png", "*.bmp", "*.webp"]:
        image_paths.extend(images_dir.glob(ext))
    image_paths = sorted(image_paths)

    samples = []
    stats = {
        "total_images_found": 0,
        "missing_label_files": 0,
        "empty_label_as_healthy": 0,
        "multi_class_images": 0,
        "invalid_label_lines": 0,
        "kept_samples": 0
    }

    for img_path in image_paths:
        stats["total_images_found"] += 1
        label_path = labels_dir / f"{img_path.stem}.txt"

        if not label_path.exists():
            stats["missing_label_files"] += 1
            continue

        with label_path.open("r", encoding="utf-8") as f:
            lines = [line.strip() for line in f.readlines() if line.strip()]

        if len(lines) == 0:
            label_idx = healthy_idx
            stats["empty_label_as_healthy"] += 1
        else:
            label_ids = []
            for line in lines:
                parts = line.split()
                if len(parts) < 5:
                    stats["invalid_label_lines"] += 1
                    continue

                try:
                    cls_id = int(parts[0])
                except ValueError:
                    stats["invalid_label_lines"] += 1
                    continue

                if cls_id < 0 or cls_id >= len(class_names):
                    stats["invalid_label_lines"] += 1
                    continue

                label_ids.append(cls_id)

            if len(label_ids) == 0:
                continue

            if len(set(label_ids)) > 1:
                stats["multi_class_images"] += 1

            label_idx = Counter(label_ids).most_common(1)[0][0]

        samples.append({
            "image_path": str(img_path),
            "label": label_idx,
            "label_name": class_names[label_idx]
        })

    stats["kept_samples"] = len(samples)
    return samples, stats


def split_samples(samples: List[Dict], val_ratio: float = 0.2, seed: int = 42):
    labels = [s["label"] for s in samples]
    indices = np.arange(len(samples))

    try:
        train_idx, val_idx = train_test_split(
            indices,
            test_size=val_ratio,
            random_state=seed,
            stratify=labels
        )
    except ValueError:
        train_idx, val_idx = train_test_split(
            indices,
            test_size=val_ratio,
            random_state=seed,
            shuffle=True
        )

    train_samples = [samples[i] for i in train_idx]
    val_samples = [samples[i] for i in val_idx]
    return train_samples, val_samples


def save_json(data, path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)


def main():
    parser = argparse.ArgumentParser(description="Prepare rice leaf classification samples")
    parser.add_argument(
        "--dataset-root",
        required=False,
        default=None,
        help="Path that contains train/images and train/labels"
    )
    parser.add_argument(
        "--labels-file",
        default="models/rice/rice_leaf_classifier/labels.json",
        help="Path to labels.json"
    )
    parser.add_argument(
        "--output-dir",
        default="datasets/outputs/rice/rice_leaf_classifier/data",
        help="Directory to save converted samples"
    )
    parser.add_argument(
        "--val-ratio",
        type=float,
        default=0.2,
        help="Validation split ratio"
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="Random seed"
    )
    parser.add_argument(
        "--healthy-class-name",
        default="HealthyLeaf",
        help="Class name used for empty label files"
    )

    args = parser.parse_args()

    # 濡傛灉娌′紶鍙傛暟锛屽氨鐢ㄩ粯璁よ矾寰勶紙寮€鍙戠敤锛?
    if args.dataset_root is None:
        args.dataset_root = "datasets/raw/rice_leaf_spot_disease_annotated_dataset"


    class_names = load_class_names(args.labels_file)
    samples, stats = collect_classification_samples(
        dataset_root=args.dataset_root,
        class_names=class_names,
        healthy_class_name=args.healthy_class_name
    )

    if len(samples) == 0:
        raise RuntimeError("No usable samples found.")

    train_samples, val_samples = split_samples(
        samples=samples,
        val_ratio=args.val_ratio,
        seed=args.seed
    )

    output_dir = Path(args.output_dir)
    save_json(samples, output_dir / "all_samples.json")
    save_json(train_samples, output_dir / "train_samples.json")
    save_json(val_samples, output_dir / "val_samples.json")
    save_json(stats, output_dir / "prepare_stats.json")

    print("Preparation finished.")
    print(f"All samples:   {len(samples)}")
    print(f"Train samples: {len(train_samples)}")
    print(f"Val samples:   {len(val_samples)}")
    print(f"Saved to:      {output_dir}")


if __name__ == "__main__":
    main()
