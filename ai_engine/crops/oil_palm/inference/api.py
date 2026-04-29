from fastapi import APIRouter, File, UploadFile
import logging

logger = logging.getLogger(__name__)

router = APIRouter(tags=["oil_palm"])

@router.post(
    "/oil-palm/analyze-image",
    summary="Analyze oil palm image (Mock)",
)
async def analyze_image(file: UploadFile = File(...)):
    logger.info("Oil Palm: analyze-image request")
    return {
        "crop": "oil_palm",
        "status": "mock",
        "message": "Oil palm pipeline placeholder is ready.",
        "results": [],
    }

@router.post(
    "/oil-palm/analyze-session",
    summary="Analyze oil palm session (Mock)",
)
async def analyze_session():
    logger.info("Oil Palm: analyze-session request")
    return {
        "crop": "oil_palm",
        "status": "mock",
        "message": "Oil palm pipeline placeholder is ready.",
        "results": [],
    }

@router.post(
    "/oil-palm/analyze-uav-mission",
    summary="Analyze oil palm UAV mission (Mock)",
)
async def analyze_uav_mission():
    logger.info("Oil Palm: analyze-uav-mission request")
    return {
        "crop": "oil_palm",
        "status": "mock",
        "message": "Oil palm pipeline placeholder is ready.",
        "results": [],
    }
