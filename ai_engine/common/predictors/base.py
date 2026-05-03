from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class PredictorContext:
    crop: str
    task: str
    image_role: str | None = None
    tree_code: str | None = None
    session_id: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)


class BasePredictor(ABC):
    crop: str
    task: str
    model_version: str
    mode: str = "mock"

    @abstractmethod
    def predict(self, image_bytes: bytes, context: PredictorContext) -> dict[str, Any]:
        """Return a stable prediction envelope for one image."""
