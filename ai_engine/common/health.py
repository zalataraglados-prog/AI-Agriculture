from __future__ import annotations

from datetime import datetime, timezone

from fastapi import APIRouter

router = APIRouter()


@router.get("/health")
def health() -> dict:
    return {
        "status": "ok",
        "service": "ai_engine",
        "ts": datetime.now(timezone.utc).isoformat(),
    }
