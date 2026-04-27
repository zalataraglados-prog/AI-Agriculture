import json
import logging
import yaml
from pathlib import Path
from typing import Any, Dict, List

import numpy as np
import torch
import torch.nn as nn
import torchvision
from PIL import Image
from torchvision import transforms

from ai_engine.common.base_predictor import BasePredictor

logger = logging.getLogger(__name__)


def load_json(path: str) -> Any:
    """Helper to load JSON files."""
    with Path(path).open("r", encoding="utf-8") as f:
        return json.load(f)


def load_config(path: str) -> Dict[str, Any]:
    """Helper to load config files, with YAML migration support."""
    p = Path(path)
    if p.suffix == ".json":
        logger.warning("DeprecationWarning: Using .json config is deprecated. Please migrate to .yaml: %s", p)
        with p.open("r", encoding="utf-8") as f:
            return json.load(f)
    if p.suffix in (".yaml", ".yml"):
        if p.with_suffix(".json").exists():
            logger.warning("DeprecationWarning: Found legacy config file %s. Please remove it.", p.with_suffix(".json"))
    with p.open("r", encoding="utf-8") as f:
        return yaml.safe_load(f)


def build_model(num_classes: int) -> nn.Module:
    """Constructs the ResNet18 model architecture with a custom classification head."""
    model = torchvision.models.resnet18(weights=None)
    model.fc = nn.Sequential(
        nn.Dropout(p=0.3),
        nn.Linear(model.fc.in_features, num_classes)
    )
    return model


class RiceLeafClassifier(BasePredictor):
    """
    Rice leaf disease classifier using ResNet18.
    """

    def __init__(
        self,
        checkpoint_path: str,
        labels_file: str = "models/rice/rice_leaf_classifier/labels.json",
        config_file: str = "models/rice/rice_leaf_classifier/config.yaml",
        advice_file: str = "models/rice/rice_leaf_classifier/advice_map.yaml",
        device: str = None
    ):
        self.class_names: List[str] = load_json(labels_file)
        self.config: Dict = load_config(config_file)
        self.advice_map: Dict[str, str] = load_config(advice_file) if Path(advice_file).exists() else {}
        self.device = torch.device(
            device if device is not None else ("cuda" if torch.cuda.is_available() else "cpu")
        )
        self._model_version = "rice_cls_v0.1.0"

        logger.info("Initializing RiceLeafClassifier (version %s) on device: %s", self._model_version, self.device)
        
        self.model = build_model(num_classes=len(self.class_names))
        
        logger.info("Loading checkpoint from: %s", checkpoint_path)
        checkpoint = torch.load(checkpoint_path, map_location=self.device)

        if isinstance(checkpoint, dict) and "model_state_dict" in checkpoint:
            state_dict = checkpoint["model_state_dict"]
        else:
            state_dict = checkpoint

        self.model.load_state_dict(state_dict)
        self.model.to(self.device)
        self.model.eval()

        img_size = self.config.get("img_size", 224)
        self.transform = transforms.Compose([
            transforms.Resize((256, 256)),
            transforms.CenterCrop(img_size),
            transforms.ToTensor(),
            transforms.Normalize(
                mean=[0.485, 0.456, 0.406],
                std=[0.229, 0.224, 0.225]
            ),
        ])
        logger.info("RiceLeafClassifier initialization complete.")

    def predict(self, image: Image.Image, top_k: int = 3) -> Dict[str, Any]:
        """
        Run inference on a single RGB image.
        """
        try:
            input_tensor = self.transform(image).unsqueeze(0).to(self.device)

            with torch.no_grad():
                logits = self.model(input_tensor)
                probs = torch.softmax(logits, dim=1)[0].cpu().numpy()

            top_indices = np.argsort(probs)[::-1][:top_k]

            predicted_class = self.class_names[top_indices[0]]
            advice_code = self.advice_map.get(predicted_class)
            
            metadata = {}
            if advice_code:
                metadata["advice_code"] = advice_code

            result = {
                "predicted_class": predicted_class,
                "confidence": float(probs[top_indices[0]]),
                "topk": [
                    {
                        "label": self.class_names[idx],
                        "score": float(probs[idx])
                    }
                    for idx in top_indices
                ],
                "model_version": self._model_version,
                "metadata": metadata,
                "geometry": None,
            }
            logger.info("Prediction success: %s (%.2f%%)", result["predicted_class"], result["confidence"] * 100)
            return result
        except Exception as e:
            logger.error("Inference failed: %s", str(e))
            raise

    def get_model_info(self) -> Dict[str, str]:
        """
        Return model metadata.
        """
        return {
            "model_name": "RiceLeafClassifier",
            "model_version": self._model_version,
            "architecture": self.config.get("model_name", "resnet18"),
            "num_classes": str(len(self.class_names)),
        }
