from pathlib import Path

from PIL import Image

from ai_engine.crops.rice.training.prepare_rice_cls_dataset import collect_classification_samples


def create_dummy_image(path: Path):
    img = Image.new("RGB", (64, 64), color=(255, 255, 255))
    img.save(path)


def test_collect_classification_samples_rules(tmp_path):
    dataset_root = tmp_path / "dataset"
    images_dir = dataset_root / "train" / "images"
    labels_dir = dataset_root / "train" / "labels"
    images_dir.mkdir(parents=True, exist_ok=True)
    labels_dir.mkdir(parents=True, exist_ok=True)

    class_names = [
        "Bacterial_Leaf_Blight",
        "Brown_Spot",
        "HealthyLeaf",
        "Leaf_Blast"
    ]

    # case 1: empty label -> HealthyLeaf
    img1 = images_dir / "img1.jpg"
    create_dummy_image(img1)
    (labels_dir / "img1.txt").write_text("", encoding="utf-8")

    # case 2: single class -> that class
    img2 = images_dir / "img2.jpg"
    create_dummy_image(img2)
    (labels_dir / "img2.txt").write_text("1 0.5 0.5 0.2 0.2\n", encoding="utf-8")

    # case 3: multiple classes -> majority class
    img3 = images_dir / "img3.jpg"
    create_dummy_image(img3)
    (labels_dir / "img3.txt").write_text(
        "3 0.5 0.5 0.2 0.2\n"
        "3 0.6 0.6 0.2 0.2\n"
        "1 0.4 0.4 0.2 0.2\n",
        encoding="utf-8"
    )

    samples, stats = collect_classification_samples(
        dataset_root=str(dataset_root),
        class_names=class_names,
        healthy_class_name="HealthyLeaf"
    )

    assert len(samples) == 3
    assert stats["empty_label_as_healthy"] == 1

    labels_by_name = {Path(s["image_path"]).stem: s["label_name"] for s in samples}

    assert labels_by_name["img1"] == "HealthyLeaf"
    assert labels_by_name["img2"] == "Brown_Spot"
    assert labels_by_name["img3"] == "Leaf_Blast"
