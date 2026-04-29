"""Smart Farm AI Engine FastAPI application entry point.

Start the server with::

    uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000

Configuration
-------------
The following environment variables are recognised:

* ``MODEL_CHECKPOINT_PATH``  path to the ``.pth`` checkpoint file.
  Defaults to ``models/rice/rice_leaf_classifier/best_model.pth`` when unset.
* ``MODEL_LABELS_FILE``      path to ``labels.json``.
  Defaults to ``models/rice/rice_leaf_classifier/labels.json``.
* ``MODEL_CONFIG_FILE``      path to ``config.yaml``.
  Defaults to ``models/rice/rice_leaf_classifier/config.yaml``.
"""

import logging
import os
# Fix: Ensure UTF-8 is explicitly handled
import sys
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from ai_engine.common.adapters.image_adapter import ImageLoadError
from ai_engine.common.health import router as common_router
from ai_engine.crops.rice.inference.api import router as rice_router, set_classifier
from ai_engine.crops.oil_palm.inference.api import router as oil_palm_router

# ------------------------------------------------------------------
# Logging
# ------------------------------------------------------------------

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
logger = logging.getLogger(__name__)

# ------------------------------------------------------------------
# Environment-based configuration
# ------------------------------------------------------------------

CROP_PROFILE = os.environ.get("CROP_PROFILE", "rice").lower()

MODEL_CHECKPOINT_PATH = os.environ.get(
    "MODEL_CHECKPOINT_PATH",
    "models/rice/rice_leaf_classifier/best_model.pth",
)
MODEL_LABELS_FILE = os.environ.get(
    "MODEL_LABELS_FILE",
    "models/rice/rice_leaf_classifier/labels.json",
)
MODEL_CONFIG_FILE = os.environ.get(
    "MODEL_CONFIG_FILE",
    "models/rice/rice_leaf_classifier/config.yaml",
)
MODEL_ADVICE_FILE = os.environ.get(
    "MODEL_ADVICE_FILE",
    "models/rice/rice_leaf_classifier/advice_map.yaml",
)


# ------------------------------------------------------------------
# Lifespan: model preloading at startup
# ------------------------------------------------------------------

@asynccontextmanager
async def lifespan(app: FastAPI):
    """Load the AI model at startup based on CROP_PROFILE."""
    logger.info("=== Smart Farm AI Engine starting [%s] ===", CROP_PROFILE)
    
    if CROP_PROFILE == "rice":
        try:
            from ai_engine.crops.rice.inference.rice_leaf_classifier import RiceLeafClassifier

            logger.info("Loading Rice Model Assets...")
            classifier = RiceLeafClassifier(
                checkpoint_path=MODEL_CHECKPOINT_PATH,
                labels_file=MODEL_LABELS_FILE,
                config_file=MODEL_CONFIG_FILE,
                advice_file=MODEL_ADVICE_FILE,
            )
            set_classifier(classifier)
            logger.info("Rice model loaded successfully.")
        except Exception as exc:
            logger.error("FATAL: Rice model failed to load at startup: %s", exc)
            # Fail fast: orchestration (Docker/K8s) will see the crash and not route traffic.
            raise RuntimeError(f"Required model assets not found or invalid: {exc}") from exc
    elif CROP_PROFILE == "oil_palm":
        logger.info("Oil Palm mode: Using mock/future YOLOv8 predictor.")
    
    yield
    logger.info("=== Smart Farm AI Engine shutting down ===")


# ------------------------------------------------------------------
# FastAPI app
# ------------------------------------------------------------------

app = FastAPI(
    title=f"Smart Farm AI Engine ({CROP_PROFILE.upper()})",
    description="AI inference micro-service for crop disease detection.",
    version="0.2.0",
    lifespan=lifespan,
)

# CORS: allow trusted dashboard/backend origins (override in env)
cors_origins_env = os.environ.get(
    "CORS_ORIGINS",
    "http://localhost:8088,http://127.0.0.1:8088",
)
allow_origins = [origin.strip() for origin in cors_origins_env.split(",") if origin.strip()]
allow_credentials = os.environ.get("CORS_ALLOW_CREDENTIALS", "false").lower() == "true"
allow_methods = [
    method.strip()
    for method in os.environ.get("CORS_ALLOW_METHODS", "GET,POST,OPTIONS").split(",")
    if method.strip()
]
allow_headers = [
    header.strip()
    for header in os.environ.get("CORS_ALLOW_HEADERS", "Authorization,Content-Type").split(",")
    if header.strip()
]

app.add_middleware(
    CORSMiddleware,
    allow_origins=allow_origins,
    allow_credentials=allow_credentials,
    allow_methods=allow_methods,
    allow_headers=allow_headers,
)

app.include_router(common_router, prefix="/api/v1")
app.include_router(rice_router, prefix="/api/v1")
app.include_router(oil_palm_router, prefix="/api/v1")


# ------------------------------------------------------------------
# Global exception handlers: graceful degradation
# ------------------------------------------------------------------

@app.exception_handler(ImageLoadError)
async def image_load_error_handler(request: Request, exc: ImageLoadError):
    """Image could not be decoded: client fault (422)."""
    logger.warning("ImageLoadError: %s", exc)
    return JSONResponse(
        status_code=422,
        content={"status": "error", "message": str(exc)},
    )


@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    """Catch-all for unexpected errors: never expose a raw 500."""
    logger.error("Unhandled exception on %s %s: %s", request.method, request.url.path, exc, exc_info=True)
    return JSONResponse(
        status_code=500,
        content={"status": "error", "message": "Internal inference error"},
    )
