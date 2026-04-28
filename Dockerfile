# ==============================================================
# Smart Farm AI Engine — Multi-profile Dockerfile
# ==============================================================
# Build Rice:      docker build --build-arg CROP_PROFILE=rice -t ai-engine-rice .
# Build Oil Palm:  docker build --build-arg CROP_PROFILE=oil_palm -t ai-engine-palm .
# ==============================================================

# ---- Stage 1: Builder ----------------------------------------
FROM python:3.11-slim AS builder

ARG CROP_PROFILE=rice
WORKDIR /build

# Copy requirements
COPY requirements/ ./requirements/
COPY requirements.txt .

# Install base dependencies
RUN pip install --no-cache-dir -r requirements.txt

# Install crop-specific dependencies
# Note: For Rice, we use the CPU-only index for torch/torchvision
RUN if [ "$CROP_PROFILE" = "rice" ]; then \
        pip install --no-cache-dir \
            torch torchvision \
            --index-url https://download.pytorch.org/whl/cpu && \
        pip install --no-cache-dir -r requirements/rice-inference.txt; \
    elif [ "$CROP_PROFILE" = "oil_palm" ]; then \
        pip install --no-cache-dir -r requirements/oil-palm-inference.txt; \
    else \
        echo "Unknown CROP_PROFILE: $CROP_PROFILE" && exit 1; \
    fi

# ---- Stage 2: Runtime ----------------------------------------
FROM python:3.11-slim

RUN useradd --create-home appuser
WORKDIR /app

# Copy installed Python packages from builder
COPY --from=builder /usr/local/lib/python3.11/site-packages /usr/local/lib/python3.11/site-packages
COPY --from=builder /usr/local/bin /usr/local/bin

# Copy application code (models are NOT baked in — mount via volume at runtime)
COPY ai_engine/ ./ai_engine/
# Models directory is expected to be mounted: -v ./models:/app/models:ro

# Environment variables
ARG CROP_PROFILE=rice
ENV CROP_PROFILE=${CROP_PROFILE}

# Default paths for Rice (can be overridden at runtime)
ENV MODEL_CHECKPOINT_PATH=/app/models/rice/rice_leaf_classifier/best_model.pth \
    MODEL_LABELS_FILE=/app/models/rice/rice_leaf_classifier/labels.json \
    MODEL_CONFIG_FILE=/app/models/rice/rice_leaf_classifier/config.yaml \
    MODEL_ADVICE_FILE=/app/models/rice/rice_leaf_classifier/advice_map.yaml

EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/api/v1/health')" || exit 1

USER appuser

# Use shell form to support environment variable expansion if needed, 
# but uvicorn module syntax is preferred.
CMD ["sh", "-c", "uvicorn ai_engine.main:app --host 0.0.0.0 --port 8000"]
