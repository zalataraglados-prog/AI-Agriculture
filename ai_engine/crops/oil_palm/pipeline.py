from __future__ import annotations

import logging
import os
from pathlib import Path
from typing import Any

from ai_engine.common.predictors.base import BasePredictor, PredictorContext
from ai_engine.common.registry import ModelRegistry
from ai_engine.crops.oil_palm.inference.mock_predictors import (
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
OIL_PALM_CONFIDENCE_THRESHOLD = float(
    os.environ.get("OIL_PALM_CONFIDENCE_THRESHOLD", "0.5")
)

# Per-task model path environment variables
_TASK_MODEL_PATH_ENV = {
    "ffb_maturity": "OIL_PALM_FFB_MODEL_PATH",
    "uav_tree_crown": "OIL_PALM_UAV_CROWN_MODEL_PATH",
    "ganoderma_risk": "OIL_PALM_GANODERMA_MODEL_PATH",
    "a0_image_routing": "OIL_PALM_A0_MODEL_PATH",
}

# Mock predictor classes keyed by task
_MOCK_PREDICTORS: dict[str, type[BasePredictor]] = {
    "ffb_maturity": FFBMockPredictor,
    "ganoderma_risk": GanodermaMockPredictor,
    "growth_vigor": GrowthMockPredictor,
    "uav_tree_crown": UAVTileMockPredictor,
}

# Real predictor classes keyed by task (lazy-imported to avoid heavy deps in mock mode)
_REAL_PREDICTOR_IMPORT_MAP: dict[str, tuple[str, str]] = {
    "ffb_maturity": (
        "ai_engine.crops.oil_palm.inference.real_predictors",
        "FFBRealPredictor",
    ),
    "uav_tree_crown": (
        "ai_engine.crops.oil_palm.inference.real_predictors",
        "UAVTileRealPredictor",
    ),
    "ganoderma_risk": (
        "ai_engine.crops.oil_palm.inference.real_predictors",
        "GanodermaRealPredictor",
    ),
    "growth_vigor": (
        "ai_engine.crops.oil_palm.inference.real_predictors",
        "GrowthRealPredictor",
    ),
}


def _get_model_path(task: str) -> str | None:
    """Get the model weight file path from environment variable for a given task."""
    env_key = _TASK_MODEL_PATH_ENV.get(task)
    if env_key is None:
        return None
    return os.environ.get(env_key)


def _try_load_real_predictor(
    task: str, model_path: str, confidence_threshold: float
) -> BasePredictor | None:
    """Attempt to lazy-import and instantiate a real predictor for the given task.

    Returns None if the import map has no entry for the task.
    Raises ImportError or other exceptions on failure.
    """
    entry = _REAL_PREDICTOR_IMPORT_MAP.get(task)
    if entry is None:
        return None
    module_path, class_name = entry
    import importlib

    module = importlib.import_module(module_path)
    cls = getattr(module, class_name)
    return cls(model_path=model_path, confidence_threshold=confidence_threshold)


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
        envelope["metadata"]["registered_capabilities"] = self.registry.capabilities(self.crop)
        return envelope


def build_default_oil_palm_pipeline() -> OilPalmPipeline:
    """Build the oil palm pipeline based on OIL_PALM_MODEL_MODE.

    Mode semantics:
    - mock:   All tasks use mock predictors. No real weights needed.
              Suitable for demo, CI, and environments without model files.
    - real:   All declared real tasks must load real weights.
              Missing weight files cause fail-fast (RuntimeError).
              Suitable for production validation.
    - hybrid: Try real predictor if weight file exists, otherwise fallback to mock.
              Suitable for incremental model deployment.
    """
    mode = OIL_PALM_MODEL_MODE
    if mode not in ("mock", "real", "hybrid"):
        logger.warning(
            "Unknown OIL_PALM_MODEL_MODE=%r, falling back to 'mock'", mode
        )
        mode = "mock"

    logger.info("Building OilPalmPipeline in mode=%s", mode)
    registry = ModelRegistry()

    # Tasks that get registered into the pipeline
    tasks_to_register = ["ffb_maturity", "ganoderma_risk", "growth_vigor", "uav_tree_crown"]

    for task in tasks_to_register:
        if mode == "mock":
            # Pure mock: register mock predictor directly
            mock_cls = _MOCK_PREDICTORS.get(task)
            if mock_cls:
                registry.register(mock_cls())
                logger.info("  [%s] registered mock predictor", task)
            continue

        # real or hybrid: check for model weight path
        model_path = _get_model_path(task)
        weight_exists = model_path and Path(model_path).exists()

        if mode == "real":
            if not model_path:
                raise RuntimeError(
                    f"OIL_PALM_MODEL_MODE=real but no model path set for task={task}. "
                    f"Set {_TASK_MODEL_PATH_ENV.get(task, '???')} environment variable."
                )
            if not weight_exists:
                raise RuntimeError(
                    f"OIL_PALM_MODEL_MODE=real but weight file not found: {model_path} "
                    f"for task={task}"
                )
            predictor = _try_load_real_predictor(
                task, model_path, OIL_PALM_CONFIDENCE_THRESHOLD
            )
            if predictor is None:
                raise RuntimeError(
                    f"No real predictor implementation registered for task={task}"
                )
            registry.register(predictor)
            logger.info("  [%s] registered real predictor (path=%s)", task, model_path)

        elif mode == "hybrid":
            if weight_exists and model_path:
                try:
                    predictor = _try_load_real_predictor(
                        task, model_path, OIL_PALM_CONFIDENCE_THRESHOLD
                    )
                    if predictor:
                        registry.register(predictor)
                        logger.info(
                            "  [%s] registered real predictor (path=%s)", task, model_path
                        )
                        continue
                except Exception as exc:
                    logger.warning(
                        "  [%s] failed to load real predictor, falling back to mock: %s",
                        task, exc,
                    )

            # Fallback to mock
            mock_cls = _MOCK_PREDICTORS.get(task)
            if mock_cls:
                registry.register(mock_cls())
                logger.info("  [%s] registered mock predictor (hybrid fallback)", task)

    return OilPalmPipeline(registry, model_mode=mode)
