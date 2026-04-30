# Issue #65 Delivery: Oil Palm AI Route + V1

## Scope delivered
- Capability route endpoint:
  - `GET /api/v1/oil-palm/route`
- V1 callable endpoint:
  - `POST /api/v1/oil-palm/predict-v1`
- Existing mock compatibility endpoints retained:
  - `POST /api/v1/oil-palm/analyze-image`
  - `POST /api/v1/oil-palm/analyze-session`
  - `POST /api/v1/oil-palm/analyze-uav-mission`

## Capability route (v1)
The route endpoint defines four capability lanes and their contracts:
1. `disease_analysis`
2. `growth_analysis`
3. `weather_prediction`
4. `yield_assessment`

## V1 output contract (frontend-friendly)
`/oil-palm/predict-v1` returns:
- `predicted_class`
- `confidence`
- `model_version`
- `topk[]`
- `metadata` with:
  - `disease_rate`
  - `is_diseased`
  - `growth_vigor_index`
  - `weather_risk_score`
  - `yield_risk_score`
  - `crop`
  - `location`
  - `plantation_id`
  - `capability_bundle[]`

## Quick validation
```bash
uvicorn ai_engine.main:app --host 0.0.0.0 --port 8000
curl -s http://127.0.0.1:8000/api/v1/oil-palm/route
curl -s -X POST http://127.0.0.1:8000/api/v1/oil-palm/predict-v1 \
  -F "file=@test.jpg" \
  -F "location=sector_01" \
  -F "plantation_id=plm_demo_01"
```

## Notes
- Current implementation is deterministic mock logic for v1 contract stability.
- It is ready for replacement by real model inference without changing API schema.
