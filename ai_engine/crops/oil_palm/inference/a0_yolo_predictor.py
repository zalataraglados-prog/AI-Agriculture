from __future__ import annotations

import io
import json
import os
from pathlib import Path
from typing import Any

import yaml
from PIL import Image

from ai_engine.common.predictors.base import BasePredictor, PredictorContext


ROLE_TO_LABEL = {
    "fruit": "fruit_bunch",
    "trunk_base": "trunk_base",
    "crown": "crown_region",
}

LABEL_TO_ROLE = {label: role for role, label in ROLE_TO_LABEL.items()}


class A0YoloPredictor(BasePredictor):
    crop = "oil_palm"
    task = "a0_structure_detection"
    mode = "real"

    def __init__(
        self,
        *,
        weights_path: str | Path,
        labels_path: str | Path = "models/oil_palm/a0_image_routing/labels.json",
        config_path: str | Path | None = "models/oil_palm/a0_image_routing/inference_config.yaml",
        confidence_threshold: float | None = None,
    ) -> None:
        self.weights_path = Path(weights_path)
        self.labels_path = Path(labels_path)
        self.config_path = Path(config_path) if config_path else None
        self.config = self._load_config(self.config_path)
        self.labels = self._load_labels(self.labels_path)
        self.model_version = str(
            self.config.get("model_version", "oil_palm_a0_yolo_structure_detector_v1")
        )
        self.confidence_threshold = float(
            confidence_threshold
            if confidence_threshold is not None
            else self.config.get("confidence_threshold", 0.5)
        )
        self.iou_threshold = float(self.config.get("iou_threshold", 0.7))
        self.input_size = int(self.config.get("input_size", 960))
        self.max_detections = int(self.config.get("max_detections", 300))

        if not self.weights_path.exists():
            raise FileNotFoundError(f"A0 YOLO weights not found: {self.weights_path}")
        if not self.labels_path.exists():
            raise FileNotFoundError(f"A0 labels file not found: {self.labels_path}")

        try:
            from ultralytics import YOLO
        except Exception as exc:  # pragma: no cover - exercised when deps missing
            raise RuntimeError(
                "ultralytics is required for the real A0 YOLO predictor"
            ) from exc

        self.model = YOLO(str(self.weights_path))

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        requested_role = (context.image_role or "").strip().lower()
        image = Image.open(io.BytesIO(image_bytes)).convert("RGB")
        width, height = image.size

        prediction = self.model.predict(
            source=image,
            imgsz=self.input_size,
            conf=self.confidence_threshold,
            iou=self.iou_threshold,
            max_det=self.max_detections,
            verbose=False,
        )
        first = prediction[0] if prediction else None
        candidates = self._candidates_from_result(first, width, height)
        requested_label = ROLE_TO_LABEL.get(requested_role)
        detected_labels = sorted({item["label"] for item in candidates})
        detected_roles = sorted(
            {item["suggested_role"] for item in candidates if item.get("suggested_role")}
        )

        if not candidates:
            route_status = "no_supported_structure_detected"
        elif requested_label is None:
            route_status = "unsupported_requested_role"
        elif requested_label not in detected_labels:
            route_status = "role_mismatch"
        else:
            route_status = "needs_user_confirmation"

        results = []
        geometry = []
        for item in candidates:
            candidate_geometry = item["geometry"]
            geometry.append(candidate_geometry)
            results.append(
                {
                    "task": self.task,
                    "label": item["label"],
                    "confidence": item["confidence"],
                    "geometry": candidate_geometry,
                    "metadata": {
                        "candidate_id": item["candidate_id"],
                        "suggested_role": item["suggested_role"],
                        "selected_by_default": True,
                    },
                }
            )

        metadata = {
            "crop": context.crop,
            "task": self.task,
            "image_role": requested_role,
            "requested_image_role": requested_role,
            "detected_roles": detected_roles,
            "route_status": route_status,
            "a0_candidates": candidates,
            "requires_user_confirmation": route_status == "needs_user_confirmation",
            "mock": False,
            "model_profile": "a0_yolo_structure_router",
            "weights_path": str(self.weights_path),
            "confidence_threshold": self.confidence_threshold,
            "iou_threshold": self.iou_threshold,
            **(context.metadata or {}),
        }
        return {
            "status": "success",
            "results": results,
            "geometry": geometry,
            "metadata": metadata,
            "model_version": self.model_version,
        }

    def _candidates_from_result(
        self,
        result: Any,
        width: int,
        height: int,
    ) -> list[dict[str, Any]]:
        boxes = getattr(result, "boxes", None)
        if boxes is None:
            return []

        xyxy_values = _to_list(getattr(boxes, "xyxy", []))
        confidence_values = _to_list(getattr(boxes, "conf", []))
        class_values = _to_list(getattr(boxes, "cls", []))
        candidates: list[dict[str, Any]] = []

        for idx, xyxy in enumerate(xyxy_values):
            if len(xyxy) < 4:
                continue
            class_idx = int(float(class_values[idx])) if idx < len(class_values) else -1
            if class_idx < 0 or class_idx >= len(self.labels):
                continue
            label = self.labels[class_idx]
            confidence = (
                float(confidence_values[idx]) if idx < len(confidence_values) else 0.0
            )
            if confidence < self.confidence_threshold:
                continue
            x0, y0, x1, y1 = [float(v) for v in xyxy[:4]]
            geometry = _normalized_bbox(x0, y0, x1, y1, width, height)
            candidates.append(
                {
                    "candidate_id": f"a0_{LABEL_TO_ROLE.get(label, label)}_{idx + 1:03d}",
                    "label": label,
                    "suggested_role": LABEL_TO_ROLE.get(label, "unknown"),
                    "confidence": round(confidence, 4),
                    "geometry": geometry,
                }
            )

        return candidates

    @staticmethod
    def _load_config(path: Path | None) -> dict[str, Any]:
        if path is None or not path.exists():
            return {}
        with open(path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f) or {}
        if not isinstance(data, dict):
            raise ValueError(f"A0 inference config must be a mapping: {path}")
        return data

    @staticmethod
    def _load_labels(path: Path) -> list[str]:
        with open(path, "r", encoding="utf-8") as f:
            labels = json.load(f)
        if not isinstance(labels, list) or not all(isinstance(v, str) for v in labels):
            raise ValueError(f"A0 labels must be a JSON list of strings: {path}")
        return labels


def build_a0_yolo_predictor_from_env() -> A0YoloPredictor:
    weights_path = os.environ.get(
        "OIL_PALM_A0_MODEL_PATH",
        "models/oil_palm/a0_image_routing/runs/a0_yolo_structure_detector_v1/weights/best.pt",
    )
    labels_path = os.environ.get(
        "OIL_PALM_A0_LABELS_FILE",
        "models/oil_palm/a0_image_routing/labels.json",
    )
    config_path = os.environ.get(
        "OIL_PALM_A0_CONFIG_FILE",
        "models/oil_palm/a0_image_routing/inference_config.yaml",
    )
    threshold = os.environ.get("OIL_PALM_CONFIDENCE_THRESHOLD")
    parsed_threshold = float(threshold) if threshold not in (None, "") else None
    return A0YoloPredictor(
        weights_path=weights_path,
        labels_path=labels_path,
        config_path=config_path,
        confidence_threshold=parsed_threshold,
    )


def _to_list(value: Any) -> list[Any]:
    if hasattr(value, "detach"):
        value = value.detach()
    if hasattr(value, "cpu"):
        value = value.cpu()
    if hasattr(value, "numpy"):
        value = value.numpy()
    if hasattr(value, "tolist"):
        return value.tolist()
    return list(value)


def _normalized_bbox(
    x0: float,
    y0: float,
    x1: float,
    y1: float,
    width: int,
    height: int,
) -> dict[str, Any]:
    width = max(width, 1)
    height = max(height, 1)
    nx0 = max(0.0, min(x0 / width, 1.0))
    ny0 = max(0.0, min(y0 / height, 1.0))
    nx1 = max(0.0, min(x1 / width, 1.0))
    ny1 = max(0.0, min(y1 / height, 1.0))
    return {
        "type": "bbox",
        "x": round(min(nx0, nx1), 4),
        "y": round(min(ny0, ny1), 4),
        "w": round(abs(nx1 - nx0), 4),
        "h": round(abs(ny1 - ny0), 4),
    }
