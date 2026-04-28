from fastapi import APIRouter, File, UploadFile
from ai_engine.common.schemas.prediction import ErrorResponse, PredictionResponse, PredictionResult, PredictionItem
import logging

logger = logging.getLogger(__name__)

router = APIRouter(tags=["oil_palm"])

@router.post(
    "/oil-palm/analyze-image",
    response_model=PredictionResponse,
    summary="Analyze oil palm image (Mock)",
)
async def analyze_image(file: UploadFile = File(...)):
    logger.info("Oil Palm: analyze-image request")
    return PredictionResponse(results=[
        PredictionResult(
            predicted_class="RipeBunch",
            confidence=0.95,
            topk=[],
            model_version="oil_palm_mock_v0.1",
            metadata={"advice": "Harvest recommended"},
            geometry={"type": "box", "coords": [10, 10, 100, 100]}
        )
    ])

@router.post(
    "/oil-palm/analyze-session",
    response_model=PredictionResponse,
    summary="Analyze oil palm session (Mock)",
)
async def analyze_session():
    logger.info("Oil Palm: analyze-session request")
    return PredictionResponse(results=[])

@router.post(
    "/oil-palm/analyze-uav-mission",
    response_model=PredictionResponse,
    summary="Analyze oil palm UAV mission (Mock)",
)
async def analyze_uav_mission():
    logger.info("Oil Palm: analyze-uav-mission request")
    return PredictionResponse(results=[])
