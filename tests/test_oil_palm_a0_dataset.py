"""Tests for A0 Roboflow COCO dataset preparation."""

from __future__ import annotations

import json
import uuid
from pathlib import Path

from PIL import Image


PROJECT_ROOT = Path(__file__).resolve().parent.parent
TEST_OUTPUT_DIR = PROJECT_ROOT / "target" / "test_oil_palm_a0_dataset"


def test_a0_roboflow_importer_writes_project_yolo_layout() -> None:
    from ai_engine.crops.oil_palm.training.data_importers.import_a0_roboflow_coco import (
        A0RoboflowCocoImporter,
    )

    run_root = _run_root("importer")
    raw_root = run_root / "raw"
    output_root = run_root / "datasets" / "oil_palm" / "a0_image_routing"
    _write_package(
        raw_root,
        package_name="fruit_bunch",
        source_label="ffb",
        root_label="oil-palm-plantation",
        colors=[(200, 20, 20), (210, 30, 30), (220, 40, 40), (200, 20, 20)],
        duplicate_last=True,
    )
    _write_package(
        raw_root,
        package_name="trunk_base",
        source_label="Trunk_base",
        root_label="oil-palm-tree",
        colors=[(60, 90, 120), (70, 100, 130), (80, 110, 140)],
    )
    _write_package(
        raw_root,
        package_name="crown_region",
        source_label="crown_region",
        root_label="Healthy-BSR-Non-BSR",
        colors=[(20, 160, 70), (30, 170, 80), (40, 180, 90)],
    )

    importer = A0RoboflowCocoImporter(
        raw_dir=raw_root,
        output_dir=output_root,
        dataset_version="test_a0",
        copy_raw=True,
        overwrite=True,
    )

    result = importer.convert()

    assert result.task == "a0_image_routing"
    assert result.images_processed == 9
    assert result.images_skipped == 1
    assert result.labels_mapped == {
        "crown_region": 3,
        "fruit_bunch": 3,
        "trunk_base": 3,
    }

    data_yaml = (output_root / "yolo" / "data.yaml").read_text(encoding="utf-8")
    assert "0: fruit_bunch" in data_yaml
    assert "1: trunk_base" in data_yaml
    assert "2: crown_region" in data_yaml
    assert "unknown" not in data_yaml

    image_files = list((output_root / "yolo" / "images").rglob("*.jpg"))
    label_files = list((output_root / "yolo" / "labels").rglob("*.txt"))
    assert len(image_files) == 9
    assert len(label_files) == 9
    assert all(path.name.startswith("a0_") for path in image_files)

    boxes = []
    for label_file in label_files:
        for line in label_file.read_text(encoding="utf-8").splitlines():
            parts = line.split()
            assert len(parts) == 5
            class_id = int(parts[0])
            values = [float(value) for value in parts[1:]]
            assert class_id in {0, 1, 2}
            assert all(0.0 <= value <= 1.0 for value in values)
            assert values[2] > 0.0
            assert values[3] > 0.0
            boxes.append(class_id)

    assert sorted(set(boxes)) == [0, 1, 2]

    split_manifest = json.loads(
        (output_root / "splits" / "split_manifest.json").read_text(encoding="utf-8")
    )
    split_paths = {
        split: set((output_root / "splits" / f"{split}.txt").read_text(encoding="utf-8").split())
        for split in ("train", "val", "test")
    }
    assert split_manifest["counts"]["images"] == 9
    assert not (split_paths["train"] & split_paths["val"])
    assert not (split_paths["train"] & split_paths["test"])
    assert not (split_paths["val"] & split_paths["test"])

    qa_report = json.loads(
        (output_root / "splits" / "annotation_qa_report.json").read_text(encoding="utf-8")
    )
    assert qa_report["checks"]["contains_unknown_label"] is False
    assert qa_report["checks"]["duplicate_images_skipped"] == 1
    assert qa_report["checks"]["clipped_bboxes"] >= 1
    assert (
        output_root
        / "raw"
        / "test_a0"
        / "fruit_bunch"
        / "train"
        / "raw_file_map.csv"
    ).exists()
    assert (output_root.parent / "manifests" / "a0_image_routing.json").exists()


def test_a0_train_yolo_dry_run_args_do_not_require_ultralytics() -> None:
    from ai_engine.crops.oil_palm.training.a0_image_routing.train_yolo import (
        build_train_args,
        load_training_config,
    )

    run_root = _run_root("train")
    data_yaml = run_root / "data.yaml"
    data_yaml.parent.mkdir(parents=True, exist_ok=True)
    data_yaml.write_text("path: .\ntrain: images/train\nval: images/val\nnames:\n  0: fruit_bunch\n", encoding="utf-8")
    config = run_root / "training.yaml"
    config.write_text(
        f"""
task: a0_image_routing
model: yolov8n.pt
train:
  data: {data_yaml.as_posix()}
  project: {run_root.as_posix()}/runs
  name: test_run
  epochs: 1
  imgsz: 640
augment:
  degrees: 10
  flipud: 0.0
  mosaic: 0.5
""",
        encoding="utf-8",
    )

    loaded = load_training_config(config)
    args = build_train_args(loaded, run_name="override_run")

    assert args["data"] == str(data_yaml)
    assert args["name"] == "override_run"
    assert args["epochs"] == 1
    assert args["degrees"] == 10
    assert args["flipud"] == 0.0
    assert args["mosaic"] == 0.5


def _run_root(prefix: str) -> Path:
    path = TEST_OUTPUT_DIR / f"{prefix}_{uuid.uuid4().hex}"
    path.mkdir(parents=True, exist_ok=True)
    return path


def _write_package(
    raw_root: Path,
    *,
    package_name: str,
    source_label: str,
    root_label: str,
    colors: list[tuple[int, int, int]],
    duplicate_last: bool = False,
) -> None:
    train_dir = raw_root / package_name / "train"
    train_dir.mkdir(parents=True, exist_ok=True)

    images = []
    annotations = []
    for index, color in enumerate(colors, start=1):
        file_name = f"{package_name}_{index}.jpg"
        if duplicate_last and index == len(colors):
            duplicate_source = train_dir / f"{package_name}_1.jpg"
            (train_dir / file_name).write_bytes(duplicate_source.read_bytes())
        else:
            Image.new("RGB", (100, 100), color).save(train_dir / file_name)

        images.append(
            {
                "id": index,
                "license": 1,
                "file_name": file_name,
                "height": 100,
                "width": 100,
                "date_captured": "2026-05-17T00:00:00+00:00",
                "extra": {"name": f"{package_name}_{index}.jpg"},
            }
        )
        bbox = [80, 80, "30", "30"] if index == 2 else [10, 10, "20", "20"]
        annotations.append(
            {
                "id": index,
                "image_id": index,
                "category_id": 1,
                "bbox": bbox,
                "iscrowd": 0,
                "area": 400,
                "segmentation": [],
            }
        )

    coco = {
        "info": {
            "year": "2026",
            "version": "test",
            "description": "synthetic test data",
            "url": "https://example.test/roboflow",
            "date_created": "2026-05-17T00:00:00+00:00",
        },
        "licenses": [{"id": 1, "url": "", "name": "Unknown"}],
        "categories": [
            {"id": 0, "name": root_label, "supercategory": "none"},
            {"id": 1, "name": source_label, "supercategory": root_label},
        ],
        "images": images,
        "annotations": annotations,
    }
    (train_dir / "_annotations.coco.json").write_text(
        json.dumps(coco, indent=2),
        encoding="utf-8",
    )
