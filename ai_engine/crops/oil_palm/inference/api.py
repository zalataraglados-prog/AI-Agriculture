from __future__ import annotations

from datetime import datetime, timezone
from typing import Literal

from fastapi import APIRouter, File, Form, UploadFile
from pydantic import BaseModel, Field

from ai_engine.common.adapters.image_adapter import validate_image_bytes
from ai_engine.common.schemas.prediction import PredictionResponse
from ai_engine.crops.oil_palm.inference.predictor import analyze_oil_palm_image

router = APIRouter()


class CapabilityItem(BaseModel):
    capability: str
    v1_scope: str
    input_contract: str
    output_fields: list[str]
    metric: str


class RouteDoc(BaseModel):
    crop: Literal["oil_palm"] = "oil_palm"
    version: str = "v1"
    updated_at: str
    capabilities: list[CapabilityItem]


@router.get("/oil-palm/route")
def oil_palm_route() -> dict:
    payload = RouteDoc(
        updated_at=datetime.now(timezone.utc).isoformat(),
        capabilities=[
            CapabilityItem(
                capability="disease_analysis",
                v1_scope="single-frame frond risk classification",
                input_contract="multipart image file",
                output_fields=["predicted_class", "confidence", "metadata.disease_rate", "metadata.is_diseased"],
                metric="top1 confidence / disease_rate",
            ),
            CapabilityItem(
                capability="growth_analysis",
                v1_scope="derive growth vigor from image risk",
                input_contract="multipart image file",
                output_fields=["metadata.growth_vigor_index"],
                metric="growth_vigor_index",
            ),
            CapabilityItem(
                capability="weather_prediction",
                v1_scope="risk proxy based on current visual stress",
                input_contract="multipart image file + optional weather text",
                output_fields=["metadata.weather_risk_score"],
                metric="weather_risk_score",
            ),
            CapabilityItem(
                capability="yield_assessment",
                v1_scope="yield risk proxy based on disease and growth",
                input_contract="multipart image file",
                output_fields=["metadata.yield_risk_score"],
                metric="yield_risk_score",
            ),
        ],
    )
    return payload.model_dump()


@router.post("/oil-palm/predict-v1")
async def oil_palm_predict_v1(
    file: UploadFile = File(...),
    location: str = Form(default="unknown_location"),
    plantation_id: str = Form(default="unknown_plantation"),
) -> dict:
    image_bytes = await file.read()
    validate_image_bytes(image_bytes)
    raw = analyze_oil_palm_image(image_bytes)

    # attach self-explaining context for frontend direct rendering
    raw.setdefault("metadata", {})
    raw["metadata"].update(
        {
            "crop": "oil_palm",
            "location": location,
            "plantation_id": plantation_id,
            "capability_bundle": [
                "disease_analysis",
                "growth_analysis",
                "weather_prediction",
                "yield_assessment",
            ],
        }
    )
    return PredictionResponse(
        predicted_class=raw["predicted_class"],
        confidence=raw["confidence"],
        model_version=raw["model_version"],
        topk=raw.get("topk", []),
        metadata=raw.get("metadata", {}),
    ).model_dump()


@router.post("/oil-palm/analyze-image")
async def analyze_image(file: UploadFile = File(...)) -> dict:
    image_bytes = await file.read()
    validate_image_bytes(image_bytes)
    return analyze_oil_palm_image(image_bytes)


@router.post("/oil-palm/analyze-session")
def analyze_session(session_id: str = Form(...), frame_count: int = Form(default=0)) -> dict:
    return {
        "status": "success",
        "session_id": session_id,
        "frame_count": frame_count,
        "summary": {
            "dominant_risk": "frond_nutrient_stress",
            "recommendation": "inspect nutrient balance and leaf lesions",
        },
    }


@router.post("/oil-palm/analyze-uav-mission")
def analyze_uav_mission(mission_id: str = Form(...), image_count: int = Form(default=0)) -> dict:
    return {
        "status": "success",
        "mission_id": mission_id,
        "image_count": image_count,
        "result": {
            "hotspots": max(image_count // 5, 1) if image_count else 0,
            "risk_level": "medium" if image_count > 0 else "unknown",
        },
    }
