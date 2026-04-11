import json
from pathlib import Path
from typing import Dict, List

import numpy as np
from PIL import Image
import torch
import torch.nn as nn
import torchvision
from torchvision import transforms


def load_json(path: str):
    with Path(path).open("r", encoding="utf-8") as f:
        return json.load(f)


def build_model(num_classes: int):
    model = torchvision.models.resnet18(weights=None)
    model.fc = nn.Sequential(
        nn.Dropout(p=0.3),
        nn.Linear(model.fc.in_features, num_classes)
    )
    return model


class RiceLeafClassifier:
    def __init__(
        self,
        checkpoint_path: str,
        labels_file: str = "models/rice_leaf_classifier/labels.json",
        config_file: str = "models/rice_leaf_classifier/config.json",
        device: str = None
    ):
        self.class_names: List[str] = load_json(labels_file)
        self.config: Dict = load_json(config_file)
        self.device = torch.device(
            device if device is not None else ("cuda" if torch.cuda.is_available() else "cpu")
        )

        self.model = build_model(num_classes=len(self.class_names))
        checkpoint = torch.load(checkpoint_path, map_location=self.device)

        if isinstance(checkpoint, dict) and "model_state_dict" in checkpoint:
            state_dict = checkpoint["model_state_dict"]
        else:
            state_dict = checkpoint

        self.model.load_state_dict(state_dict)
        self.model.to(self.device)
        self.model.eval()

        img_size = self.config["img_size"]
        self.transform = transforms.Compose([
            transforms.Resize((256, 256)),
            transforms.CenterCrop(img_size),
            transforms.ToTensor(),
            transforms.Normalize(
                mean=[0.485, 0.456, 0.406],
                std=[0.229, 0.224, 0.225]
            ),
        ])

    def predict(self, image: Image.Image, top_k: int = 3) -> Dict:
        image = image.convert("RGB")
        input_tensor = self.transform(image).unsqueeze(0).to(self.device)

        with torch.no_grad():
            logits = self.model(input_tensor)
            probs = torch.softmax(logits, dim=1)[0].cpu().numpy()

        top_indices = np.argsort(probs)[::-1][:top_k]

        return {
            "predicted_class": self.class_names[top_indices[0]],
            "confidence": float(probs[top_indices[0]]),
            "topk": [
                {
                    "label": self.class_names[idx],
                    "score": float(probs[idx])
                }
                for idx in top_indices
            ],
            "model_version": "rice_cls_v0.1.0"
        }
