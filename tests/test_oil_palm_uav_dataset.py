"""Tests for UAV tree crown Roboflow COCO dataset preparation."""

from __future__ import annotations

import json
from pathlib import Path

from PIL import Image


def test_uav_roboflow_coco_importer_preserves_source_splits(tmp_path: Path) -> None:
    from ai_engine.crops.oil_palm.training.data_importers.import_uav_roboflow_coco import (
        UAVRoboflowCocoImporter,
    )

    raw_root = tmp_path / "uva_crown-1"
    _write_coco_split(
        raw_root,
        "train",
        image_count=7,
        start_id=1,
        source_label="oil_palm_crown",
        license_name="CC BY 4.0",
    )
    _write_coco_split(
        raw_root,
        "valid",
        image_count=2,
        start_id=101,
        source_label="Healthy-BSR-Non-BSR",
        license_name="CC BY 4.0",
    )
    _write_coco_split(
        raw_root,
        "test",
        image_count=1,
        start_id=201,
        source_label="oil_palm_crown",
        license_name="CC BY 4.0",
    )

    output_root = tmp_path / "datasets" / "oil_palm" / "uav_tree_crown"
    importer = UAVRoboflowCocoImporter(
        raw_dir=raw_root,
        output_dir=output_root,
        dataset_version="test_uav_coco",
        overwrite=True,
    )

    result = importer.convert()

    assert result.task == "uav_tree_crown"
    assert result.images_processed == 10
    assert result.labels_mapped == {"oil_palm_crown": 20}

    data_yaml = (output_root / "yolo" / "data.yaml").read_text(encoding="utf-8")
    assert "0: oil_palm_crown" in data_yaml
    assert "Healthy-BSR-Non-BSR" not in data_yaml

    split_manifest = json.loads(
        (output_root / "splits" / "split_manifest.json").read_text(encoding="utf-8")
    )
    assert split_manifest["split_strategy"]["method"] == "preserve_roboflow_train_valid_test"
    assert split_manifest["counts"]["splits"] == {"test": 1, "train": 7, "val": 2}
    assert split_manifest["counts"]["annotations"] == 20

    split_paths = {
        split: set((output_root / "splits" / f"{split}.txt").read_text(encoding="utf-8").split())
        for split in ("train", "val", "test")
    }
    assert len(split_paths["train"]) == 7
    assert len(split_paths["val"]) == 2
    assert len(split_paths["test"]) == 1
    assert not (split_paths["train"] & split_paths["val"])
    assert not (split_paths["train"] & split_paths["test"])
    assert not (split_paths["val"] & split_paths["test"])

    for label_file in (output_root / "yolo" / "labels").rglob("*.txt"):
        for line in label_file.read_text(encoding="utf-8").splitlines():
            parts = line.split()
            assert len(parts) == 5
            assert parts[0] == "0"
            values = [float(value) for value in parts[1:]]
            assert all(0.0 <= value <= 1.0 for value in values)
            assert values[2] > 0.0
            assert values[3] > 0.0

    qa_report = json.loads(
        (output_root / "splits" / "annotation_qa_report.json").read_text(encoding="utf-8")
    )
    assert qa_report["checks"]["single_project_class_only"] is True
    assert qa_report["checks"]["source_health_labels_used"] is False
    assert qa_report["checks"]["source_split_preserved"] is True
    assert qa_report["checks"]["extra_local_split_generated"] is False
    assert qa_report["checks"]["extra_training_augmentation_required"] is False
    assert qa_report["checks"]["clipped_bboxes"] == 10
    assert qa_report["checks"]["license_status"] == "CC BY 4.0"
    assert (output_root / "dataset_card.md").exists()
    assert (output_root.parent / "manifests" / "uav_tree_crown.json").exists()


def test_uav_coco_importer_syncs_effective_counts_after_skipping_invalid_bbox(
    tmp_path: Path,
) -> None:
    from ai_engine.crops.oil_palm.training.data_importers.import_uav_roboflow_coco import (
        UAVRoboflowCocoImporter,
    )

    raw_root = tmp_path / "uva_crown-1"
    _write_coco_split(
        raw_root,
        "train",
        image_count=1,
        start_id=1,
        source_label="oil_palm_crown",
        license_name="CC BY 4.0",
        include_invalid_bbox=True,
    )
    output_root = tmp_path / "datasets" / "oil_palm" / "uav_tree_crown"

    result = UAVRoboflowCocoImporter(
        raw_dir=raw_root,
        output_dir=output_root,
        dataset_version="test_uav_coco_invalid_bbox",
        overwrite=True,
    ).convert()

    assert result.labels_mapped == {"oil_palm_crown": 2}

    summary = json.loads(
        (output_root / "splits" / "uav_dataset_summary.json").read_text(encoding="utf-8")
    )
    assert summary["source_annotation_count"] == 3
    assert summary["annotation_count"] == 2
    assert summary["split_annotation_counts"] == {"train": 2}
    assert summary["label_counts"] == {"oil_palm_crown": 2}
    assert summary["skipped_annotations"] == 1

    split_manifest = json.loads(
        (output_root / "splits" / "split_manifest.json").read_text(encoding="utf-8")
    )
    assert split_manifest["counts"]["annotations"] == 2
    assert split_manifest["entries"][0]["annotation_count"] == 2

    qa_report = json.loads(
        (output_root / "splits" / "annotation_qa_report.json").read_text(encoding="utf-8")
    )
    assert qa_report["checks"]["license_status"] == "CC BY 4.0"
    assert qa_report["checks"]["skipped_annotations_after_clipping"] == 1


def test_uav_yolo_importer_syncs_effective_counts_after_skipping_invalid_bbox(
    tmp_path: Path,
) -> None:
    from ai_engine.crops.oil_palm.training.data_importers.import_uav_roboflow_yolo import (
        UAVRoboflowYoloImporter,
    )

    raw_root = tmp_path / "uav-yolo"
    image_dir = raw_root / "train" / "images"
    label_dir = raw_root / "train" / "labels"
    image_dir.mkdir(parents=True)
    label_dir.mkdir(parents=True)
    Image.new("RGB", (100, 100), color=(30, 90, 120)).save(image_dir / "tile_001.jpg")
    (label_dir / "tile_001.txt").write_text(
        "0 0.500000 0.500000 0.200000 0.200000\n"
        "0 1.500000 1.500000 0.100000 0.100000\n",
        encoding="utf-8",
    )
    (raw_root / "train" / "data.yaml").write_text(
        "names:\n  0: oil_palm_crown\n",
        encoding="utf-8",
    )
    output_root = tmp_path / "datasets" / "oil_palm" / "uav_tree_crown"

    result = UAVRoboflowYoloImporter(
        raw_dir=raw_root,
        output_dir=output_root,
        dataset_version="test_uav_yolo_invalid_bbox",
        overwrite=True,
    ).convert()

    assert result.labels_mapped == {"oil_palm_crown": 1}

    generated_label = next((output_root / "yolo" / "labels" / "train").glob("*.txt"))
    assert len(generated_label.read_text(encoding="utf-8").splitlines()) == 1

    summary = json.loads(
        (output_root / "splits" / "uav_dataset_summary.json").read_text(encoding="utf-8")
    )
    assert summary["source_annotation_count"] == 2
    assert summary["annotation_count"] == 1
    assert summary["split_annotation_counts"] == {"train": 1}
    assert summary["label_counts"] == {"oil_palm_crown": 1}
    assert summary["skipped_annotations"] == 1

    split_manifest = json.loads(
        (output_root / "splits" / "split_manifest.json").read_text(encoding="utf-8")
    )
    assert split_manifest["counts"]["annotations"] == 1
    assert split_manifest["entries"][0]["annotation_count"] == 1


def test_uav_prepare_dataset_dry_run_uses_coco_importer(tmp_path: Path, capsys) -> None:
    from ai_engine.crops.oil_palm.training.uav_tree_crown.prepare_dataset import main

    raw_root = tmp_path / "uva_crown-1"
    _write_coco_split(raw_root, "train", image_count=1, start_id=1, source_label="oil_palm_crown")
    output_root = tmp_path / "datasets" / "oil_palm" / "uav_tree_crown"

    exit_code = main([
        "--source-root",
        str(raw_root),
        "--output-root",
        str(output_root),
        "--dataset-version",
        "dry_run_uav",
        "--dry-run",
    ])

    assert exit_code == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["status"] == "ok"
    assert payload["dry_run"] is True
    assert payload["result"]["task"] == "uav_tree_crown"
    assert payload["summary"]["split_strategy"] == "preserve_roboflow_train_valid_test"
    assert not (output_root / "yolo").exists()


def test_uav_train_yolo_dry_run_args_do_not_require_ultralytics(tmp_path: Path) -> None:
    from ai_engine.crops.oil_palm.training.uav_tree_crown.train_yolo import (
        build_train_args,
        load_training_config,
    )

    data_yaml = tmp_path / "data.yaml"
    data_yaml.write_text(
        "path: .\ntrain: images/train\nval: images/val\ntest: images/test\nnames:\n  0: oil_palm_crown\n",
        encoding="utf-8",
    )
    config = tmp_path / "training.yaml"
    config.write_text(
        f"""
task: uav_tree_crown
model: yolov8n.pt
train:
  data: {data_yaml.as_posix()}
  project: {tmp_path.as_posix()}/runs
  name: test_run
  epochs: 1
  imgsz: 640
augment:
  degrees: 0
  fliplr: 0.0
  mosaic: 0.0
""",
        encoding="utf-8",
    )

    loaded = load_training_config(config)
    args = build_train_args(loaded, run_name="override_run")

    assert args["data"] == str(data_yaml)
    assert args["name"] == "override_run"
    assert args["epochs"] == 1
    assert args["imgsz"] == 640
    assert args["degrees"] == 0
    assert args["fliplr"] == 0.0
    assert args["mosaic"] == 0.0


def _write_coco_split(
    raw_root: Path,
    split: str,
    *,
    image_count: int,
    start_id: int,
    source_label: str,
    license_name: str = "Unknown",
    include_invalid_bbox: bool = False,
) -> None:
    split_dir = raw_root / split
    split_dir.mkdir(parents=True)
    images = []
    annotations = []
    for offset in range(image_count):
        image_id = start_id + offset
        file_name = f"{split}_{image_id}.jpg"
        Image.new("RGB", (100, 100), color=(20 + offset, 90, 130)).save(split_dir / file_name)
        images.append(
            {
                "id": image_id,
                "license": 1,
                "file_name": file_name,
                "height": 100,
                "width": 100,
            }
        )
        annotations.append(
            {
                "id": image_id * 10,
                "image_id": image_id,
                "category_id": 1,
                "bbox": [10, 10, 30, 30],
                "iscrowd": 0,
                "area": 900,
                "segmentation": [],
            }
        )
        annotations.append(
            {
                "id": image_id * 10 + 1,
                "image_id": image_id,
                "category_id": 1,
                "bbox": [90, 50, 20, 20],
                "iscrowd": 0,
                "area": 400,
                "segmentation": [],
            }
        )
        if include_invalid_bbox:
            annotations.append(
                {
                    "id": image_id * 10 + 2,
                    "image_id": image_id,
                    "category_id": 1,
                    "bbox": [150, 150, 10, 10],
                    "iscrowd": 0,
                    "area": 100,
                    "segmentation": [],
                }
            )

    coco = {
        "info": {
            "year": "2026",
            "version": "test",
            "description": "synthetic UAV crown test data",
            "url": "https://example.test/roboflow/uva_crown",
            "date_created": "2026-05-26T00:00:00+00:00",
        },
        "licenses": [{"id": 1, "url": "", "name": license_name}],
        "categories": [
            {"id": 0, "name": "root", "supercategory": "none"},
            {"id": 1, "name": source_label, "supercategory": "root"},
        ],
        "images": images,
        "annotations": annotations,
    }
    (split_dir / "_annotations.coco.json").write_text(
        json.dumps(coco, indent=2),
        encoding="utf-8",
    )
