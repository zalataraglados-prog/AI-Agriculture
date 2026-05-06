# Ganoderma Risk Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm Ganoderma Risk Classifier |
| Task | Image classification (risk screening) |
| Crop | Oil Palm |
| Framework | ResNet / EfficientNet (planned) |
| Current Status | **mock** |
| Model Version | `oil_palm_ganoderma_mock_v1` |

## Intended Use

Screen trunk base images for Ganoderma boninense / basal stem rot (BSR) risk indicators.
This is a **risk screening** model, NOT a diagnostic tool. All positive outputs are labeled as
`suspected`, never `confirmed`.

## Labels

See [labels.json](./labels.json): `healthy`, `suspected_risk`, `other_stress_unknown`.

v1 uses conservative 3-class labels. Future expansion (when data quality supports it):
`suspected_early`, `moderate`, `severe`, `dead_or_collapsed`.

## Training Data

Not yet trained. See `datasets/oil_palm/manifests/ganoderma_risk.example.json` for planned data sources.

## Evaluation

Not yet evaluated. See [metrics.example.json](./metrics.example.json) for the metrics template.

## Ethical Considerations

- **This model does NOT provide a medical or agronomic diagnosis.**
- All risk outputs must be labeled as `suspected_not_confirmed`.
- Expert or laboratory confirmation is REQUIRED before any disease management action.
- False negatives may delay treatment; false positives may cause unnecessary intervention.
- Model output should always be accompanied by `advice: recheck trunk base; expert confirmation required`.

## Limitations

- Ganoderma public data is sparse and often limited to hyperspectral or small samples.
- RGB-only classification may miss early-stage infections.
- Performance varies significantly across palm age, variety, and environmental conditions.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| mock_v1 | 2026-05 | Mock predictor established. No real weights. |
