# AI Engine

`ai_engine` is the modular FastAPI inference service for AI-Agriculture.

## Architecture

- `common/`: shared adapters, schemas, and global endpoints (for example health).
- `crops/rice/`: rice inference and training modules.
- `crops/oil_palm/`: oil palm placeholder/mock modules for future expansion.
- `main.py`: app composition entrypoint that includes routers.
- `infer.py`: CLI inference utility (rice classifier).

## Run API Service

```bash
uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000
```

## Environment Variables

| Variable | Description | Default |
|---|---|---|
| `CROP_PROFILE` | Active crop profile (`rice` or `oil_palm`) | `rice` |
| `MODEL_CHECKPOINT_PATH` | Rice checkpoint path | `models/rice/rice_leaf_classifier/best_model.pth` |
| `MODEL_LABELS_FILE` | Rice label map | `models/rice/rice_leaf_classifier/labels.json` |
| `MODEL_CONFIG_FILE` | Rice model config | `models/rice/rice_leaf_classifier/config.yaml` |
| `MODEL_ADVICE_FILE` | Rice advice map | `models/rice/rice_leaf_classifier/advice_map.yaml` |

## API Endpoints

### Global

- `GET /api/v1/health` (profile-safe service health)

### Rice

- `POST /api/v1/predict` (legacy rice endpoint, backward compatibility)
- `POST /api/v1/rice/predict` (explicit rice endpoint)
- `GET /api/v1/rice/health` (rice model health)

### Oil Palm (Mock)

- `POST /api/v1/oil-palm/analyze-image`
- `POST /api/v1/oil-palm/analyze-session`
- `POST /api/v1/oil-palm/analyze-uav-mission`

## CLI Inference

```bash
python -m ai_engine.infer \
  --image-path test.jpg \
  --checkpoint-path outputs/.../best_model.pth
```
