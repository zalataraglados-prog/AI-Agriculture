from __future__ import annotations

from dataclasses import dataclass

from ai_engine.common.predictors.base import BasePredictor


@dataclass(frozen=True)
class PredictorKey:
    crop: str
    task: str


class ModelRegistry:
    def __init__(self) -> None:
        self._predictors: dict[PredictorKey, BasePredictor] = {}

    def register(self, predictor: BasePredictor) -> None:
        key = PredictorKey(crop=predictor.crop, task=predictor.task)
        self._predictors[key] = predictor

    def get(self, crop: str, task: str) -> BasePredictor:
        key = PredictorKey(crop=crop, task=task)
        try:
            return self._predictors[key]
        except KeyError as exc:
            raise KeyError(f"predictor not registered for crop={crop} task={task}") from exc

    def capabilities(self, crop: str | None = None) -> list[dict[str, str]]:
        items = []
        for key, predictor in sorted(
            self._predictors.items(),
            key=lambda item: (item[0].crop, item[0].task),
        ):
            if crop is not None and key.crop != crop:
                continue
            items.append(
                {
                    "crop": key.crop,
                    "task": key.task,
                    "model_version": predictor.model_version,
                    "mode": predictor.mode,
                }
            )
        return items
