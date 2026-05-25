from __future__ import annotations

import io
import json
import os
from pathlib import Path
from typing import Any, Mapping

from PIL import Image

from ai_engine.common.predictors.base import BasePredictor, PredictorContext


DEFAULT_LABELS = ["healthy", "suspected_risk"]
DEFAULT_MODEL_VERSION = "oil_palm_ganoderma_resnet18_v1.0.0_offline"
DEFAULT_MEAN = [0.485, 0.456, 0.406]
DEFAULT_STD = [0.229, 0.224, 0.225]
DEFAULT_INPUT_SIZE = [224, 224]


class GanodermaResNetPredictor(BasePredictor):
    crop = "oil_palm"
    task = "ganoderma_risk"
    mode = "real"

    def __init__(
        self,
        *,
        weights_path: str | Path,
        labels_path: str | Path = "models/oil_palm/ganoderma_risk/labels.json",
        metrics_path: str | Path | None = "models/oil_palm/ganoderma_risk/metrics.json",
        device: str = "cpu",
        model_version: str | None = None,
    ) -> None:
        self.weights_path = Path(weights_path)
        self.labels_path = Path(labels_path)
        self.metrics_path = Path(metrics_path) if metrics_path else None

        if not self.weights_path.exists():
            raise FileNotFoundError(
                f"Ganoderma ResNet weights not found: {self.weights_path}"
            )
        if not self.labels_path.exists():
            raise FileNotFoundError(
                f"Ganoderma labels file not found: {self.labels_path}"
            )

        labels = _load_string_list(self.labels_path)
        self.metrics = _load_json_mapping(self.metrics_path)
        self.class_labels = _active_class_labels(labels, self.metrics)
        self.model_version = (
            model_version
            or str(self.metrics.get("model_version") or DEFAULT_MODEL_VERSION)
        )
        self.input_size = _preprocessing_list(
            self.metrics,
            "resize",
            DEFAULT_INPUT_SIZE,
            expected_len=2,
        )
        normalize = self.metrics.get("preprocessing", {}).get("normalize", {})
        self.normalize_mean = _float_list(
            normalize.get("mean"),
            DEFAULT_MEAN,
            expected_len=3,
        )
        self.normalize_std = _float_list(
            normalize.get("std"),
            DEFAULT_STD,
            expected_len=3,
        )

        try:
            import torch
            from torchvision import models, transforms
        except Exception as exc:  # pragma: no cover - exercised when deps missing
            raise RuntimeError(
                "torch and torchvision are required for the real Ganoderma predictor"
            ) from exc

        self.torch = torch
        self.device = _resolve_device(torch, device)
        self.transform = transforms.Compose(
            [
                transforms.Resize(tuple(self.input_size)),
                transforms.ToTensor(),
                transforms.Normalize(mean=self.normalize_mean, std=self.normalize_std),
            ]
        )
        self.model = models.resnet18(weights=None)
        in_features = int(getattr(self.model.fc, "in_features", 512))
        self.model.fc = torch.nn.Linear(in_features, len(self.class_labels))

        state_dict = torch.load(str(self.weights_path), map_location=self.device)
        self.model.load_state_dict(_normalize_state_dict(state_dict))
        self.model.to(self.device)
        self.model.eval()

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        image = Image.open(io.BytesIO(image_bytes)).convert("RGB")
        tensor = self.transform(image).unsqueeze(0).to(self.device)
        with self.torch.no_grad():
            logits = self.model(tensor)
            probabilities = self.torch.nn.functional.softmax(logits, dim=1)[0]

        probability_values = _to_float_list(probabilities)
        if len(probability_values) != len(self.class_labels):
            raise RuntimeError(
                "Ganoderma probability vector does not match active class labels"
            )

        top_index = max(range(len(probability_values)), key=probability_values.__getitem__)
        label = self.class_labels[top_index]
        confidence = round(float(probability_values[top_index]), 4)
        probability_by_label = {
            class_label: round(float(probability_values[idx]), 4)
            for idx, class_label in enumerate(self.class_labels)
        }
        geometry = {"type": "whole_image"}
        metadata = self._metadata(
            context=context,
            label=label,
            probability_by_label=probability_by_label,
        )

        return {
            "status": "success",
            "results": [
                {
                    "task": self.task,
                    "label": label,
                    "confidence": confidence,
                    "geometry": geometry,
                    "metadata": metadata,
                }
            ],
            "geometry": [geometry],
            "metadata": metadata,
            "model_version": self.model_version,
        }

    def _metadata(
        self,
        *,
        context: PredictorContext,
        label: str,
        probability_by_label: dict[str, float],
    ) -> dict[str, Any]:
        suspected = label == "suspected_risk"
        return {
            "crop": context.crop,
            "task": self.task,
            "image_role": context.image_role,
            "tree_code": context.tree_code,
            "session_id": context.session_id,
            "mock": False,
            "model_profile": "ganoderma_resnet18_risk_classifier",
            "active_labels": self.class_labels,
            "reserved_labels": _reserved_labels(self.metrics),
            "probabilities": probability_by_label,
            "risk_status": label,
            "risk_language": "suspected_not_confirmed",
            "diagnosis_status": "not_confirmed",
            "diagnosis_policy": (
                "Ganoderma v1 is a risk-screening classifier and does not output "
                "confirmed disease."
            ),
            "user_review_status": "confirmed_by_default",
            "advice": (
                "recheck trunk base and request expert confirmation"
                if suspected
                else "continue routine monitoring"
            ),
            "weights_path": str(self.weights_path),
            **(context.metadata or {}),
        }


def build_ganoderma_resnet_predictor_from_env() -> GanodermaResNetPredictor:
    weights_path = os.environ.get(
        "OIL_PALM_GANODERMA_MODEL_PATH",
        "models/oil_palm/ganoderma_risk/best.pth",
    )
    labels_path = os.environ.get(
        "OIL_PALM_GANODERMA_LABELS_FILE",
        "models/oil_palm/ganoderma_risk/labels.json",
    )
    metrics_path = os.environ.get(
        "OIL_PALM_GANODERMA_METRICS_FILE",
        "models/oil_palm/ganoderma_risk/metrics.json",
    )
    device = os.environ.get("OIL_PALM_GANODERMA_DEVICE", "cpu")
    model_version = os.environ.get("OIL_PALM_GANODERMA_MODEL_VERSION") or None
    return GanodermaResNetPredictor(
        weights_path=weights_path,
        labels_path=labels_path,
        metrics_path=metrics_path,
        device=device,
        model_version=model_version,
    )


def _load_json_mapping(path: Path | None) -> dict[str, Any]:
    if path is None or not path.exists():
        return {}
    with path.open("r", encoding="utf-8") as source:
        data = json.load(source)
    if not isinstance(data, dict):
        raise ValueError(f"Ganoderma metrics must be a JSON object: {path}")
    return data


def _load_string_list(path: Path) -> list[str]:
    with path.open("r", encoding="utf-8") as source:
        labels = json.load(source)
    if not isinstance(labels, list) or not all(isinstance(item, str) for item in labels):
        raise ValueError(f"Ganoderma labels must be a JSON list of strings: {path}")
    return labels


def _active_class_labels(project_labels: list[str], metrics: Mapping[str, Any]) -> list[str]:
    class_order = metrics.get("class_order")
    if isinstance(class_order, dict) and class_order:
        labels = [
            str(class_order[key])
            for key in sorted(class_order, key=lambda value: int(str(value)))
        ]
    else:
        raw = metrics.get("active_training_labels") or DEFAULT_LABELS
        labels = list(raw) if isinstance(raw, list) else DEFAULT_LABELS

    if not labels or not all(isinstance(item, str) for item in labels):
        raise ValueError("Ganoderma active class labels must be a list of strings")
    missing = [label for label in labels if label not in project_labels]
    if missing:
        raise ValueError(f"Ganoderma active labels are missing from labels file: {missing}")
    return labels


def _reserved_labels(metrics: Mapping[str, Any]) -> list[str]:
    labels = metrics.get("labels")
    active = metrics.get("active_training_labels") or DEFAULT_LABELS
    if not isinstance(labels, list) or not isinstance(active, list):
        return ["other_stress_unknown"]
    return [str(label) for label in labels if label not in active]


def _preprocessing_list(
    metrics: Mapping[str, Any],
    key: str,
    default: list[float],
    *,
    expected_len: int,
) -> list[int]:
    preprocessing = metrics.get("preprocessing", {})
    value = preprocessing.get(key) if isinstance(preprocessing, dict) else None
    values = _float_list(value, default, expected_len=expected_len)
    return [int(item) for item in values]


def _float_list(
    value: Any,
    default: list[float],
    *,
    expected_len: int,
) -> list[float]:
    if not isinstance(value, list) or len(value) != expected_len:
        return list(default)
    try:
        return [float(item) for item in value]
    except (TypeError, ValueError):
        return list(default)


def _resolve_device(torch: Any, device: str) -> str:
    requested = (device or "cpu").strip().lower()
    if requested == "auto":
        cuda = getattr(torch, "cuda", None)
        if cuda is not None and callable(getattr(cuda, "is_available", None)):
            return "cuda" if cuda.is_available() else "cpu"
        return "cpu"
    return requested


def _normalize_state_dict(value: Any) -> Any:
    if isinstance(value, dict):
        for key in ("model_state_dict", "state_dict"):
            nested = value.get(key)
            if isinstance(nested, dict):
                value = nested
                break
    if isinstance(value, dict):
        normalized = {}
        for key, tensor in value.items():
            clean_key = key[7:] if isinstance(key, str) and key.startswith("module.") else key
            normalized[clean_key] = tensor
        return normalized
    return value


def _to_float_list(value: Any) -> list[float]:
    if hasattr(value, "detach"):
        value = value.detach()
    if hasattr(value, "cpu"):
        value = value.cpu()
    if hasattr(value, "tolist"):
        value = value.tolist()
    return [float(item) for item in value]
