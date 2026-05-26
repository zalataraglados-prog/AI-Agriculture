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
| `OIL_PALM_A0_MODEL_PATH` | A0 YOLO weights path | `models/oil_palm/a0_image_routing/runs/a0_yolo_structure_detector_v1/weights/best.pt` |
| `OIL_PALM_A0_LABELS_FILE` | A0 YOLO labels file | `models/oil_palm/a0_image_routing/labels.json` |
| `OIL_PALM_A0_CONFIG_FILE` | A0 inference config file | `models/oil_palm/a0_image_routing/inference_config.yaml` |
| `OIL_PALM_CONFIDENCE_THRESHOLD` | A0 confidence threshold override | `0.5` |
| `OIL_PALM_GANODERMA_MODEL_PATH` | Ganoderma ResNet18 `.pth` state_dict path | `models/oil_palm/ganoderma_risk/best.pth` |
| `OIL_PALM_GANODERMA_LABELS_FILE` | Ganoderma project labels file | `models/oil_palm/ganoderma_risk/labels.json` |
| `OIL_PALM_GANODERMA_METRICS_FILE` | Ganoderma metrics/model metadata file | `models/oil_palm/ganoderma_risk/metrics.json` |
| `OIL_PALM_GANODERMA_DEVICE` | Ganoderma runtime device: `cpu`, `cuda`, or `auto` | `cpu` |
| `OIL_PALM_GANODERMA_MODEL_VERSION` | Optional Ganoderma model version override | metrics `model_version` |
| `OIL_PALM_UAV_CROWN_MODEL_PATH` | UAV crown YOLO weights path | `models/oil_palm/uav_tree_crown/runs/uav_yolo_tree_crown_detector_v1/weights/best.pt` |
| `OIL_PALM_UAV_CROWN_LABELS_FILE` | UAV crown YOLO labels file | `models/oil_palm/uav_tree_crown/labels.json` |
| `OIL_PALM_UAV_CROWN_CONFIG_FILE` | UAV crown inference config file | `models/oil_palm/uav_tree_crown/inference_config.yaml` |
| `OIL_PALM_UAV_CROWN_CONFIDENCE_THRESHOLD` | UAV crown confidence threshold override | generic threshold or config |

`OIL_PALM_A0_MODEL_PATH` can point at the trained A0 YOLO baseline recorded in
`models/oil_palm/a0_image_routing/inference_config.yaml`.

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

The service still defaults to mock predictors. A0, Ganoderma, and UAV crown can
be enabled per task by mounting weights and switching `OIL_PALM_MODEL_MODE`.

- `mock`: all tasks use mock predictors. This is the default and is safe for CI,
  demos, and environments without weights.
- `hybrid`: registers real A0, Ganoderma, and/or UAV predictors when their
  weights and dependencies are available, then keeps missing FFB/Growth tasks
  and unconfigured predictors on safe mock fallback. This is the recommended
  incremental deployment mode.
- `real`: requires the currently integrated real A0, Ganoderma, and UAV
  predictors to load successfully.

Current oil palm image role routing:

| `image_role` | Task |
| --- | --- |
| `fruit` | `ffb_maturity` |
| `trunk_base` | `ganoderma_risk` |
| `crown` | `growth_vigor` |
| `uav_tile` | `uav_tree_crown` |

A0 structure detection can now run as a real YOLOv8 gatekeeper in `hybrid` or
`real` mode. It validates the requested `image_role`, returns bbox candidates,
and uses `route_status` values such as `needs_user_confirmation`,
`role_mismatch`, and `no_supported_structure_detected`. The weights are not
committed; mount them and point `OIL_PALM_A0_MODEL_PATH` at the mounted file.

The first trained A0 YOLO baseline is documented under
`models/oil_palm/a0_image_routing/`: `metrics.json` records training metrics,
`inference_config.yaml` records runtime settings, and
`runs/a0_yolo_structure_detector_v1/weights/best.pt` is ignored by Git.

Current A0 YOLO labels are `fruit_bunch`, `trunk_base`, and `crown_region`.
`unknown` is an inference status, not a trained bbox class.

Ganoderma v1 can now run as a real ResNet18 classifier for
`image_role=trunk_base` in `hybrid` or `real` mode. It loads a PyTorch
`state_dict` from `OIL_PALM_GANODERMA_MODEL_PATH`, uses the two active classes
recorded in `models/oil_palm/ganoderma_risk/metrics.json`, and returns only
`healthy` or `suspected_risk`. `other_stress_unknown` remains a reserved future
label. Positive outputs are risk-screening signals only; `confirmed` is not a
model diagnosis label.

UAV tree crown can now run as a real YOLOv8 detector for `image_role=uav_tile`
in `hybrid` or `real` mode once the UAV feature branch provides
`models/oil_palm/uav_tree_crown/inference_config.yaml` and the ignored
`runs/uav_yolo_tree_crown_detector_v1/weights/best.pt` artifact. It returns
tile-normalized `oil_palm_crown` bbox candidates with
`review_status=requires_human_confirmation`; Cloud remains responsible for tile
offsets, global coordinates, cross-tile NMS, confirmation, and `tree_code`
assignment.

Minimal Ganoderma v1 runtime example:

```bash
CROP_PROFILE=oil_palm \
OIL_PALM_MODEL_MODE=hybrid \
OIL_PALM_GANODERMA_MODEL_PATH=/opt/ai-agriculture/models/oil_palm/ganoderma_risk/best.pth \
uvicorn ai_engine.main:app --host 0.0.0.0 --port 8000
```

## Response Contract

Oil palm mock and real predictors should preserve this envelope:

- `status`
- `results[]`
- `geometry[]`
- `metadata`
- `model_version`

This keeps Cloud, frontend, assessment reports, and OpenClaw tools stable while
real models are added incrementally.
