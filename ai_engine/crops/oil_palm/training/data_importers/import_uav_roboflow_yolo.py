"""Import Roboflow YOLO exports into the UAV crown training layout."""

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

import yaml
from PIL import Image

from ai_engine.crops.oil_palm.training.data_importers.base_importer import (
    BaseImporter,
    ImportResult,
    SENTINEL_FILENAMES,
)

UAV_LABELS = ["oil_palm_crown"]
UAV_LABEL_TO_ID = {"oil_palm_crown": 0}
DEFAULT_SPLIT_RATIOS = {"train": 0.70, "val": 0.15, "test": 0.15}
IMAGE_EXTENSIONS = {".jpg", ".jpeg", ".png", ".bmp", ".tif", ".tiff", ".webp"}


@dataclass
class UAVAnnotation:
    """One normalized YOLO bbox before export."""

    bbox_xywh: tuple[float, float, float, float]
    source_class_id: int
    source_label: str


@dataclass
class UAVSample:
    """One UAV tile image and its annotations."""

    source_image_path: Path
    source_label_path: Path | None
    source_file_name: str
    width: int
    height: int
    image_hash: str
    group_id: str
    annotations: list[UAVAnnotation]
    output_file_name: str = ""
    split: str = ""


@dataclass
class UAVImportSummary:
    """Detailed conversion summary written as tracked metadata."""

    dataset_version: str
    source_root: str
    output_root: str
    labels: list[str]
    split_ratios: dict[str, float]
    split_seed: int
    split_grouping: str
    image_count: int = 0
    annotation_count: int = 0
    split_counts: dict[str, int] = field(default_factory=dict)
    split_annotation_counts: dict[str, int] = field(default_factory=dict)
    label_counts: dict[str, int] = field(default_factory=dict)
    empty_label_images: int = 0
    clipped_bboxes: int = 0
    skipped_annotations: int = 0
    source_class_counts: dict[str, int] = field(default_factory=dict)
    source_labels_seen: dict[str, str] = field(default_factory=dict)
    source_package: dict[str, Any] = field(default_factory=dict)
    notes: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, Any]:
        return {
            "dataset_version": self.dataset_version,
            "source_root": self.source_root,
            "output_root": self.output_root,
            "labels": self.labels,
            "split_ratios": self.split_ratios,
            "split_seed": self.split_seed,
            "split_grouping": self.split_grouping,
            "image_count": self.image_count,
            "annotation_count": self.annotation_count,
            "split_counts": self.split_counts,
            "split_annotation_counts": self.split_annotation_counts,
            "label_counts": self.label_counts,
            "empty_label_images": self.empty_label_images,
            "clipped_bboxes": self.clipped_bboxes,
            "skipped_annotations": self.skipped_annotations,
            "source_class_counts": self.source_class_counts,
            "source_labels_seen": self.source_labels_seen,
            "source_package": self.source_package,
            "notes": self.notes,
        }


class UAVRoboflowYoloImporter(BaseImporter):
    """Convert a train-only Roboflow YOLO export to the project YOLO layout."""

    def __init__(
        self,
        raw_dir: str | Path,
        output_dir: str | Path,
        label_map: dict[str, str] | None = None,
        *,
        dataset_version: str = "roboflow_uav_tree_crown_2026_05_26",
        split_ratios: dict[str, float] | None = None,
        seed: int = 42,
        split_grouping: str = "filename",
        copy_raw: bool = False,
        overwrite: bool = False,
        dry_run: bool = False,
    ) -> None:
        super().__init__(
            raw_dir=raw_dir,
            output_dir=output_dir,
            label_map=label_map or {},
        )
        self.dataset_version = dataset_version
        self.split_ratios = split_ratios or DEFAULT_SPLIT_RATIOS
        self.seed = seed
        self.split_grouping = split_grouping
        self.copy_raw = copy_raw
        self.overwrite = overwrite
        self.dry_run = dry_run
        self.last_summary: UAVImportSummary | None = None

    def convert(self) -> ImportResult:
        self.validate_raw_dir()
        layout = _discover_yolo_layout(self.raw_dir)
        source_names = _load_source_names(self.raw_dir, layout.root)
        samples = self._load_samples(layout, source_names)
        self._assign_output_names(samples)
        self._assign_splits(samples)

        summary = self._build_summary(samples, layout, source_names)
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
            task="uav_tree_crown",
            images_processed=summary.image_count,
            images_skipped=0,
            labels_mapped=summary.label_counts,
            output_dir=str(self.output_dir),
            notes=summary.notes,
        )

    def _load_samples(
        self,
        layout: "YoloLayout",
        source_names: dict[int, str],
    ) -> list[UAVSample]:
        samples: list[UAVSample] = []
        image_paths = [
            path
            for path in sorted(layout.image_dir.rglob("*"))
            if path.is_file() and path.suffix.lower() in IMAGE_EXTENSIONS
        ]
        if not image_paths:
            raise ValueError(f"No image files found under {layout.image_dir}")

        for image_path in image_paths:
            relative = image_path.relative_to(layout.image_dir)
            label_path = layout.label_dir / relative.with_suffix(".txt")
            annotations = (
                _read_yolo_annotations(label_path, source_names)
                if label_path.exists()
                else []
            )
            with Image.open(image_path) as image:
                width, height = image.size

            samples.append(
                UAVSample(
                    source_image_path=image_path,
                    source_label_path=label_path if label_path.exists() else None,
                    source_file_name=relative.as_posix(),
                    width=width,
                    height=height,
                    image_hash=_hash_file(image_path),
                    group_id=_build_group_id(relative, self.split_grouping),
                    annotations=annotations,
                )
            )

        return samples

    def _assign_output_names(self, samples: list[UAVSample]) -> None:
        for index, sample in enumerate(
            sorted(samples, key=lambda item: item.source_file_name),
            start=1,
        ):
            suffix = sample.source_image_path.suffix.lower()
            sample.output_file_name = f"uav_crown_{index:06d}{suffix}"

    def _assign_splits(self, samples: list[UAVSample]) -> None:
        rng = random.Random(self.seed)
        groups: dict[str, list[UAVSample]] = defaultdict(list)
        for sample in samples:
            groups[sample.group_id].append(sample)

        group_items = list(groups.items())
        rng.shuffle(group_items)
        group_items.sort(key=lambda item: -len(item[1]))

        targets = _split_targets(len(samples), self.split_ratios)
        assigned_counts = {"train": 0, "val": 0, "test": 0}

        for _group_id, group_samples in group_items:
            split = _choose_split_for_group(targets, assigned_counts)
            for sample in group_samples:
                sample.split = split
            assigned_counts[split] += len(group_samples)

        for split in ("train", "val", "test"):
            if assigned_counts[split] > 0 or len(samples) < 3:
                continue
            donor = max(("train", "val", "test"), key=lambda name: assigned_counts[name])
            donor_samples = [sample for sample in samples if sample.split == donor]
            if len(donor_samples) <= 1:
                continue
            moved = donor_samples[-1]
            moved.split = split
            assigned_counts[donor] -= 1
            assigned_counts[split] += 1

    def _build_summary(
        self,
        samples: list[UAVSample],
        layout: "YoloLayout",
        source_names: dict[int, str],
    ) -> UAVImportSummary:
        split_counts = Counter(sample.split for sample in samples)
        split_annotation_counts = Counter()
        source_class_counts = Counter()

        for sample in samples:
            split_annotation_counts[sample.split] += len(sample.annotations)
            for annotation in sample.annotations:
                source_class_counts[str(annotation.source_class_id)] += 1

        annotation_count = sum(len(sample.annotations) for sample in samples)
        notes = [
            "Generated from a train-only Roboflow YOLO export.",
            "All source classes are normalized to the single project label oil_palm_crown.",
            "The original export has no validation/test split; this importer creates a deterministic 70/15/15 split.",
            "Mission IDs are not present in the source export, so split grouping is best-effort and recorded for review.",
            "License is recorded as Unknown until Roboflow/source permissions are documented.",
        ]

        return UAVImportSummary(
            dataset_version=self.dataset_version,
            source_root=_metadata_path(self.raw_dir),
            output_root=_metadata_path(self.output_dir),
            labels=UAV_LABELS,
            split_ratios=self.split_ratios,
            split_seed=self.seed,
            split_grouping=self.split_grouping,
            image_count=len(samples),
            annotation_count=annotation_count,
            split_counts=dict(sorted(split_counts.items())),
            split_annotation_counts=dict(sorted(split_annotation_counts.items())),
            label_counts={"oil_palm_crown": annotation_count},
            empty_label_images=sum(1 for sample in samples if not sample.annotations),
            source_class_counts=dict(sorted(source_class_counts.items())),
            source_labels_seen={str(key): value for key, value in sorted(source_names.items())},
            source_package={
                "name": "roboflow_uav_tree_crown",
                "format": "roboflow_yolo",
                "source_root": _metadata_path(layout.root),
                "image_dir": _metadata_path(layout.image_dir),
                "label_dir": _metadata_path(layout.label_dir),
                "license": "Unknown",
                "roboflow_url": "https://app.roboflow.com/ds/zKMDyUsHc9",
                "images": len(samples),
                "annotations": annotation_count,
            },
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
        source = self.raw_dir.resolve()
        target = raw_target.resolve()
        if target.is_relative_to(source):
            raise ValueError(
                "--copy-raw requires an external source-root; refusing to copy a raw "
                "directory into itself."
            )
        if raw_target.exists():
            if not self.overwrite:
                raise FileExistsError(
                    f"{raw_target} already exists. Use --overwrite to refresh raw copy."
                )
            shutil.rmtree(raw_target)
        raw_target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(self.raw_dir, raw_target)

    def _write_yolo_dataset(self, samples: list[UAVSample]) -> tuple[int, int]:
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
                clipped, was_clipped = _clip_normalized_bbox(annotation.bbox_xywh)
                if was_clipped:
                    clipped_count += 1
                x_center, y_center, width, height = clipped
                if width <= 0.0 or height <= 0.0:
                    skipped_count += 1
                    continue
                lines.append(
                    "0 "
                    f"{x_center:.6f} {y_center:.6f} "
                    f"{width:.6f} {height:.6f}"
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
            "  0: oil_palm_crown",
        ]
        (yolo_root / "data.yaml").write_text("\n".join(lines) + "\n", encoding="utf-8")

    def _write_metadata(self, summary: UAVImportSummary, samples: list[UAVSample]) -> None:
        summary_dict = summary.as_dict()
        splits_dir = self.output_dir / "splits"
        licenses_dir = self.output_dir / "licenses"
        manifests_dir = self.output_dir.parent / "manifests"

        for split in ("train", "val", "test"):
            entries = [
                f"yolo/images/{sample.split}/{sample.output_file_name}"
                for sample in sorted(samples, key=lambda item: item.output_file_name)
                if sample.split == split
            ]
            (splits_dir / f"{split}.txt").write_text(
                "\n".join(entries) + ("\n" if entries else ""),
                encoding="utf-8",
            )

        (splits_dir / "split_manifest.json").write_text(
            json.dumps(_split_manifest(summary, samples), indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
        (splits_dir / "uav_dataset_summary.json").write_text(
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
        (manifests_dir / "uav_tree_crown.json").write_text(
            json.dumps(_manifest(summary), indent=2, ensure_ascii=False),
            encoding="utf-8",
        )


@dataclass(frozen=True)
class YoloLayout:
    root: Path
    image_dir: Path
    label_dir: Path


def _discover_yolo_layout(raw_dir: Path) -> YoloLayout:
    roots = [raw_dir / "train", raw_dir]
    for root in roots:
        if not root.exists() or not root.is_dir():
            continue
        image_dir = root / "images" if (root / "images").is_dir() else root
        label_dir = root / "labels" if (root / "labels").is_dir() else root
        if any(path.suffix.lower() in IMAGE_EXTENSIONS for path in image_dir.rglob("*") if path.is_file()):
            return YoloLayout(root=root, image_dir=image_dir, label_dir=label_dir)
    raise ValueError(f"No supported Roboflow YOLO train layout found under {raw_dir}")


def _load_source_names(raw_dir: Path, train_root: Path) -> dict[int, str]:
    for data_yaml in (raw_dir / "data.yaml", train_root / "data.yaml"):
        if not data_yaml.exists():
            continue
        with data_yaml.open("r", encoding="utf-8") as f:
            data = yaml.safe_load(f) or {}
        names = data.get("names", {})
        if isinstance(names, list):
            return {idx: str(label) for idx, label in enumerate(names)}
        if isinstance(names, dict):
            return {int(key): str(value) for key, value in names.items()}
    return {0: "oil_palm_crown"}


def _read_yolo_annotations(
    label_path: Path,
    source_names: dict[int, str],
) -> list[UAVAnnotation]:
    annotations: list[UAVAnnotation] = []
    for line in label_path.read_text(encoding="utf-8").splitlines():
        parts = line.strip().split()
        if len(parts) < 5:
            continue
        try:
            source_class_id = int(float(parts[0]))
            x_center, y_center, width, height = (float(value) for value in parts[1:5])
        except ValueError:
            continue
        annotations.append(
            UAVAnnotation(
                bbox_xywh=(x_center, y_center, width, height),
                source_class_id=source_class_id,
                source_label=source_names.get(source_class_id, f"class_{source_class_id}"),
            )
        )
    return annotations


def _clip_normalized_bbox(
    bbox: tuple[float, float, float, float],
) -> tuple[tuple[float, float, float, float], bool]:
    x_center, y_center, width, height = bbox
    x1 = max(0.0, min(1.0, x_center - width / 2.0))
    y1 = max(0.0, min(1.0, y_center - height / 2.0))
    x2 = max(0.0, min(1.0, x_center + width / 2.0))
    y2 = max(0.0, min(1.0, y_center + height / 2.0))
    clipped_width = max(0.0, x2 - x1)
    clipped_height = max(0.0, y2 - y1)
    clipped = (
        x1 + clipped_width / 2.0,
        y1 + clipped_height / 2.0,
        clipped_width,
        clipped_height,
    )
    was_clipped = any(abs(new - old) > 1e-6 for new, old in zip(clipped, bbox))
    return clipped, was_clipped


def _build_group_id(relative_path: Path, split_grouping: str) -> str:
    stem = _strip_roboflow_suffix(relative_path.with_suffix("").as_posix())
    if split_grouping == "mission_hint":
        lower = stem.lower()
        for marker in ("_tile_", "-tile-", "_patch_", "-patch_"):
            index = lower.find(marker)
            if index > 0:
                return stem[:index]
    return stem


def _strip_roboflow_suffix(value: str) -> str:
    return (
        value.split("_jpg.rf.")[0]
        .split("_jpeg.rf.")[0]
        .split("_png.rf.")[0]
        .split("_bmp.rf.")[0]
    )


def _hash_file(path: Path) -> str:
    digest = hashlib.md5()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _split_targets(total: int, ratios: dict[str, float]) -> dict[str, int]:
    if total <= 0:
        return {"train": 0, "val": 0, "test": 0}
    if total < 3:
        return {"train": total, "val": 0, "test": 0}
    train = max(1, round(total * ratios.get("train", 0.70)))
    val = max(1, round(total * ratios.get("val", 0.15)))
    test = max(1, total - train - val)
    while train + val + test > total:
        train -= 1
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


def _metadata_path(path: str | Path) -> str:
    resolved = Path(path).resolve()
    try:
        return resolved.relative_to(Path.cwd().resolve()).as_posix()
    except ValueError:
        parts = list(resolved.parts)
        if resolved.drive and len(parts) > 1:
            return "<external>/" + "/".join(parts[1:])
        return resolved.as_posix()


def _has_real_entries(path: Path) -> bool:
    if not path.exists():
        return False
    return any(entry.name not in SENTINEL_FILENAMES for entry in path.iterdir())


def _clear_directory_files(path: Path) -> None:
    if not path.exists():
        return
    for entry in sorted(path.rglob("*"), key=lambda item: len(item.parts), reverse=True):
        if entry.is_file() and entry.name not in SENTINEL_FILENAMES:
            entry.chmod(0o666)
            entry.unlink()


def _split_manifest(summary: UAVImportSummary, samples: list[UAVSample]) -> dict[str, Any]:
    entries = []
    for sample in sorted(samples, key=lambda item: item.output_file_name):
        entries.append(
            {
                "image": f"yolo/images/{sample.split}/{sample.output_file_name}",
                "label": f"yolo/labels/{sample.split}/{Path(sample.output_file_name).stem}.txt",
                "split": sample.split,
                "group_id": sample.group_id,
                "source_file_name": sample.source_file_name,
                "source_label_file": (
                    _metadata_path(sample.source_label_path)
                    if sample.source_label_path is not None
                    else None
                ),
                "labels": ["oil_palm_crown"] if sample.annotations else [],
                "annotation_count": len(sample.annotations),
                "source_classes": sorted(
                    {
                        str(annotation.source_class_id)
                        for annotation in sample.annotations
                    }
                ),
            }
        )
    return {
        "task": "uav_tree_crown",
        "dataset_version": summary.dataset_version,
        "split_strategy": {
            "method": "best_effort_filename_grouping",
            "grouping": summary.split_grouping,
            "ratios": summary.split_ratios,
            "seed": summary.split_seed,
            "constraint": (
                "Mission IDs were not present in the Roboflow export; keep future "
                "mission-level exports grouped by mission before retraining."
            ),
        },
        "counts": {
            "images": summary.image_count,
            "annotations": summary.annotation_count,
            "splits": summary.split_counts,
            "labels": summary.label_counts,
        },
        "entries": entries,
    }


def _qa_report(summary: UAVImportSummary) -> dict[str, Any]:
    return {
        "task": "uav_tree_crown",
        "dataset_version": summary.dataset_version,
        "checks": {
            "labels_match_project_standard": summary.labels == UAV_LABELS,
            "single_project_class_only": summary.label_counts.keys() <= {"oil_palm_crown"},
            "source_health_labels_used": False,
            "empty_label_images": summary.empty_label_images,
            "clipped_bboxes": summary.clipped_bboxes,
            "skipped_annotations_after_clipping": summary.skipped_annotations,
            "license_status": "Unknown",
            "mission_grouping_available": False,
        },
        "notes": summary.notes,
    }


def _write_sources_csv(path: Path, summary: UAVImportSummary) -> None:
    source = summary.source_package
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
        writer.writerow(
            {
                "dataset_version": summary.dataset_version,
                "source_name": source["name"],
                "source_path": source["source_root"],
                "roboflow_url": source["roboflow_url"],
                "license": source["license"],
                "images": source["images"],
                "annotations": source["annotations"],
                "notes": "Roboflow YOLO export; private key omitted; license pending verification",
            }
        )


def _dataset_card(summary: UAVImportSummary) -> str:
    return f"""# UAV Tree Crown Dataset

## Status

Local YOLO training dataset prepared from a train-only Roboflow export.

## Labels

0. `oil_palm_crown`

The source metadata mentioned health/status concepts, but the reviewed export
contains only crown instances. This project dataset keeps UAV v1 as a
single-class crown detector.

## Counts

- Images: {summary.image_count}
- Annotations: {summary.annotation_count}
- Split counts: {json.dumps(summary.split_counts, ensure_ascii=False)}
- Label counts: {json.dumps(summary.label_counts, ensure_ascii=False)}
- Empty-label images: {summary.empty_label_images}
- Bboxes clipped to image bounds: {summary.clipped_bboxes}

## Sources And License

Source exports are stored outside Git and can be copied into the ignored `raw/`
layer when `--copy-raw` is used. License is currently recorded as `Unknown`;
verify source permissions before publishing trained weights.

## Notes

- Generated version: `{summary.dataset_version}`
- Split method: deterministic 70/15/15 split with `{summary.split_grouping}` grouping.
- UAV model detects per-tile crowns only. Cloud remains responsible for tile
  offsets, global coordinates, NMS, human confirmation, and tree_code creation.
"""


def _manifest(summary: UAVImportSummary) -> dict[str, Any]:
    source = summary.source_package
    return {
        "task": "uav_tree_crown",
        "version": summary.dataset_version,
        "description": "UAV tile oil palm crown detection dataset generated from Roboflow YOLO export",
        "internal_training_standard": {
            "format": "yolo_detection",
            "annotation_type": "bbox",
            "image_size": None,
            "coordinate_system": "normalized_xywh",
        },
        "sources": [
            {
                "name": source["name"],
                "format": source["format"],
                "source_root": source["source_root"],
                "license": source["license"],
                "url": source["roboflow_url"],
                "images": source["images"],
                "annotations": source["annotations"],
                "importer": "import_uav_roboflow_yolo.py",
            }
        ],
        "label_map": {"0": "oil_palm_crown"},
        "label_notes": (
            "Single-class crown detection. Source health-status metadata is not "
            "used because the current export contains only oil_palm_crown boxes."
        ),
        "split_strategy": {
            "method": "best_effort_filename_grouping",
            "ratios": summary.split_ratios,
            "seed": summary.split_seed,
            "constraint": "Future UAV exports should include mission/block IDs for leakage-safe mission grouping.",
            "source_limitation": "The current Roboflow export did not include validation/test splits or mission IDs.",
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
