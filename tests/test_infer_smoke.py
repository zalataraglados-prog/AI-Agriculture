import json
import sys
from pathlib import Path

from PIL import Image
import torch

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from service.core.rice_leaf_classifier import build_model
from service.infer import predict_image_file


def test_predict_image_file_returns_expected_keys(tmp_path):
    labels = [
        "Bacterial_Leaf_Blight",
        "Brown_Spot",
        "HealthyLeaf",
        "Leaf_Blast"
    ]
    config = {
        "img_size": 224
    }

    labels_file = tmp_path / "labels.json"
    config_file = tmp_path / "config.json"
    checkpoint_file = tmp_path / "model.pth"
    image_file = tmp_path / "test.jpg"

    labels_file.write_text(json.dumps(labels, ensure_ascii=False, indent=2), encoding="utf-8")
    config_file.write_text(json.dumps(config, ensure_ascii=False, indent=2), encoding="utf-8")

    model = build_model(num_classes=len(labels))
    torch.save(model.state_dict(), checkpoint_file)

    img = Image.new("RGB", (224, 224), color=(128, 128, 128))
    img.save(image_file)

    result = predict_image_file(
        image_path=str(image_file),
        checkpoint_path=str(checkpoint_file),
        labels_file=str(labels_file),
        config_file=str(config_file),
        top_k=3
    )

    assert "predicted_class" in result
    assert "confidence" in result
    assert "topk" in result
    assert "model_version" in result
    assert isinstance(result["topk"], list)
    assert len(result["topk"]) == 3
