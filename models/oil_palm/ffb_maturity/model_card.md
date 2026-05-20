# FFB Maturity Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm FFB Maturity Detector |
| Task | Object detection + maturity classification |
| Crop | Oil Palm |
| Framework | YOLOv8n |
| Current Status | **offline trained v1; not integrated into AI Engine runtime** |
| Model Version | `oil_palm_ffb_yolov8n_v1.0.0_offline` |

## Intended Use

Detect fresh fruit bunches (FFB) in oil palm field images and classify visible
maturity stage. The output should support harvest-readiness review for a known
tree/session, not estimate whole-tree yield from a single photo.

## Labels

Project-standard labels are `flower`, `unripe`, `underripe`, `ripe`,
`overripe`, and `abnormal`.

The v1 handoff metrics only report `unripe`, `underripe`, `ripe`, and
`overripe`. The teammate must confirm whether `flower` and `abnormal` were not
present, filtered out, or not trained.

## Training Data

The recorded v1 metrics refer to the Roboflow source listed in
`datasets/oil_palm/manifests/ffb_maturity.example.json`. Dataset version, split
counts, exact class mapping, and license details still require teammate
confirmation before this model is treated as merge-ready production evidence.

## Evaluation

Recorded handoff metrics:

| Metric | Value |
|--------|-------|
| Precision | 0.837 |
| Recall | 0.837 |
| mAP50 | 0.892 |
| mAP50-95 | 0.666 |

Per-class AP50 was reported for four active classes only. See
`metrics.json` for the normalized schema and pending fields.

## Runtime Status

This branch records offline training metadata only. AI Engine still uses the
safe mock FFB predictor unless a future runtime branch adds a real predictor and
loads weights through `OIL_PALM_FFB_MODEL_PATH`.

## Limitations

- v1 does not estimate yield quantity from a single image.
- Occluded or partially visible bunches may be misclassified.
- `flower` and `abnormal` coverage is unconfirmed.
- Weight path, hash, split counts, and dataset version are not yet recorded.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| `oil_palm_ffb_yolov8n_v1.0.0_offline` | 2026-05-19 | Offline YOLOv8n handoff metrics recorded; runtime integration pending. |
| `oil_palm_ffb_mock_v1` | 2026-05 | Mock predictor established. No real weights. |
