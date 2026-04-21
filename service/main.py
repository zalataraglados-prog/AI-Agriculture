"""Smart Farm AI Engine — FastAPI application entry point.

Start the server with::

    uvicorn service.main:app --reload --host 0.0.0.0 --port 8000

Configuration
-------------
The following environment variables are recognised:

* ``MODEL_CHECKPOINT_PATH``  — path to the ``.pth`` checkpoint file.
  Defaults to ``models/rice_leaf_classifier/best_model.pth`` when unset.
* ``MODEL_LABELS_FILE``      — path to ``labels.json``.
  Defaults to ``models/rice_leaf_classifier/labels.json``.
* ``MODEL_CONFIG_FILE``      — path to ``config.yaml``.
  Defaults to ``models/rice_leaf_classifier/config.yaml``.
"""

import logging
import os
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from service.adapters.image_adapter import ImageLoadError
from service.api.v1.predict import router as predict_router, set_classifier
from service.core.rice_leaf_classifier import RiceLeafClassifier

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

MODEL_CHECKPOINT_PATH = os.environ.get(
    "MODEL_CHECKPOINT_PATH",
    "models/rice_leaf_classifier/best_model.pth",
)
MODEL_LABELS_FILE = os.environ.get(
    "MODEL_LABELS_FILE",
    "models/rice_leaf_classifier/labels.json",
)
MODEL_CONFIG_FILE = os.environ.get(
    "MODEL_CONFIG_FILE",
    "models/rice_leaf_classifier/config.yaml",
)
MODEL_ADVICE_FILE = os.environ.get(
    "MODEL_ADVICE_FILE",
    "models/rice_leaf_classifier/advice_map.yaml",
)


# ------------------------------------------------------------------
# Lifespan — model preloading at startup
# ------------------------------------------------------------------

@asynccontextmanager
async def lifespan(app: FastAPI):
    """Load the AI model at startup.  If loading fails, the process
    exits immediately so the error surfaces in logs / health probes
    rather than silently serving broken requests.
    """
    logger.info("=== Smart Farm AI Engine starting ===")
    logger.info("Checkpoint : %s", MODEL_CHECKPOINT_PATH)
    logger.info("Labels     : %s", MODEL_LABELS_FILE)
    logger.info("Config     : %s", MODEL_CONFIG_FILE)
    logger.info("Advice Map : %s", MODEL_ADVICE_FILE)

    classifier = RiceLeafClassifier(
        checkpoint_path=MODEL_CHECKPOINT_PATH,
        labels_file=MODEL_LABELS_FILE,
        config_file=MODEL_CONFIG_FILE,
        advice_file=MODEL_ADVICE_FILE,
    )
    set_classifier(classifier)

    logger.info("Model loaded successfully — service is ready.")
    yield
    logger.info("=== Smart Farm AI Engine shutting down ===")


# ------------------------------------------------------------------
# FastAPI app
# ------------------------------------------------------------------

app = FastAPI(
    title="Smart Farm AI Engine",
    description="AI inference micro-service for crop disease detection.",
    version="0.1.0",
    lifespan=lifespan,
)

# CORS — allow the Rust cloud backend to call this service
cors_origins_env = os.environ.get("CORS_ORIGINS", "*")
allow_origins = [origin.strip() for origin in cors_origins_env.split(",") if origin.strip()]

app.add_middleware(
    CORSMiddleware,
    allow_origins=allow_origins,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(predict_router)


# ------------------------------------------------------------------
# Global exception handlers — graceful degradation
# ------------------------------------------------------------------

@app.exception_handler(ImageLoadError)
async def image_load_error_handler(request: Request, exc: ImageLoadError):
    """Image could not be decoded — client's fault (422)."""
    logger.warning("ImageLoadError: %s", exc)
    return JSONResponse(
        status_code=422,
        content={"status": "error", "message": str(exc)},
    )


@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    """Catch-all for unexpected errors — never expose a raw 500."""
    logger.error("Unhandled exception on %s %s: %s", request.method, request.url.path, exc, exc_info=True)
    return JSONResponse(
        status_code=500,
        content={"status": "error", "message": "Internal inference error"},
    )
