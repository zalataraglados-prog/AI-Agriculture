from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, Field


class TopKItem(BaseModel):
    predicted_class: str
    confidence: float = Field(ge=0.0, le=1.0)


class PredictionResponse(BaseModel):
    status: str = "success"
    predicted_class: str
    confidence: float = Field(ge=0.0, le=1.0)
    model_version: str
    topk: list[TopKItem] = Field(default_factory=list)
    metadata: dict[str, Any] = Field(default_factory=dict)


class PredictionResult(BaseModel):
    task: str
    label: str
    confidence: float = Field(ge=0.0, le=1.0)
    geometry: dict[str, Any] = Field(default_factory=dict)
    metadata: dict[str, Any] = Field(default_factory=dict)


class PredictionEnvelope(BaseModel):
    status: Literal["success", "error"] = "success"
    results: list[PredictionResult] = Field(default_factory=list)
    geometry: list[dict[str, Any]] = Field(default_factory=list)
    metadata: dict[str, Any] = Field(default_factory=dict)
    model_version: str
