"""Tests for UAV tree crown Roboflow YOLO dataset preparation."""

from __future__ import annotations

import json
from pathlib import Path

from PIL import Image


def test_uav_roboflow_importer_writes_project_yolo_layout(tmp_path: Path) -> None:
    from ai_engine.crops.oil_palm.training.data_importers.import_uav_roboflow_yolo import (
        UAVRoboflowYoloImporter,
    )

    raw_root = tmp_path / "roboflow_export"
    train_dir = raw_root / "train"
    train_dir.mkdir(parents=True)
    (raw_root / "data.yaml").write_text(
        "names:\n"
        "  0: Healthy-BSR-Non-BSR\n"
        "  1: oil_palm_crown\n",
        encoding="utf-8",
    )

    for index in range(10):
        image_name = f"tile_{index:02d}.jpg"
        Image.new("RGB", (128, 128), color=(20 + index, 90, 40)).save(train_dir / image_name)
        source_class = 1 if index % 2 else 0
        label_lines = [
            f"{source_class} 0.500000 0.500000 0.250000 0.250000",
            f"{source_class} 1.050000 0.500000 0.200000 0.200000",
        ]
        (train_dir / f"tile_{index:02d}.txt").write_text(
            "\n".join(label_lines) + "\n",
            encoding="utf-8",
        )

    output_root = tmp_path / "datasets" / "oil_palm" / "uav_tree_crown"
    importer = UAVRoboflowYoloImporter(
        raw_dir=raw_root,
        output_dir=output_root,
        dataset_version="test_uav",
        overwrite=True,
    )

    result = importer.convert()

    assert result.task == "uav_tree_crown"
    assert result.images_processed == 10
    assert result.labels_mapped == {"oil_palm_crown": 20}

    data_yaml = (output_root / "yolo" / "data.yaml").read_text(encoding="utf-8")
    assert "0: oil_palm_crown" in data_yaml
    assert "Healthy-BSR-Non-BSR" not in data_yaml

    image_files = list((output_root / "yolo" / "images").rglob("*.jpg"))
    label_files = list((output_root / "yolo" / "labels").rglob("*.txt"))
    assert len(image_files) == 10
    assert len(label_files) == 10
    assert all(path.name.startswith("uav_crown_") for path in image_files)

    for label_file in label_files:
        for line in label_file.read_text(encoding="utf-8").splitlines():
            parts = line.split()
            assert len(parts) == 5
            assert parts[0] == "0"
            values = [float(value) for value in parts[1:]]
            assert all(0.0 <= value <= 1.0 for value in values)
            assert values[2] > 0.0
            assert values[3] > 0.0

    split_manifest = json.loads(
        (output_root / "splits" / "split_manifest.json").read_text(encoding="utf-8")
    )
    split_paths = {
        split: set((output_root / "splits" / f"{split}.txt").read_text(encoding="utf-8").split())
        for split in ("train", "val", "test")
    }
    assert split_manifest["counts"]["images"] == 10
    assert split_manifest["counts"]["annotations"] == 20
    assert not (split_paths["train"] & split_paths["val"])
    assert not (split_paths["train"] & split_paths["test"])
    assert not (split_paths["val"] & split_paths["test"])
    assert all(split_paths[split] for split in ("train", "val", "test"))

    qa_report = json.loads(
        (output_root / "splits" / "annotation_qa_report.json").read_text(encoding="utf-8")
    )
    assert qa_report["checks"]["single_project_class_only"] is True
    assert qa_report["checks"]["source_health_labels_used"] is False
    assert qa_report["checks"]["clipped_bboxes"] == 10
    assert (output_root / "dataset_card.md").exists()
    assert (output_root.parent / "manifests" / "uav_tree_crown.json").exists()


def test_uav_roboflow_importer_accepts_images_labels_layout(tmp_path: Path) -> None:
    from ai_engine.crops.oil_palm.training.data_importers.import_uav_roboflow_yolo import (
        UAVRoboflowYoloImporter,
    )

    raw_root = tmp_path / "roboflow_export"
    image_dir = raw_root / "train" / "images"
    label_dir = raw_root / "train" / "labels"
    image_dir.mkdir(parents=True)
    label_dir.mkdir(parents=True)
    Image.new("RGB", (64, 64), color=(30, 120, 80)).save(image_dir / "ortho_tile.jpg")
    (label_dir / "ortho_tile.txt").write_text(
        "0 0.500000 0.500000 0.400000 0.400000\n",
        encoding="utf-8",
    )

    output_root = tmp_path / "datasets" / "oil_palm" / "uav_tree_crown"
    result = UAVRoboflowYoloImporter(
        raw_dir=raw_root,
        output_dir=output_root,
        dataset_version="test_uav_nested",
        overwrite=True,
    ).convert()

    assert result.images_processed == 1
    assert result.labels_mapped == {"oil_palm_crown": 1}
    assert (output_root / "yolo" / "images" / "train" / "uav_crown_000001.jpg").exists()
    assert (output_root / "yolo" / "labels" / "train" / "uav_crown_000001.txt").exists()


def test_uav_train_yolo_dry_run_args_do_not_require_ultralytics(tmp_path: Path) -> None:
    from ai_engine.crops.oil_palm.training.uav_tree_crown.train_yolo import (
        build_train_args,
        load_training_config,
    )

    data_yaml = tmp_path / "data.yaml"
    data_yaml.write_text(
        "path: .\ntrain: images/train\nval: images/val\nnames:\n  0: oil_palm_crown\n",
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
  degrees: 20
  flipud: 0.5
  mosaic: 0.7
""",
        encoding="utf-8",
    )

    loaded = load_training_config(config)
    args = build_train_args(loaded, run_name="override_run")

    assert args["data"] == str(data_yaml)
    assert args["name"] == "override_run"
    assert args["epochs"] == 1
    assert args["imgsz"] == 640
    assert args["degrees"] == 20
    assert args["flipud"] == 0.5
    assert args["mosaic"] == 0.7
