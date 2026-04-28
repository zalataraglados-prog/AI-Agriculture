"""Global, profile-safe health endpoint."""

import os

from fastapi import APIRouter

router = APIRouter(tags=["common"])


@router.get("/health", summary="Global service health check")
async def health():
    """Profile-safe health endpoint that does not require crop model loading."""
    return {
        "status": "ok",
        "service": "smart-farm-ai-engine",
        "crop_profile": os.environ.get("CROP_PROFILE", "rice").lower(),
    }
