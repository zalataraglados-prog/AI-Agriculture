from __future__ import annotations

from typing import Any

from ai_engine.common.predictors.base import PredictorContext
from ai_engine.common.registry import ModelRegistry
from ai_engine.crops.oil_palm.inference.mock_predictors import (
    FFBMockPredictor,
    GanodermaMockPredictor,
    GrowthMockPredictor,
    UAVTileMockPredictor,
)


IMAGE_ROLE_TO_TASK = {
    "fruit": "ffb_maturity",
    "trunk_base": "ganoderma_risk",
    "crown": "growth_vigor",
    "uav_tile": "uav_tree_crown",
}


class OilPalmPipeline:
    crop = "oil_palm"

    def __init__(self, registry: ModelRegistry) -> None:
        self.registry = registry

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
        envelope["metadata"]["pipeline"] = "oil_palm_mock_pipeline_v1"
        envelope["metadata"]["registered_capabilities"] = self.registry.capabilities(self.crop)
        return envelope


def build_default_oil_palm_pipeline() -> OilPalmPipeline:
    registry = ModelRegistry()
    registry.register(FFBMockPredictor())
    registry.register(GanodermaMockPredictor())
    registry.register(GrowthMockPredictor())
    registry.register(UAVTileMockPredictor())
    return OilPalmPipeline(registry)
