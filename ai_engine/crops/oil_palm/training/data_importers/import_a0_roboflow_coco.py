"""Import Roboflow COCO exports into the A0 YOLO training layout.

The importer reads the three single-class Roboflow exports used by A0:
fruit bunch, trunk base, and crown region. It keeps the raw source immutable,
maps source labels into the project label set, performs a grouped split, and
writes YOLO detection files for training.
"""

from __future__ import annotations

import csv
import hashlib
import json
import random
import shutil
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from PIL import Image

from ai_engine.crops.oil_palm.training.data_importers.base_importer import (
    BaseImporter,
    ImportResult,
    SENTINEL_FILENAMES,
)

A0_LABELS = ["fruit_bunch", "trunk_base", "crown_region"]
A0_LABEL_TO_ID = {label: idx for idx, label in enumerate(A0_LABELS)}

DEFAULT_SOURCE_LABEL_MAP = {
    "ffb": "fruit_bunch",
    "fruit_bunch": "fruit_bunch",
    "Trunk_base": "trunk_base",
    "trunk_base": "trunk_base",
    "crown_region": "crown_region",
}

DEFAULT_SPLIT_RATIOS = {"train": 0.70, "val": 0.15, "test": 0.15}


@dataclass
class A0Annotation:
    """A normalized bbox annotation before YOLO export."""

    label: str
    bbox_xywh: tuple[float, float, float, float]
    source_category: str


@dataclass
class A0Sample:
    """One image and its mapped annotations."""

    source_dataset: str
    source_image_path: Path
    source_file_name: str
    original_name: str
    width: int
    height: int
    image_hash: str
    group_id: str
    annotations: list[A0Annotation]
    output_file_name: str = ""
    split: str = ""

    @property
    def primary_label(self) -> str:
        return self.annotations[0].label if self.annotations else "empty"


@dataclass
class A0ImportSummary:
    """Detailed conversion summary written to JSON metadata files."""

    dataset_version: str
    source_root: str
    output_root: str
    labels: list[str]
    split_ratios: dict[str, float]
    split_seed: int
    image_count: int = 0
    annotation_count: int = 0
    split_counts: dict[str, int] = field(default_factory=dict)
    split_annotation_counts: dict[str, int] = field(default_factory=dict)
    label_counts: dict[str, int] = field(default_factory=dict)
    label_image_counts: dict[str, int] = field(default_factory=dict)
    skipped_duplicate_images: list[dict[str, str]] = field(default_factory=list)
    clipped_bboxes: int = 0
    skipped_annotations: int = 0
    source_packages: list[dict[str, Any]] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, Any]:
        return {
            "dataset_version": self.dataset_version,
            "source_root": self.source_root,
            "output_root": self.output_root,
            "labels": self.labels,
            "split_ratios": self.split_ratios,
            "split_seed": self.split_seed,
            "image_count": self.image_count,
            "annotation_count": self.annotation_count,
            "split_counts": self.split_counts,
            "split_annotation_counts": self.split_annotation_counts,
            "label_counts": self.label_counts,
            "label_image_counts": self.label_image_counts,
            "skipped_duplicate_images": self.skipped_duplicate_images,
            "clipped_bboxes": self.clipped_bboxes,
            "skipped_annotations": self.skipped_annotations,
            "source_packages": self.source_packages,
            "notes": self.notes,
        }


class A0RoboflowCocoImporter(BaseImporter):
    """Convert A0 Roboflow COCO exports to the project YOLO layout."""

    def __init__(
        self,
        raw_dir: str | Path,
        output_dir: str | Path,
        label_map: dict[str, str] | None = None,
        *,
        dataset_version: str = "roboflow_a0_2026_05_17",
        split_ratios: dict[str, float] | None = None,
        seed: int = 42,
        copy_raw: bool = False,
        overwrite: bool = False,
        dry_run: bool = False,
    ) -> None:
        super().__init__(
            raw_dir=raw_dir,
            output_dir=output_dir,
            label_map=label_map or DEFAULT_SOURCE_LABEL_MAP,
        )
        self.dataset_version = dataset_version
        self.split_ratios = split_ratios or DEFAULT_SPLIT_RATIOS
        self.seed = seed
        self.copy_raw = copy_raw
        self.overwrite = overwrite
        self.dry_run = dry_run
        self.last_summary: A0ImportSummary | None = None

    def convert(self) -> ImportResult:
        """Convert source COCO packages into YOLO train/val/test files."""
        self.validate_raw_dir()
        samples, source_packages = self._load_samples()
        samples, duplicate_records = self._drop_duplicate_images(samples)
        self._assign_output_names(samples)
        self._assign_splits(samples)

        summary = self._build_summary(samples, source_packages, duplicate_records)
        self.last_summary = summary

        if not self.dry_run:
            self._prepare_output_directories()
            if self.copy_raw:
                self._copy_raw_source()
            clipped, skipped = self._write_yolo_dataset(samples)
            summary.clipped_bboxes = clipped
            summary.skipped_annotations = skipped
            summary.annotation_count = sum(summary.label_counts.values()) - skipped
            self._write_metadata(summary, samples)

        return ImportResult(
            source_name=self.dataset_version,
            task="a0_image_routing",
            images_processed=summary.image_count,
            images_skipped=len(summary.skipped_duplicate_images),
            labels_mapped=summary.label_counts,
            output_dir=str(self.output_dir),
            notes=summary.notes,
        )

    def _load_samples(self) -> tuple[list[A0Sample], list[dict[str, Any]]]:
        samples: list[A0Sample] = []
        source_packages: list[dict[str, Any]] = []

        for package_dir in sorted(path for path in self.raw_dir.iterdir() if path.is_dir()):
            annotation_path = package_dir / "train" / "_annotations.coco.json"
            if not annotation_path.exists():
                continue

            with annotation_path.open("r", encoding="utf-8") as f:
                coco = json.load(f)

            categories = {
                item["id"]: item["name"]
                for item in coco.get("categories", [])
                if "id" in item and "name" in item
            }
            images_by_id = {item["id"]: item for item in coco.get("images", [])}
            annotations_by_image: dict[int, list[dict[str, Any]]] = defaultdict(list)
            for annotation in coco.get("annotations", []):
                annotations_by_image[annotation.get("image_id")].append(annotation)

            package_sample_count = 0
            package_annotation_count = 0
            for image_id, image_record in images_by_id.items():
                source_path = package_dir / "train" / image_record["file_name"]
                if not source_path.exists():
                    continue

                mapped_annotations: list[A0Annotation] = []
                for annotation in annotations_by_image.get(image_id, []):
                    source_label = categories.get(annotation.get("category_id"), "")
                    mapped_label = self.map_label(source_label)
                    if mapped_label not in A0_LABEL_TO_ID:
                        continue
                    bbox = _parse_bbox(annotation.get("bbox"))
                    if bbox is None:
                        continue
                    mapped_annotations.append(
                        A0Annotation(
                            label=mapped_label,
                            bbox_xywh=bbox,
                            source_category=source_label,
                        )
                    )

                if not mapped_annotations:
                    continue

                with Image.open(source_path) as image:
                    actual_width, actual_height = image.size
                image_hash = _hash_file(source_path)
                original_name = (
                    image_record.get("extra", {}).get("name")
                    or _strip_roboflow_suffix(image_record["file_name"])
                )
                samples.append(
                    A0Sample(
                        source_dataset=package_dir.name,
                        source_image_path=source_path,
                        source_file_name=image_record["file_name"],
                        original_name=original_name,
                        width=actual_width,
                        height=actual_height,
                        image_hash=image_hash,
                        group_id=_build_group_id(package_dir.name, original_name),
                        annotations=mapped_annotations,
                    )
                )
                package_sample_count += 1
                package_annotation_count += len(mapped_annotations)

            source_packages.append(
                {
                    "name": package_dir.name,
                    "annotation_path": _metadata_path(annotation_path),
                    "images": package_sample_count,
                    "annotations": package_annotation_count,
                    "license": _license_name(coco.get("licenses", [])),
                    "roboflow_url": coco.get("info", {}).get("url", ""),
                    "date_created": coco.get("info", {}).get("date_created", ""),
                }
            )

        if not samples:
            raise ValueError(f"No A0 samples found under {self.raw_dir}")

        return samples, source_packages

    def _drop_duplicate_images(
        self,
        samples: list[A0Sample],
    ) -> tuple[list[A0Sample], list[dict[str, str]]]:
        seen: dict[str, A0Sample] = {}
        unique: list[A0Sample] = []
        duplicates: list[dict[str, str]] = []

        for sample in sorted(samples, key=lambda item: (
            item.primary_label,
            item.group_id,
            item.source_file_name,
        )):
            if sample.image_hash in seen:
                duplicates.append(
                    {
                        "kept": _metadata_path(seen[sample.image_hash].source_image_path),
                        "skipped": _metadata_path(sample.source_image_path),
                        "hash": sample.image_hash,
                    }
                )
                continue
            seen[sample.image_hash] = sample
            unique.append(sample)

        return unique, duplicates

    def _assign_output_names(self, samples: list[A0Sample]) -> None:
        for index, sample in enumerate(
            sorted(samples, key=lambda item: (
                item.primary_label,
                item.group_id,
                item.source_file_name,
            )),
            start=1,
        ):
            sample.output_file_name = f"a0_{index:06d}.jpg"

    def _assign_splits(self, samples: list[A0Sample]) -> None:
        rng = random.Random(self.seed)

        samples_by_label: dict[str, list[A0Sample]] = defaultdict(list)
        for sample in samples:
            samples_by_label[sample.primary_label].append(sample)

        for label, label_samples in samples_by_label.items():
            groups: dict[str, list[A0Sample]] = defaultdict(list)
            for sample in label_samples:
                groups[sample.group_id].append(sample)

            group_items = list(groups.items())
            rng.shuffle(group_items)
            group_items.sort(key=lambda item: -len(item[1]))

            targets = _split_targets(len(label_samples), self.split_ratios)
            assigned_counts = {"train": 0, "val": 0, "test": 0}

            for _group_id, group_samples in group_items:
                split = _choose_split_for_group(targets, assigned_counts)
                for sample in group_samples:
                    sample.split = split
                assigned_counts[split] += len(group_samples)

            missing_splits = [
                split
                for split in ("train", "val", "test")
                if assigned_counts[split] == 0 and len(label_samples) >= 3
            ]
            for split in missing_splits:
                donor = max(("train", "val", "test"), key=lambda name: assigned_counts[name])
                donor_samples = [sample for sample in label_samples if sample.split == donor]
                if len(donor_samples) <= 1:
                    continue
                moved = donor_samples[-1]
                moved.split = split
                assigned_counts[donor] -= 1
                assigned_counts[split] += 1

    def _build_summary(
        self,
        samples: list[A0Sample],
        source_packages: list[dict[str, Any]],
        duplicate_records: list[dict[str, str]],
    ) -> A0ImportSummary:
        split_counts = Counter(sample.split for sample in samples)
        split_annotation_counts = Counter()
        label_counts = Counter()
        label_image_sets: dict[str, set[str]] = defaultdict(set)

        for sample in samples:
            split_annotation_counts[sample.split] += len(sample.annotations)
            for annotation in sample.annotations:
                label_counts[annotation.label] += 1
                label_image_sets[annotation.label].add(sample.output_file_name)

        notes = [
            "Generated from Roboflow COCO exports.",
            "No empty-label negative samples are included in this baseline export.",
            "License is recorded as Unknown until source-specific permissions are documented.",
        ]

        return A0ImportSummary(
            dataset_version=self.dataset_version,
            source_root=_metadata_path(self.raw_dir),
            output_root=_metadata_path(self.output_dir),
            labels=A0_LABELS,
            split_ratios=self.split_ratios,
            split_seed=self.seed,
            image_count=len(samples),
            annotation_count=sum(label_counts.values()),
            split_counts=dict(sorted(split_counts.items())),
            split_annotation_counts=dict(sorted(split_annotation_counts.items())),
            label_counts=dict(sorted(label_counts.items())),
            label_image_counts={
                label: len(label_image_sets[label])
                for label in A0_LABELS
            },
            skipped_duplicate_images=duplicate_records,
            source_packages=source_packages,
            notes=notes,
        )

    def _prepare_output_directories(self) -> None:
        self.output_dir.mkdir(parents=True, exist_ok=True)
        yolo_root = self.output_dir / "yolo"

        image_root = yolo_root / "images"
        label_root = yolo_root / "labels"
        if not self.overwrite:
            for root in (image_root, label_root):
                if _has_real_entries(root):
                    raise FileExistsError(
                        f"{root} already contains generated data. "
                        "Use --overwrite to regenerate it."
                    )
        else:
            _clear_directory_files(image_root)
            _clear_directory_files(label_root)

        for split in ("train", "val", "test"):
            (image_root / split).mkdir(parents=True, exist_ok=True)
            (label_root / split).mkdir(parents=True, exist_ok=True)
        (self.output_dir / "splits").mkdir(parents=True, exist_ok=True)
        (self.output_dir / "licenses").mkdir(parents=True, exist_ok=True)

    def _copy_raw_source(self) -> None:
        raw_target = self.output_dir / "raw" / self.dataset_version
        if raw_target.exists():
            if not self.overwrite:
                raise FileExistsError(
                    f"{raw_target} already exists. Use --overwrite to refresh raw copy."
                )
        raw_target.parent.mkdir(parents=True, exist_ok=True)
        raw_target.mkdir(parents=True, exist_ok=True)

        for package_dir in sorted(path for path in self.raw_dir.iterdir() if path.is_dir()):
            source_train = package_dir / "train"
            source_annotations = source_train / "_annotations.coco.json"
            if not source_annotations.exists():
                continue

            target_train = raw_target / package_dir.name / "train"
            target_train.mkdir(parents=True, exist_ok=True)
            with source_annotations.open("r", encoding="utf-8") as f:
                coco = json.load(f)

            file_map_rows: list[dict[str, str]] = []
            for index, image_record in enumerate(coco.get("images", []), start=1):
                original_file_name = image_record["file_name"]
                source_image = source_train / original_file_name
                if not source_image.exists():
                    continue
                compact_name = f"{package_dir.name}_{index:06d}.jpg"
                shutil.copy2(source_image, target_train / compact_name)
                image_record["file_name"] = compact_name
                image_record.setdefault("extra", {})
                image_record["extra"]["roboflow_file_name"] = original_file_name
                file_map_rows.append(
                    {
                        "compact_file_name": compact_name,
                        "roboflow_file_name": original_file_name,
                    }
                )

            (target_train / "_annotations.coco.json").write_text(
                json.dumps(coco, indent=2, ensure_ascii=False),
                encoding="utf-8",
            )
            with (target_train / "raw_file_map.csv").open("w", encoding="utf-8", newline="") as f:
                writer = csv.DictWriter(
                    f,
                    fieldnames=["compact_file_name", "roboflow_file_name"],
                )
                writer.writeheader()
                writer.writerows(file_map_rows)

    def _write_yolo_dataset(self, samples: list[A0Sample]) -> tuple[int, int]:
        yolo_root = self.output_dir / "yolo"
        clipped_count = 0
        skipped_count = 0

        for sample in samples:
            image_out = yolo_root / "images" / sample.split / sample.output_file_name
            label_out = (
                yolo_root
                / "labels"
                / sample.split
                / f"{Path(sample.output_file_name).stem}.txt"
            )
            shutil.copy2(sample.source_image_path, image_out)

            lines: list[str] = []
            for annotation in sample.annotations:
                clipped, was_clipped = _clip_bbox(annotation.bbox_xywh, sample.width, sample.height)
                if was_clipped:
                    clipped_count += 1
                x, y, width, height = clipped
                if width <= 0 or height <= 0:
                    skipped_count += 1
                    continue
                class_id = A0_LABEL_TO_ID[annotation.label]
                x_center = (x + width / 2.0) / sample.width
                y_center = (y + height / 2.0) / sample.height
                norm_width = width / sample.width
                norm_height = height / sample.height
                lines.append(
                    f"{class_id} "
                    f"{x_center:.6f} {y_center:.6f} "
                    f"{norm_width:.6f} {norm_height:.6f}"
                )

            label_out.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")

        self._write_data_yaml(yolo_root)
        return clipped_count, skipped_count

    def _write_data_yaml(self, yolo_root: Path) -> None:
        lines = [
            f"path: {yolo_root.as_posix()}",
            "train: images/train",
            "val: images/val",
            "test: images/test",
            "",
            "names:",
        ]
        for idx, label in enumerate(A0_LABELS):
            lines.append(f"  {idx}: {label}")
        (yolo_root / "data.yaml").write_text("\n".join(lines) + "\n", encoding="utf-8")

    def _write_metadata(self, summary: A0ImportSummary, samples: list[A0Sample]) -> None:
        summary_dict = summary.as_dict()
        splits_dir = self.output_dir / "splits"
        licenses_dir = self.output_dir / "licenses"
        manifests_dir = self.output_dir.parent / "manifests"

        split_entries = {
            split: [
                f"yolo/images/{sample.split}/{sample.output_file_name}"
                for sample in sorted(samples, key=lambda item: item.output_file_name)
                if sample.split == split
            ]
            for split in ("train", "val", "test")
        }
        for split, entries in split_entries.items():
            (splits_dir / f"{split}.txt").write_text(
                "\n".join(entries) + ("\n" if entries else ""),
                encoding="utf-8",
            )

        (splits_dir / "split_manifest.json").write_text(
            json.dumps(_split_manifest(summary, samples), indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
        (splits_dir / "a0_dataset_summary.json").write_text(
            json.dumps(summary_dict, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
        (splits_dir / "annotation_qa_report.json").write_text(
            json.dumps(_qa_report(summary), indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
        _write_sources_csv(licenses_dir / "sources.csv", summary)
        (self.output_dir / "dataset_card.md").write_text(
            _dataset_card(summary),
            encoding="utf-8",
        )

        manifests_dir.mkdir(parents=True, exist_ok=True)
        (manifests_dir / "a0_image_routing.json").write_text(
            json.dumps(_manifest(summary), indent=2, ensure_ascii=False),
            encoding="utf-8",
        )


def _parse_bbox(value: Any) -> tuple[float, float, float, float] | None:
    if not isinstance(value, list) or len(value) != 4:
        return None
    try:
        x, y, width, height = (float(item) for item in value)
    except (TypeError, ValueError):
        return None
    return x, y, width, height


def _strip_roboflow_suffix(file_name: str) -> str:
    stem = Path(file_name).stem
    return stem.split("_jpg.rf.")[0].split("_jpeg.rf.")[0].split("_png.rf.")[0]


def _build_group_id(source_dataset: str, original_name: str) -> str:
    stem = Path(original_name).stem
    if "_MP4-" in stem:
        return f"{source_dataset}:{stem.split('_MP4-')[0]}_MP4"
    if "_mp4-" in stem:
        return f"{source_dataset}:{stem.split('_mp4-')[0]}_mp4"
    if "-" in stem and stem.rsplit("-", 1)[-1].isdigit():
        return f"{source_dataset}:{stem.rsplit('-', 1)[0]}"
    return f"{source_dataset}:{stem}"


def _hash_file(path: Path) -> str:
    digest = hashlib.md5()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _license_name(licenses: list[dict[str, Any]]) -> str:
    if not licenses:
        return "Unknown"
    return licenses[0].get("name") or "Unknown"


def _metadata_path(path: str | Path) -> str:
    """Return a repo-relative path or an external placeholder for metadata."""
    resolved = Path(path).resolve()
    try:
        return resolved.relative_to(Path.cwd().resolve()).as_posix()
    except ValueError:
        parts = list(resolved.parts)
        if resolved.drive and len(parts) > 1:
            return "<external>/" + "/".join(parts[1:])
        return resolved.as_posix()


def _split_targets(total: int, ratios: dict[str, float]) -> dict[str, int]:
    if total <= 0:
        return {"train": 0, "val": 0, "test": 0}
    val = max(1, round(total * ratios.get("val", 0.15))) if total >= 3 else 0
    test = max(1, round(total * ratios.get("test", 0.15))) if total >= 3 else 0
    train = max(1, total - val - test)
    return {"train": train, "val": val, "test": test}


def _choose_split_for_group(
    targets: dict[str, int],
    assigned_counts: dict[str, int],
) -> str:
    deficits = {
        split: targets[split] - assigned_counts[split]
        for split in ("train", "val", "test")
    }
    return max(("train", "val", "test"), key=lambda split: (deficits[split], split == "train"))


def _clip_bbox(
    bbox: tuple[float, float, float, float],
    image_width: int,
    image_height: int,
) -> tuple[tuple[float, float, float, float], bool]:
    x, y, width, height = bbox
    x1 = max(0.0, min(float(image_width), x))
    y1 = max(0.0, min(float(image_height), y))
    x2 = max(0.0, min(float(image_width), x + width))
    y2 = max(0.0, min(float(image_height), y + height))
    clipped = (x1, y1, max(0.0, x2 - x1), max(0.0, y2 - y1))
    was_clipped = any(
        abs(new_value - old_value) > 1e-6
        for new_value, old_value in zip(clipped, bbox)
    )
    return clipped, was_clipped


def _has_real_entries(path: Path) -> bool:
    if not path.exists():
        return False
    return any(entry.name not in SENTINEL_FILENAMES for entry in path.iterdir())


def _clear_directory_files(path: Path) -> None:
    """Remove files under a generated directory while keeping folders in place."""
    if not path.exists():
        return
    for entry in sorted(path.rglob("*"), key=lambda item: len(item.parts), reverse=True):
        if entry.is_file() and entry.name not in SENTINEL_FILENAMES:
            entry.chmod(0o666)
            entry.unlink()


def _split_manifest(summary: A0ImportSummary, samples: list[A0Sample]) -> dict[str, Any]:
    entries = []
    for sample in sorted(samples, key=lambda item: item.output_file_name):
        entries.append(
            {
                "image": f"yolo/images/{sample.split}/{sample.output_file_name}",
                "label": f"yolo/labels/{sample.split}/{Path(sample.output_file_name).stem}.txt",
                "split": sample.split,
                "group_id": sample.group_id,
                "source_dataset": sample.source_dataset,
                "source_file_name": sample.source_file_name,
                "original_name": sample.original_name,
                "labels": sorted({annotation.label for annotation in sample.annotations}),
                "annotation_count": len(sample.annotations),
            }
        )
    return {
        "task": "a0_image_routing",
        "dataset_version": summary.dataset_version,
        "split_strategy": {
            "method": "grouped_by_source_dataset_and_original_media",
            "ratios": summary.split_ratios,
            "seed": summary.split_seed,
            "constraint": "Frames or near-duplicate images from the same source group stay in one split.",
        },
        "counts": {
            "images": summary.image_count,
            "annotations": summary.annotation_count,
            "splits": summary.split_counts,
            "labels": summary.label_counts,
        },
        "entries": entries,
    }


def _qa_report(summary: A0ImportSummary) -> dict[str, Any]:
    return {
        "task": "a0_image_routing",
        "dataset_version": summary.dataset_version,
        "checks": {
            "labels_match_project_standard": summary.labels == A0_LABELS,
            "contains_unknown_label": "unknown" in summary.labels,
            "negative_empty_label_images": 0,
            "duplicate_images_skipped": len(summary.skipped_duplicate_images),
            "clipped_bboxes": summary.clipped_bboxes,
            "skipped_annotations_after_clipping": summary.skipped_annotations,
            "license_status": "Unknown",
        },
        "notes": summary.notes,
    }


def _write_sources_csv(path: Path, summary: A0ImportSummary) -> None:
    with path.open("w", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(
            f,
            fieldnames=[
                "dataset_version",
                "source_name",
                "source_path",
                "roboflow_url",
                "license",
                "images",
                "annotations",
                "notes",
            ],
        )
        writer.writeheader()
        for source in summary.source_packages:
            writer.writerow(
                {
                    "dataset_version": summary.dataset_version,
                    "source_name": source["name"],
                    "source_path": source["annotation_path"],
                    "roboflow_url": source.get("roboflow_url", ""),
                    "license": source.get("license") or "Unknown",
                    "images": source["images"],
                    "annotations": source["annotations"],
                    "notes": "Roboflow COCO export; license pending verification",
                }
            )


def _dataset_card(summary: A0ImportSummary) -> str:
    return f"""# A0 Image Routing Dataset

## Status

Local YOLO training dataset generated from Roboflow COCO exports.

## Labels

0. `fruit_bunch`
1. `trunk_base`
2. `crown_region`

`unknown` is not a YOLO class. This baseline does not include empty-label
negative samples yet.

## Counts

- Images: {summary.image_count}
- Annotations: {summary.annotation_count}
- Split counts: {json.dumps(summary.split_counts, ensure_ascii=False)}
- Label counts: {json.dumps(summary.label_counts, ensure_ascii=False)}
- Duplicate images skipped: {len(summary.skipped_duplicate_images)}
- Bboxes clipped to image bounds: {summary.clipped_bboxes}

## Sources And License

Source exports are stored locally under `{summary.source_root}` and copied into
the ignored `raw/` layer when `--copy-raw` is used. License is currently
recorded as `Unknown`; verify source permissions before publishing trained
weights.

## Notes

- Generated version: `{summary.dataset_version}`
- Split method: grouped by source dataset and original media name.
- A0 detects structure candidates only; it does not infer maturity, disease, or
  tree-level conclusions.
"""


def _manifest(summary: A0ImportSummary) -> dict[str, Any]:
    return {
        "task": "a0_image_routing",
        "version": summary.dataset_version,
        "description": "A0 structure detection and routing dataset generated from Roboflow COCO exports",
        "internal_training_standard": {
            "format": "yolo_detection",
            "annotation_type": "bbox",
            "image_size": None,
            "coordinate_system": "normalized_xywh",
        },
        "sources": [
            {
                "name": source["name"],
                "format": "roboflow_coco",
                "annotation_path": source["annotation_path"],
                "license": source.get("license") or "Unknown",
                "url": source.get("roboflow_url", ""),
                "images": source["images"],
                "annotations": source["annotations"],
                "importer": "import_a0_roboflow_coco.py",
            }
            for source in summary.source_packages
        ],
        "label_map": {str(idx): label for idx, label in enumerate(A0_LABELS)},
        "label_notes": (
            "A0 detects supported oil palm structures and proposes bbox candidates "
            "for user confirmation. unknown is not a YOLO class."
        ),
        "split_strategy": {
            "method": "grouped_by_source_dataset_and_original_media",
            "ratios": summary.split_ratios,
            "seed": summary.split_seed,
            "constraint": "Images from the same source media group stay in the same split",
        },
        "expected_counts": {
            "train": summary.split_counts.get("train", 0),
            "val": summary.split_counts.get("val", 0),
            "test": summary.split_counts.get("test", 0),
            "total": summary.image_count,
            "annotations": summary.annotation_count,
            "labels": summary.label_counts,
        },
        "importer_directory": "ai_engine/crops/oil_palm/training/data_importers/",
        "notes": summary.notes,
    }
