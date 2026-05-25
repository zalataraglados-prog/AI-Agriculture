from __future__ import annotations

import logging
import os
from typing import Any

from ai_engine.common.predictors.base import BasePredictor, PredictorContext
from ai_engine.common.registry import ModelRegistry
from ai_engine.crops.oil_palm.inference.a0_yolo_predictor import (
    build_a0_yolo_predictor_from_env,
)
from ai_engine.crops.oil_palm.inference.ganoderma_resnet_predictor import (
    build_ganoderma_resnet_predictor_from_env,
)
from ai_engine.crops.oil_palm.inference.mock_predictors import (
    A0StructureMockPredictor,
    FFBMockPredictor,
    GanodermaMockPredictor,
    GrowthMockPredictor,
    UAVTileMockPredictor,
)

logger = logging.getLogger(__name__)

IMAGE_ROLE_TO_TASK = {
    "fruit": "ffb_maturity",
    "trunk_base": "ganoderma_risk",
    "crown": "growth_vigor",
    "uav_tile": "uav_tree_crown",
}

# ---------------------------------------------------------------------------
# Environment-based configuration for model mode switching
# ---------------------------------------------------------------------------

OIL_PALM_MODEL_MODE = os.environ.get("OIL_PALM_MODEL_MODE", "mock").lower()


def get_oil_palm_confidence_threshold(default: float = 0.5) -> float:
    """Parse the future real-predictor threshold only when it is needed."""
    raw = os.environ.get("OIL_PALM_CONFIDENCE_THRESHOLD")
    if raw is None or raw.strip() == "":
        return default
    try:
        return float(raw)
    except ValueError:
        logger.warning(
            "Invalid OIL_PALM_CONFIDENCE_THRESHOLD=%r, falling back to %s",
            raw,
            default,
        )
        return default

# Mock predictor classes keyed by task
_MOCK_PREDICTORS: dict[str, type[BasePredictor]] = {
    "a0_structure_detection": A0StructureMockPredictor,
    "ffb_maturity": FFBMockPredictor,
    "ganoderma_risk": GanodermaMockPredictor,
    "growth_vigor": GrowthMockPredictor,
    "uav_tree_crown": UAVTileMockPredictor,
}


class OilPalmPipeline:
    crop = "oil_palm"

    def __init__(self, registry: ModelRegistry, model_mode: str = "mock") -> None:
        self.registry = registry
        self.model_mode = model_mode

    @property
    def supported_image_roles(self) -> list[str]:
        return sorted(IMAGE_ROLE_TO_TASK)

    def analyze(
        self,
        *,
        image_bytes: bytes,
        image_role: str,
        tree_code: str | None = None,
        session_id: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized_role = image_role.strip().lower()
        task = IMAGE_ROLE_TO_TASK.get(normalized_role)
        if task is None:
            raise ValueError(f"unsupported image_role: {image_role}")

        context = PredictorContext(
            crop=self.crop,
            task=task,
            image_role=normalized_role,
            tree_code=tree_code,
            session_id=session_id,
            metadata=metadata or {},
        )
        predictor = self.registry.get(self.crop, task)
        envelope = predictor.predict(image_bytes, context)
        envelope.setdefault("metadata", {})
        envelope["metadata"].update(context.metadata)
        envelope["metadata"]["pipeline"] = f"oil_palm_{self.model_mode}_pipeline_v1"
        envelope["metadata"]["model_mode"] = self.model_mode
        if self.model_mode == "hybrid":
            envelope["metadata"]["model_mode_note"] = (
                "hybrid uses configured real oil palm predictors and keeps "
                "unconfigured tasks on safe mock fallback"
            )
        envelope["metadata"]["registered_capabilities"] = self.registry.capabilities(self.crop)
        return envelope

    def detect_structures(
        self,
        *,
        image_bytes: bytes,
        image_role: str,
        tree_code: str | None = None,
        session_id: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized_role = image_role.strip().lower()
        context = PredictorContext(
            crop=self.crop,
            task="a0_structure_detection",
            image_role=normalized_role,
            tree_code=tree_code,
            session_id=session_id,
            metadata=metadata or {},
        )
        predictor = self.registry.get(self.crop, "a0_structure_detection")
        envelope = predictor.predict(image_bytes, context)
        envelope.setdefault("metadata", {})
        envelope["metadata"].update(
            {
                "tree_code": tree_code,
                "session_id": session_id,
                "pipeline": f"oil_palm_{self.model_mode}_pipeline_v1",
                "model_mode": self.model_mode,
                "registered_capabilities": self.registry.capabilities(self.crop),
            }
        )
        return envelope


def build_default_oil_palm_pipeline() -> OilPalmPipeline:
    """Build the oil palm pipeline based on OIL_PALM_MODEL_MODE.

    Mode semantics:
    - mock:   All tasks use mock predictors. No real weights needed.
              Suitable for demo, CI, and environments without model files.
    - real:   Require configured real predictors for the currently integrated
              A0 and Ganoderma runtime branches.
    - hybrid: Use configured real predictors and keep missing per-task models on
              safe mock fallback.
    """
    mode = OIL_PALM_MODEL_MODE
    if mode not in ("mock", "real", "hybrid"):
        logger.warning(
            "Unknown OIL_PALM_MODEL_MODE=%r, falling back to 'mock'", mode
        )
        mode = "mock"

    logger.info("Building OilPalmPipeline in mode=%s", mode)
    if mode == "hybrid":
        logger.info(
            "Oil palm hybrid mode will use configured real predictors and keep "
            "unavailable oil palm tasks on safe mock fallback."
        )

    registry = ModelRegistry()

    real_predictors: dict[str, BasePredictor] = {}
    if mode in ("real", "hybrid"):
        try:
            real_predictors["a0_structure_detection"] = build_a0_yolo_predictor_from_env()
            logger.info("  [a0_structure_detection] registered real YOLO predictor")
        except Exception as exc:
            if mode == "real":
                raise RuntimeError(
                    "OIL_PALM_MODEL_MODE=real requires a usable A0 YOLO predictor. "
                    "Set OIL_PALM_A0_MODEL_PATH/OIL_PALM_A0_LABELS_FILE and install "
                    "oil palm inference dependencies."
                ) from exc
            logger.warning(
                "A0 real predictor unavailable in hybrid mode; falling back to mock: %s",
                exc,
            )

        try:
            real_predictors["ganoderma_risk"] = build_ganoderma_resnet_predictor_from_env()
            logger.info("  [ganoderma_risk] registered real ResNet18 predictor")
        except Exception as exc:
            if mode == "real":
                raise RuntimeError(
                    "OIL_PALM_MODEL_MODE=real requires a usable Ganoderma ResNet "
                    "predictor. Set OIL_PALM_GANODERMA_MODEL_PATH/"
                    "OIL_PALM_GANODERMA_LABELS_FILE and install oil palm inference "
                    "dependencies."
                ) from exc
            logger.warning(
                "Ganoderma real predictor unavailable in hybrid mode; falling back "
                "to mock: %s",
                exc,
            )

    for predictor in real_predictors.values():
        registry.register(predictor)

    # Tasks that get registered into the pipeline. Downstream oil palm tasks
    # remain mock until their own real model branches land.
    tasks_to_register = [
        "a0_structure_detection",
        "ffb_maturity",
        "ganoderma_risk",
        "growth_vigor",
        "uav_tree_crown",
    ]

    for task in tasks_to_register:
        if task in real_predictors:
            continue
        mock_cls = _MOCK_PREDICTORS.get(task)
        if mock_cls:
            registry.register(mock_cls())
            logger.info("  [%s] registered mock predictor", task)

    return OilPalmPipeline(registry, model_mode=mode)
