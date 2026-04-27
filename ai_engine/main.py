"""Smart Farm AI Engine 鈥?FastAPI application entry point.

Start the server with::

    uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000

Configuration
-------------
The following environment variables are recognised:

* ``MODEL_CHECKPOINT_PATH``  鈥?path to the ``.pth`` checkpoint file.
  Defaults to ``models/rice/rice_leaf_classifier/best_model.pth`` when unset.
* ``MODEL_LABELS_FILE``      鈥?path to ``labels.json``.
  Defaults to ``models/rice/rice_leaf_classifier/labels.json``.
* ``MODEL_CONFIG_FILE``      鈥?path to ``config.yaml``.
  Defaults to ``models/rice/rice_leaf_classifier/config.yaml``.
"""

import logging
import os
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from ai_engine.common.adapters.image_adapter import ImageLoadError
from ai_engine.api.v1.predict import router as predict_router, set_classifier
from ai_engine.crops.rice.inference.rice_leaf_classifier import RiceLeafClassifier

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
# Lifespan 鈥?model preloading at startup
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

    logger.info("Model loaded successfully 鈥?service is ready.")
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

# CORS 鈥?allow trusted dashboard/backend origins (override in env)
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

app.include_router(predict_router)


# ------------------------------------------------------------------
# Global exception handlers 鈥?graceful degradation
# ------------------------------------------------------------------

@app.exception_handler(ImageLoadError)
async def image_load_error_handler(request: Request, exc: ImageLoadError):
    """Image could not be decoded 鈥?client's fault (422)."""
    logger.warning("ImageLoadError: %s", exc)
    return JSONResponse(
        status_code=422,
        content={"status": "error", "message": str(exc)},
    )


@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    """Catch-all for unexpected errors 鈥?never expose a raw 500."""
    logger.error("Unhandled exception on %s %s: %s", request.method, request.url.path, exc, exc_info=True)
    return JSONResponse(
        status_code=500,
        content={"status": "error", "message": "Internal inference error"},
    )
