from __future__ import annotations

import hashlib


class RiceLeafClassifier:
    """Lightweight deterministic mock classifier for service continuity."""

    def __init__(self, checkpoint_path: str, labels_file: str, config_file: str, advice_file: str) -> None:
        self.checkpoint_path = checkpoint_path
        self.labels_file = labels_file
        self.config_file = config_file
        self.advice_file = advice_file
        self.model_version = "rice_mock_v1"
        self.labels = ["Healthy", "Leaf_Blast", "Leaf_Scald"]

    def predict_bytes(self, image_bytes: bytes) -> dict:
        digest = hashlib.sha1(image_bytes).hexdigest()
        idx = int(digest[:2], 16) % len(self.labels)
        confidence = 0.62 + (int(digest[2:4], 16) / 255.0) * 0.36
        predicted = self.labels[idx]
        topk = [
            {"predicted_class": predicted, "confidence": round(min(confidence, 0.99), 4)},
            {"predicted_class": self.labels[(idx + 1) % len(self.labels)], "confidence": 0.2},
        ]
        metadata = {
            "advice_code": "inspect_leaf" if predicted != "Healthy" else "normal_monitoring",
            "disease_rate": round(topk[0]["confidence"] if predicted != "Healthy" else 0.12, 4),
            "is_diseased": predicted != "Healthy",
        }
        return {
            "predicted_class": predicted,
            "confidence": topk[0]["confidence"],
            "model_version": self.model_version,
            "topk": topk,
            "metadata": metadata,
        }
