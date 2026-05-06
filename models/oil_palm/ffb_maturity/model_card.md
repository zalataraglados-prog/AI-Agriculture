# FFB Maturity Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm FFB Maturity Detector |
| Task | Object detection + maturity classification |
| Crop | Oil Palm |
| Framework | YOLOv8 (planned) |
| Current Status | **mock** |
| Model Version | `oil_palm_ffb_mock_v1` |

## Intended Use

Detect fresh fruit bunches (FFB) in oil palm field images and classify their maturity stage.
Used for harvest readiness assessment. Inputs are typically `image_role=fruit` photos taken by field workers.

## Labels

See [labels.json](./labels.json): `flower`, `unripe`, `underripe`, `ripe`, `overripe`, `abnormal`.

## Training Data

Not yet trained. See `datasets/oil_palm/manifests/ffb_maturity.example.json` for planned data sources.

## Evaluation

Not yet evaluated. See [metrics.example.json](./metrics.example.json) for the metrics template.

## Ethical Considerations

- Model output should be treated as a **recommendation**, not a definitive harvest decision.
- Local agronomist expertise should be consulted for borderline maturity cases.
- Model performance may vary across palm varieties, lighting conditions, and camera angles.

## Limitations

- v1 will not estimate yield quantity from a single image.
- Occluded or partially visible bunches may be misclassified.
- Depth information (if available in training data) is not used in v1 inference.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| mock_v1 | 2026-05 | Mock predictor established. No real weights. |
