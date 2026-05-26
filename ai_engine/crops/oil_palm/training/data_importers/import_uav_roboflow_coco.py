"""Import Roboflow COCO UAV crown exports into the project YOLO layout."""

from __future__ import annotations

import csv
import hashlib
import json
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

UAV_LABELS = ["oil_palm_crown"]
UAV_LABEL_TO_ID = {"oil_palm_crown": 0}
SOURCE_SPLITS = {"train": "train", "valid": "val", "val": "val", "test": "test"}
IMAGE_EXTENSIONS = {".jpg", ".jpeg", ".png", ".bmp", ".tif", ".tiff", ".webp"}


@dataclass
class UAVCocoAnnotation:
    """One source COCO bbox before YOLO export."""

    bbox_xywh: tuple[float, float, float, float]
    source_category_id: int
    source_label: str


@dataclass
class UAVCocoSample:
    """One UAV tile image and its mapped annotations."""

    source_split: str
    split: str
    source_image_path: Path
    source_annotation_path: Path
    source_file_name: str
    original_image_id: int
    width: int
    height: int
    image_hash: str
    annotations: list[UAVCocoAnnotation]
    output_file_name: str = ""


@dataclass
class UAVCocoImportSummary:
    """Detailed conversion summary written as tracked metadata."""

    dataset_version: str
    source_root: str
    output_root: str
    labels: list[str]
    split_strategy: str
    image_count: int = 0
    source_annotation_count: int = 0
    annotation_count: int = 0
    split_counts: dict[str, int] = field(default_factory=dict)
    split_annotation_counts: dict[str, int] = field(default_factory=dict)
    label_counts: dict[str, int] = field(default_factory=dict)
    empty_label_images: int = 0
    clipped_bboxes: int = 0
    skipped_annotations: int = 0
    source_class_counts: dict[str, int] = field(default_factory=dict)
    source_labels_seen: dict[str, str] = field(default_factory=dict)
    source_packages: list[dict[str, Any]] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, Any]:
        return {
            "dataset_version": self.dataset_version,
            "source_root": self.source_root,
            "output_root": self.output_root,
            "labels": self.labels,
            "split_strategy": self.split_strategy,
            "image_count": self.image_count,
            "source_annotation_count": self.source_annotation_count,
            "annotation_count": self.annotation_count,
            "split_counts": self.split_counts,
            "split_annotation_counts": self.split_annotation_counts,
            "label_counts": self.label_counts,
            "empty_label_images": self.empty_label_images,
            "clipped_bboxes": self.clipped_bboxes,
            "skipped_annotations": self.skipped_annotations,
            "source_class_counts": self.source_class_counts,
            "source_labels_seen": self.source_labels_seen,
            "source_packages": self.source_packages,
            "notes": self.notes,
        }


class UAVRoboflowCocoImporter(BaseImporter):
    """Convert a Roboflow COCO UAV export to project YOLO format."""

    def __init__(
        self,
        raw_dir: str | Path,
        output_dir: str | Path,
        label_map: dict[str, str] | None = None,
        *,
        dataset_version: str = "roboflow_uav_crown_v1_2026_05_26",
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
        self.copy_raw = copy_raw
        self.overwrite = overwrite
        self.dry_run = dry_run
        self.last_summary: UAVCocoImportSummary | None = None

    def convert(self) -> ImportResult:
        self.validate_raw_dir()
        samples, source_packages, source_labels = self._load_samples()
        self._assign_output_names(samples)

        summary = self._build_summary(samples, source_packages, source_labels)
        self.last_summary = summary

        if not self.dry_run:
            self._prepare_output_directories()
            if self.copy_raw:
                self._copy_raw_source()
            clipped_bboxes, skipped_annotations = self._write_yolo_dataset(samples)
            summary.clipped_bboxes = clipped_bboxes
            summary.skipped_annotations = skipped_annotations
            _sync_effective_annotation_counts(summary, samples)
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
    ) -> tuple[list[UAVCocoSample], list[dict[str, Any]], dict[int, str]]:
        samples: list[UAVCocoSample] = []
        source_packages: list[dict[str, Any]] = []
        all_source_labels: dict[int, str] = {}

        for split_dir in _source_split_dirs(self.raw_dir):
            annotation_path = split_dir / "_annotations.coco.json"
            with annotation_path.open("r", encoding="utf-8") as f:
                coco = json.load(f)

            categories = {
                int(item["id"]): str(item["name"])
                for item in coco.get("categories", [])
                if "id" in item and "name" in item
            }
            all_source_labels.update(categories)
            images_by_id = {int(item["id"]): item for item in coco.get("images", [])}
            annotations_by_image: dict[int, list[dict[str, Any]]] = defaultdict(list)
            for annotation in coco.get("annotations", []):
                image_id = annotation.get("image_id")
                if image_id is None:
                    continue
                annotations_by_image[int(image_id)].append(annotation)

            split_sample_count = 0
            split_annotation_count = 0
            target_split = SOURCE_SPLITS.get(split_dir.name, "train")
            for image_id, image_record in images_by_id.items():
                image_path = _resolve_image_path(split_dir, image_record.get("file_name", ""))
                if image_path is None:
                    continue

                mapped_annotations = []
                for annotation in annotations_by_image.get(image_id, []):
                    bbox = _parse_bbox(annotation.get("bbox"))
                    if bbox is None:
                        continue
                    category_id = int(annotation.get("category_id", 0))
                    mapped_annotations.append(
                        UAVCocoAnnotation(
                            bbox_xywh=bbox,
                            source_category_id=category_id,
                            source_label=categories.get(category_id, f"class_{category_id}"),
                        )
                    )

                width, height = _image_size(image_path, image_record)
                samples.append(
                    UAVCocoSample(
                        source_split=split_dir.name,
                        split=target_split,
                        source_image_path=image_path,
                        source_annotation_path=annotation_path,
                        source_file_name=str(image_record.get("file_name", image_path.name)),
                        original_image_id=image_id,
                        width=width,
                        height=height,
                        image_hash=_hash_file(image_path),
                        annotations=mapped_annotations,
                    )
                )
                split_sample_count += 1
                split_annotation_count += len(mapped_annotations)

            source_packages.append(
                {
                    "name": f"roboflow_uav_crown_{split_dir.name}",
                    "split": split_dir.name,
                    "project_split": target_split,
                    "annotation_path": _metadata_path(annotation_path),
                    "images": split_sample_count,
                    "annotations": split_annotation_count,
                    "license": _license_name(coco.get("licenses", [])),
                    "roboflow_url": coco.get("info", {}).get("url", ""),
                    "date_created": coco.get("info", {}).get("date_created", ""),
                }
            )

        if not samples:
            raise ValueError(f"No UAV COCO samples found under {self.raw_dir}")
        return samples, source_packages, all_source_labels

    def _assign_output_names(self, samples: list[UAVCocoSample]) -> None:
        split_order = {"train": 0, "val": 1, "test": 2}
        for index, sample in enumerate(
            sorted(
                samples,
                key=lambda item: (
                    split_order.get(item.split, 9),
                    item.source_file_name,
                    item.original_image_id,
                ),
            ),
            start=1,
        ):
            suffix = sample.source_image_path.suffix.lower()
            sample.output_file_name = f"uav_crown_{index:06d}{suffix}"

    def _build_summary(
        self,
        samples: list[UAVCocoSample],
        source_packages: list[dict[str, Any]],
        source_labels: dict[int, str],
    ) -> UAVCocoImportSummary:
        split_counts = Counter(sample.split for sample in samples)
        split_annotation_counts = Counter()
        source_class_counts = Counter()

        for sample in samples:
            split_annotation_counts[sample.split] += len(sample.annotations)
            for annotation in sample.annotations:
                source_class_counts[str(annotation.source_category_id)] += 1

        annotation_count = sum(len(sample.annotations) for sample in samples)
        license_status = _license_status(source_packages)
        license_note = (
            "License is recorded as Unknown until Roboflow/source permissions are documented."
            if license_status == "Unknown"
            else f"Source metadata reports {license_status}."
        )
        notes = [
            "Generated from Roboflow COCO export version uva_crown/1.",
            "Source train/valid/test splits are preserved; no extra split is generated.",
            "All source categories are normalized to the single project label oil_palm_crown.",
            "Roboflow-side augmentation has already been applied, so project training config disables additional augmentation for the first baseline.",
            license_note,
        ]
        return UAVCocoImportSummary(
            dataset_version=self.dataset_version,
            source_root=_metadata_path(self.raw_dir),
            output_root=_metadata_path(self.output_dir),
            labels=UAV_LABELS,
            split_strategy="preserve_roboflow_train_valid_test",
            image_count=len(samples),
            source_annotation_count=annotation_count,
            annotation_count=annotation_count,
            split_counts=dict(sorted(split_counts.items())),
            split_annotation_counts=dict(sorted(split_annotation_counts.items())),
            label_counts={"oil_palm_crown": annotation_count},
            empty_label_images=sum(1 for sample in samples if not sample.annotations),
            source_class_counts=dict(sorted(source_class_counts.items())),
            source_labels_seen={
                str(key): value
                for key, value in sorted(source_labels.items())
            },
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

    def _write_yolo_dataset(self, samples: list[UAVCocoSample]) -> tuple[int, int]:
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
            valid_annotations: list[UAVCocoAnnotation] = []
            for annotation in sample.annotations:
                clipped, was_clipped = _clip_bbox(annotation.bbox_xywh, sample.width, sample.height)
                if was_clipped:
                    clipped_count += 1
                x, y, width, height = clipped
                if width <= 0.0 or height <= 0.0:
                    skipped_count += 1
                    continue
                x_center = (x + width / 2.0) / sample.width
                y_center = (y + height / 2.0) / sample.height
                norm_width = width / sample.width
                norm_height = height / sample.height
                lines.append(
                    "0 "
                    f"{x_center:.6f} {y_center:.6f} "
                    f"{norm_width:.6f} {norm_height:.6f}"
                )
                valid_annotations.append(
                    UAVCocoAnnotation(
                        bbox_xywh=clipped,
                        source_category_id=annotation.source_category_id,
                        source_label=annotation.source_label,
                    )
                )

            label_out.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")
            sample.annotations = valid_annotations

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

    def _write_metadata(
        self,
        summary: UAVCocoImportSummary,
        samples: list[UAVCocoSample],
    ) -> None:
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


def _source_split_dirs(raw_dir: Path) -> list[Path]:
    split_dirs = []
    for source_split in ("train", "valid", "val", "test"):
        split_dir = raw_dir / source_split
        if (split_dir / "_annotations.coco.json").exists():
            split_dirs.append(split_dir)
    if not split_dirs and (raw_dir / "_annotations.coco.json").exists():
        split_dirs.append(raw_dir)
    if not split_dirs:
        raise ValueError(
            f"No Roboflow COCO _annotations.coco.json files found under {raw_dir}"
        )
    return split_dirs


def _resolve_image_path(split_dir: Path, file_name: str) -> Path | None:
    candidates = [
        split_dir / file_name,
        split_dir / "images" / file_name,
        split_dir / Path(file_name).name,
        split_dir / "images" / Path(file_name).name,
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    stem = Path(file_name).stem
    for path in split_dir.rglob("*"):
        if path.is_file() and path.stem == stem and path.suffix.lower() in IMAGE_EXTENSIONS:
            return path
    return None


def _image_size(image_path: Path, image_record: dict[str, Any]) -> tuple[int, int]:
    try:
        with Image.open(image_path) as image:
            return image.size
    except Exception:
        width = int(image_record.get("width") or 0)
        height = int(image_record.get("height") or 0)
        if width <= 0 or height <= 0:
            raise
        return width, height


def _parse_bbox(value: Any) -> tuple[float, float, float, float] | None:
    if not isinstance(value, list) or len(value) != 4:
        return None
    try:
        x, y, width, height = (float(item) for item in value)
    except (TypeError, ValueError):
        return None
    return x, y, width, height


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
    was_clipped = any(abs(new - old) > 1e-6 for new, old in zip(clipped, bbox))
    return clipped, was_clipped


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


def _license_status(source_packages: list[dict[str, Any]]) -> str:
    licenses = sorted(
        {
            str(source.get("license") or "Unknown").strip() or "Unknown"
            for source in source_packages
        }
    )
    known = [license_name for license_name in licenses if license_name != "Unknown"]
    if not known:
        return "Unknown"
    if len(known) == 1 and len(licenses) == 1:
        return known[0]
    return "Mixed: " + ", ".join(licenses)


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


def _split_manifest(
    summary: UAVCocoImportSummary,
    samples: list[UAVCocoSample],
) -> dict[str, Any]:
    entries = []
    for sample in sorted(samples, key=lambda item: item.output_file_name):
        entries.append(
            {
                "image": f"yolo/images/{sample.split}/{sample.output_file_name}",
                "label": f"yolo/labels/{sample.split}/{Path(sample.output_file_name).stem}.txt",
                "split": sample.split,
                "source_split": sample.source_split,
                "source_file_name": sample.source_file_name,
                "source_annotation_path": _metadata_path(sample.source_annotation_path),
                "original_image_id": sample.original_image_id,
                "labels": ["oil_palm_crown"] if sample.annotations else [],
                "annotation_count": len(sample.annotations),
                "source_classes": sorted(
                    {
                        str(annotation.source_category_id)
                        for annotation in sample.annotations
                    }
                ),
            }
        )
    return {
        "task": "uav_tree_crown",
        "dataset_version": summary.dataset_version,
        "split_strategy": {
            "method": summary.split_strategy,
            "constraint": "Roboflow train/valid/test split is preserved; no extra local split is generated.",
        },
        "counts": {
            "images": summary.image_count,
            "annotations": summary.annotation_count,
            "splits": summary.split_counts,
            "labels": summary.label_counts,
        },
        "entries": entries,
    }


def _sync_effective_annotation_counts(
    summary: UAVCocoImportSummary,
    samples: list[UAVCocoSample],
) -> None:
    split_annotation_counts = Counter()
    for sample in samples:
        split_annotation_counts[sample.split] += len(sample.annotations)

    annotation_count = sum(split_annotation_counts.values())
    summary.annotation_count = annotation_count
    summary.split_annotation_counts = dict(sorted(split_annotation_counts.items()))
    summary.label_counts = {"oil_palm_crown": annotation_count}
    summary.empty_label_images = sum(1 for sample in samples if not sample.annotations)


def _qa_report(summary: UAVCocoImportSummary) -> dict[str, Any]:
    return {
        "task": "uav_tree_crown",
        "dataset_version": summary.dataset_version,
        "checks": {
            "labels_match_project_standard": summary.labels == UAV_LABELS,
            "single_project_class_only": summary.label_counts.keys() <= {"oil_palm_crown"},
            "source_health_labels_used": False,
            "source_split_preserved": True,
            "extra_local_split_generated": False,
            "extra_training_augmentation_required": False,
            "empty_label_images": summary.empty_label_images,
            "clipped_bboxes": summary.clipped_bboxes,
            "skipped_annotations_after_clipping": summary.skipped_annotations,
            "license_status": _license_status(summary.source_packages),
        },
        "notes": summary.notes,
    }


def _write_sources_csv(path: Path, summary: UAVCocoImportSummary) -> None:
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
                    "notes": "Roboflow COCO export; private API key omitted",
                }
            )


def _dataset_card(summary: UAVCocoImportSummary) -> str:
    license_status = _license_status(summary.source_packages)
    return f"""# UAV Tree Crown Dataset

## Status

Local YOLO training dataset prepared from Roboflow COCO export
`doyles-workspace/uva_crown` version 1.

## Labels

0. `oil_palm_crown`

The project normalizes all source categories to a single UAV crown-localization
label. This model does not classify health, disease, growth, maturity, or yield.

## Counts

- Images: {summary.image_count}
- Source annotations: {summary.source_annotation_count}
- Effective YOLO annotations: {summary.annotation_count}
- Split counts: {json.dumps(summary.split_counts, ensure_ascii=False)}
- Label counts: {json.dumps(summary.label_counts, ensure_ascii=False)}
- Empty-label images: {summary.empty_label_images}
- Bboxes clipped to image bounds: {summary.clipped_bboxes}

## Sources And License

Source exports are stored outside Git and can be copied into the ignored `raw/`
layer when `--copy-raw` is used. License status: `{license_status}` as reported
by source metadata; verify source permissions before publishing trained weights.

## Notes

- Generated version: `{summary.dataset_version}`
- Split method: preserve Roboflow train/valid/test.
- Roboflow-side augmentation has already been applied; project training config
  disables additional augmentation for the first baseline.
- UAV model detects per-tile crowns only. Cloud handles tile offsets, global
  coordinate reconstruction, NMS, human confirmation, and tree_code creation.
"""


def _manifest(summary: UAVCocoImportSummary) -> dict[str, Any]:
    return {
        "task": "uav_tree_crown",
        "version": summary.dataset_version,
        "description": "UAV tile oil palm crown detection dataset generated from Roboflow COCO export",
        "internal_training_standard": {
            "format": "yolo_detection",
            "source_format": "roboflow_coco",
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
                "importer": "import_uav_roboflow_coco.py",
            }
            for source in summary.source_packages
        ],
        "label_map": {"0": "oil_palm_crown"},
        "label_notes": (
            "Single-class crown detection. Source categories are normalized to "
            "oil_palm_crown; health/status labels are not part of UAV v1."
        ),
        "split_strategy": {
            "method": summary.split_strategy,
            "ratios": None,
            "seed": None,
            "constraint": "Roboflow train/valid/test split is preserved.",
        },
        "expected_counts": {
            "train": summary.split_counts.get("train", 0),
            "val": summary.split_counts.get("val", 0),
            "test": summary.split_counts.get("test", 0),
            "total": summary.image_count,
            "source_annotations": summary.source_annotation_count,
            "annotations": summary.annotation_count,
            "labels": summary.label_counts,
        },
        "importer_directory": "ai_engine/crops/oil_palm/training/data_importers/",
        "notes": summary.notes,
    }
