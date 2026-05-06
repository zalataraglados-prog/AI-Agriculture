# AI Engine

AI Engine is the FastAPI inference service for the multi-crop AI-Agriculture
platform. It keeps crop modules isolated while sharing common schemas,
predictor interfaces, image adapters, and health endpoints.

## Directory Layout

```text
ai_engine/
  main.py
  infer.py
  common/
    health.py
    registry.py
    predictors/
      base.py
    adapters/
      image_adapter.py
    schemas/
      prediction.py
  crops/
    rice/
      inference/
      training/
    oil_palm/
      inference/
        api.py
        predictor.py
        mock_predictors.py
      pipeline.py
      training/
        common/
        data_importers/
        ffb_maturity/
        uav_tree_crown/
        ganoderma_risk/
        a0_image_routing/
```

## Crop Boundaries

- `common/` contains cross-crop infrastructure only.
- `crops/rice/` and `crops/oil_palm/` must not import business logic from each other.
- `CROP_PROFILE` selects the active profile at runtime.
- Legacy rice endpoints remain available for compatibility.

## Run Locally

```bash
# Rice profile (default)
CROP_PROFILE=rice uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000

# Oil palm profile
CROP_PROFILE=oil_palm uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000
```

## Environment Variables

| Variable | Purpose | Default |
| --- | --- | --- |
| `CROP_PROFILE` | Crop profile: `rice` or `oil_palm` | `rice` |
| `MODEL_CHECKPOINT_PATH` | Rice checkpoint path | `models/rice/rice_leaf_classifier/best_model.pth` |
| `MODEL_LABELS_FILE` | Rice labels path | `models/rice/rice_leaf_classifier/labels.json` |
| `MODEL_CONFIG_FILE` | Rice config path | `models/rice/rice_leaf_classifier/config.yaml` |
| `MODEL_ADVICE_FILE` | Rice advice map path | `models/rice/rice_leaf_classifier/advice_map.yaml` |
| `CORS_ORIGINS` | Allowed dashboard/backend origins | `http://localhost:8088,http://127.0.0.1:8088` |
| `OIL_PALM_MODEL_MODE` | Oil palm mode: `mock`, `real`, or `hybrid` | `mock` |
| `OIL_PALM_CONFIDENCE_THRESHOLD` | Future real predictor confidence threshold | `0.5` |

Future oil palm model path variables are documented in `ai_engine/.env.example`.
They are reserved for later per-task model branches.

## API Endpoints

Common:

- `GET /api/v1/health`

Rice:

- `POST /api/v1/predict`
- `POST /api/v1/rice/predict`
- `GET /api/v1/rice/health`

Oil palm:

- `GET /api/v1/oil-palm/route`
- `POST /api/v1/oil-palm/analyze`
- `POST /api/v1/oil-palm/predict-v1`
- `POST /api/v1/oil-palm/analyze-image`
- `POST /api/v1/oil-palm/analyze-session`
- `POST /api/v1/oil-palm/analyze-uav-mission`

## Oil Palm Model Modes

`feature/oil-palm-model-data-foundation` defines the configuration surface but
does not implement real oil palm predictors yet.

- `mock`: all tasks use mock predictors. This is the default and is safe for CI,
  demos, and environments without weights.
- `hybrid`: safe fallback mode in this foundation branch. It still uses mock
  predictors until a later per-task branch registers real predictors.
- `real`: fail-fast in this foundation branch. Use it only after per-task model
  branches implement and register real predictors.

Current oil palm image role routing:

| `image_role` | Task |
| --- | --- |
| `fruit` | `ffb_maturity` |
| `trunk_base` | `ganoderma_risk` |
| `crown` | `growth_vigor` |
| `uav_tile` | `uav_tree_crown` |

A0 image routing has dataset/model placeholders only. It is not registered into
the oil palm pipeline until `feature/oil-palm-a0-routing-model`.

## Response Contract

Oil palm mock and future real predictors should preserve this envelope:

- `status`
- `results[]`
- `geometry[]`
- `metadata`
- `model_version`

This keeps Cloud, frontend, assessment reports, and OpenClaw tools stable while
real models are added incrementally.
