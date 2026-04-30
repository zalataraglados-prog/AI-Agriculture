from __future__ import annotations

from fastapi import APIRouter, File, UploadFile

from ai_engine.common.adapters.image_adapter import validate_image_bytes
from ai_engine.common.schemas.prediction import PredictionResponse

router = APIRouter()
_classifier = None


def set_classifier(classifier) -> None:
    global _classifier
    _classifier = classifier


def _predict(image_bytes: bytes) -> PredictionResponse:
    validate_image_bytes(image_bytes)
    if _classifier is not None:
        raw = _classifier.predict_bytes(image_bytes)
    else:
        raw = {
            "predicted_class": "Healthy",
            "confidence": 0.82,
            "model_version": "rice_mock_v0",
            "topk": [{"predicted_class": "Healthy", "confidence": 0.82}],
            "metadata": {"advice_code": "normal_monitoring", "disease_rate": 0.12, "is_diseased": False},
        }
    return PredictionResponse(
        predicted_class=raw["predicted_class"],
        confidence=raw["confidence"],
        model_version=raw["model_version"],
        topk=raw.get("topk", []),
        metadata=raw.get("metadata", {}),
    )


@router.post("/predict", include_in_schema=False)
async def predict_legacy(file: UploadFile = File(...)) -> dict:
    image_bytes = await file.read()
    result = _predict(image_bytes)
    return result.model_dump()


@router.post("/rice/predict")
async def predict_rice(file: UploadFile = File(...)) -> dict:
    image_bytes = await file.read()
    result = _predict(image_bytes)
    return result.model_dump()


@router.get("/rice/health")
def rice_health() -> dict:
    return {
        "status": "ok",
        "profile": "rice",
        "model_loaded": _classifier is not None,
    }
