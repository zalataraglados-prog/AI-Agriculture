"""Real predictor skeletons for oil palm tasks.

Each class below is a placeholder for a future real model predictor.
In this foundation branch, all predict() methods raise NotImplementedError.
Actual implementations will be added in per-task feature branches:

- FFBRealPredictor          -> feature/ffb-maturity-model
- UAVTileRealPredictor      -> feature/uav-crown-detection-model
- GanodermaRealPredictor    -> feature/ganoderma-risk-model
- GrowthRealPredictor       -> feature/growth-vigor-scoring
- A0RoutingRealPredictor    -> feature/oil-palm-a0-routing-model

Design contract:
- Every real predictor returns the same PredictionEnvelope structure as its mock counterpart.
- __init__ accepts model_path and confidence_threshold but does NOT load the model
  in this foundation branch.
- mode = "real" distinguishes from mock predictors in registry capabilities output.
"""

from __future__ import annotations

from typing import Any

from ai_engine.common.predictors.base import BasePredictor, PredictorContext


class FFBRealPredictor(BasePredictor):
    """Real FFB fruit bunch detection + maturity classification predictor."""

    crop = "oil_palm"
    task = "ffb_maturity"
    model_version = "oil_palm_ffb_real_v0"
    mode = "real"

    def __init__(self, model_path: str, confidence_threshold: float = 0.5) -> None:
        self.model_path = model_path
        self.confidence_threshold = confidence_threshold

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        raise NotImplementedError(
            "FFBRealPredictor not yet implemented. "
            "Implement in feature/ffb-maturity-model branch."
        )


class UAVTileRealPredictor(BasePredictor):
    """Real UAV tile crown detection predictor."""

    crop = "oil_palm"
    task = "uav_tree_crown"
    model_version = "oil_palm_uav_crown_real_v0"
    mode = "real"

    def __init__(self, model_path: str, confidence_threshold: float = 0.5) -> None:
        self.model_path = model_path
        self.confidence_threshold = confidence_threshold

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        raise NotImplementedError(
            "UAVTileRealPredictor not yet implemented. "
            "Implement in feature/uav-crown-detection-model branch."
        )


class GanodermaRealPredictor(BasePredictor):
    """Real Ganoderma / BSR risk classification predictor."""

    crop = "oil_palm"
    task = "ganoderma_risk"
    model_version = "oil_palm_ganoderma_real_v0"
    mode = "real"

    def __init__(self, model_path: str, confidence_threshold: float = 0.5) -> None:
        self.model_path = model_path
        self.confidence_threshold = confidence_threshold

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        raise NotImplementedError(
            "GanodermaRealPredictor not yet implemented. "
            "Implement in feature/ganoderma-risk-model branch."
        )


class GrowthRealPredictor(BasePredictor):
    """Real growth vigor scoring predictor.

    Note: v1 growth scoring will likely be an aggregation model (combining
    UAV crown area, FFB data, Ganoderma risk, etc.) rather than a single-image
    classifier. The real implementation may differ significantly from other
    real predictors.
    """

    crop = "oil_palm"
    task = "growth_vigor"
    model_version = "oil_palm_growth_real_v0"
    mode = "real"

    def __init__(self, model_path: str, confidence_threshold: float = 0.5) -> None:
        self.model_path = model_path
        self.confidence_threshold = confidence_threshold

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        raise NotImplementedError(
            "GrowthRealPredictor not yet implemented. "
            "Implement in feature/growth-vigor-scoring branch."
        )


class A0RoutingRealPredictor(BasePredictor):
    """Real A0 image role routing predictor."""

    crop = "oil_palm"
    task = "a0_image_routing"
    model_version = "oil_palm_a0_real_v0"
    mode = "real"

    def __init__(self, model_path: str, confidence_threshold: float = 0.5) -> None:
        self.model_path = model_path
        self.confidence_threshold = confidence_threshold

    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        raise NotImplementedError(
            "A0RoutingRealPredictor not yet implemented. "
            "Implement in feature/oil-palm-a0-routing-model branch."
        )
