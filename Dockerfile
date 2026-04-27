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

# Copy all requirements first
COPY deploy/requirements/ ./requirements/

# Install CPU-only PyTorch (common for both currently, but could be conditional)
RUN pip install --no-cache-dir \
        torch torchvision \
        --index-url https://download.pytorch.org/whl/cpu

# Install dependencies based on profile
RUN pip install --no-cache-dir -r requirements/base.txt && \
    if [ -f "requirements/${CROP_PROFILE}.txt" ]; then \
        pip install --no-cache-dir -r "requirements/${CROP_PROFILE}.txt"; \
    fi

# ---- Stage 2: Runtime ----------------------------------------
FROM python:3.11-slim

RUN useradd --create-home appuser
WORKDIR /app

# Copy installed Python packages from builder
COPY --from=builder /usr/local/lib/python3.11/site-packages /usr/local/lib/python3.11/site-packages
COPY --from=builder /usr/local/bin /usr/local/bin

# Copy application code
COPY ai_engine/ ./ai_engine/
COPY models/ ./models/

# Environment variables (Defaults for Rice)
ENV CROP_PROFILE=rice \
    MODEL_CHECKPOINT_PATH=/app/models/rice/rice_leaf_classifier/best_model.pth \
    MODEL_LABELS_FILE=/app/models/rice/rice_leaf_classifier/labels.json \
    MODEL_CONFIG_FILE=/app/models/rice/rice_leaf_classifier/config.yaml \
    MODEL_ADVICE_FILE=/app/models/rice/rice_leaf_classifier/advice_map.yaml

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/api/v1/health')" || exit 1

USER appuser

# Use module syntax for uvicorn
CMD ["sh", "-c", "uvicorn ai_engine.main:app --host 0.0.0.0 --port 8000"]
