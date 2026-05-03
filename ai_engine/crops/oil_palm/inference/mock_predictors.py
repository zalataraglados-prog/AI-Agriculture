from __future__ import annotations

import hashlib
from typing import Any

from ai_engine.common.predictors.base import BasePredictor, PredictorContext


def _digest(image_bytes: bytes, salt: str) -> bytes:
    return hashlib.sha256(salt.encode("utf-8") + image_bytes).digest()


def _unit(digest: bytes, idx: int) -> float:
    return round(digest[idx] / 255.0, 4)


def _bbox(digest: bytes, start: int) -> dict[str, Any]:
    x = round(0.08 + _unit(digest, start) * 0.62, 4)
    y = round(0.08 + _unit(digest, start + 1) * 0.62, 4)
    w = round(0.12 + _unit(digest, start + 2) * 0.18, 4)
    h = round(0.12 + _unit(digest, start + 3) * 0.18, 4)
    return {"type": "bbox", "x": x, "y": y, "w": w, "h": h}


def _envelope(
    *,
    model_version: str,
    task: str,
    label: str,
    confidence: float,
    geometry: dict[str, Any],
    context: PredictorContext,
    metadata: dict[str, Any],
) -> dict[str, Any]:
    merged_metadata = {
        "crop": context.crop,
        "task": task,
        "image_role": context.image_role,
        "tree_code": context.tree_code,
        "session_id": context.session_id,
        "mock": True,
        **metadata,
    }
    return {
        "status": "success",
        "results": [
            {
                "task": task,
                "label": label,
                "confidence": confidence,
                "geometry": geometry,
                "metadata": merged_metadata,
            }
        ],
        "geometry": [geometry],
        "metadata": merged_metadata,
        "model_version": model_version,
    }


class FFBMockPredictor(BasePredictor):
    crop = "oil_palm"
    task = "ffb_maturity"
    model_version = "oil_palm_ffb_mock_v1"
    mode = "mock"

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        digest = _digest(image_bytes, self.task)
        labels = ["unripe", "underripe", "ripe", "overripe", "abnormal"]
        label = labels[digest[0] % len(labels)]
        confidence = round(0.58 + _unit(digest, 1) * 0.34, 4)
        return _envelope(
            model_version=self.model_version,
            task=self.task,
            label=label,
            confidence=confidence,
            geometry=_bbox(digest, 2),
            context=context,
            metadata={
                "maturity": label,
                "advice": "mock: verify fruit bunch maturity before harvest action",
            },
        )


class GanodermaMockPredictor(BasePredictor):
    crop = "oil_palm"
    task = "ganoderma_risk"
    model_version = "oil_palm_ganoderma_mock_v1"
    mode = "mock"

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        digest = _digest(image_bytes, self.task)
        labels = ["healthy", "suspected_early", "moderate", "other_stress_unknown"]
        label = labels[digest[0] % len(labels)]
        confidence = round(0.54 + _unit(digest, 1) * 0.32, 4)
        return _envelope(
            model_version=self.model_version,
            task=self.task,
            label=label,
            confidence=confidence,
            geometry=_bbox(digest, 2),
            context=context,
            metadata={
                "risk_status": label,
                "risk_language": "suspected_not_confirmed",
                "advice": "mock: recheck trunk base; expert confirmation required for diagnosis",
            },
        )


class GrowthMockPredictor(BasePredictor):
    crop = "oil_palm"
    task = "growth_vigor"
    model_version = "oil_palm_growth_mock_v1"
    mode = "mock"

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        digest = _digest(image_bytes, self.task)
        vigor = round(0.35 + _unit(digest, 0) * 0.55, 4)
        if vigor >= 0.72:
            label = "strong_vigor"
        elif vigor >= 0.5:
            label = "moderate_vigor"
        else:
            label = "weak_vigor"
        return _envelope(
            model_version=self.model_version,
            task=self.task,
            label=label,
            confidence=round(0.57 + _unit(digest, 1) * 0.28, 4),
            geometry={"type": "crown_region", "coverage": round(vigor, 4)},
            context=context,
            metadata={
                "vigor_index": vigor,
                "advice": "mock: compare with UAV or future crown observations",
            },
        )


class UAVTileMockPredictor(BasePredictor):
    crop = "oil_palm"
    task = "uav_tree_crown"
    model_version = "oil_palm_uav_tile_mock_v1"
    mode = "mock"

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        digest = _digest(image_bytes, self.task)
        geometry = _bbox(digest, 2)
        confidence = round(0.6 + _unit(digest, 1) * 0.32, 4)
        return _envelope(
            model_version=self.model_version,
            task=self.task,
            label="candidate_crown",
            confidence=confidence,
            geometry=geometry,
            context=context,
            metadata={
                "candidate_type": "tree_crown",
                "advice": "mock: route candidate crown to UAV review before creating tree asset",
            },
        )
