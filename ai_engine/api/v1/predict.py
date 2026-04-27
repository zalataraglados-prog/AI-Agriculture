"""Prediction endpoint 鈥?POST /api/v1/predict

This module is part of **L1 (API Layer)**.  Its sole responsibilities
are:

1. Receive the HTTP request and read raw bytes from the upload.
2. Delegate to L2 (adapter) for image decoding.
3. Delegate to L3 (core) for inference.
4. Wrap the result in a Pydantic ``PredictionResponse`` and return it.

It MUST NOT contain any image processing or inference logic.
"""

import logging

from fastapi import APIRouter, File, UploadFile

from ai_engine.common.adapters.image_adapter import load_image_from_bytes
from ai_engine.common.schemas.prediction import (
    ErrorResponse,
    PredictionItem,
    PredictionResponse,
    PredictionResult,
)

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/v1", tags=["prediction"])

# ------------------------------------------------------------------
# Module-level reference to the pre-loaded model.
# Populated by ``ai_engine.main`` at startup 鈥?see ``lifespan()``.
# ------------------------------------------------------------------
_classifier = None


def set_classifier(classifier) -> None:
    """Called once at application startup to inject the model instance."""
    global _classifier
    _classifier = classifier


def _get_classifier():
    """Return the pre-loaded classifier or raise if not ready."""
    if _classifier is None:
        raise RuntimeError("Model not loaded 鈥?the service is not ready")
    return _classifier


# ------------------------------------------------------------------
# Routes
# ------------------------------------------------------------------

@router.post(
    "/predict",
    response_model=PredictionResponse,
    responses={
        422: {"model": ErrorResponse, "description": "Invalid image or request"},
        500: {"model": ErrorResponse, "description": "Internal inference error"},
    },
    summary="Predict crop disease from an image",
    description=(
        "Upload a single JPEG/PNG image.  The service returns the "
        "predicted disease class, confidence score, and top-k "
        "alternative predictions."
    ),
)
async def predict(file: UploadFile = File(...)):
    """Receive an image and return disease classification results."""
    logger.info("Received prediction request: filename=%s", file.filename)

    # L1: read raw bytes from the upload
    image_bytes = await file.read()

    # L2: adapter converts bytes 鈫?PIL.Image
    image = load_image_from_bytes(image_bytes)

    # L3: core engine runs inference
    raw_result = _get_classifier().predict(image)

    # Wrap in Pydantic schema
    topk_items = [PredictionItem(**item) for item in raw_result["topk"]]
    result = PredictionResult(
        predicted_class=raw_result["predicted_class"],
        confidence=raw_result["confidence"],
        topk=topk_items,
        model_version=raw_result["model_version"],
        metadata=raw_result.get("metadata", {}),
        geometry=raw_result.get("geometry"),
    )

    logger.info("Prediction complete: %s (%.2f%%)", result.predicted_class, result.confidence * 100)
    return PredictionResponse(results=[result])


@router.get(
    "/health",
    summary="Service health check",
    description="Returns service status and model availability.  Used by Docker / orchestrator health probes.",
)
async def health():
    """Return service health status."""
    classifier = _get_classifier()
    model_info = classifier.get_model_info()
    return {
        "status": "ok",
        "service": "smart-farm-ai-engine",
        "version": "0.1.0",
        "model": model_info,
    }
