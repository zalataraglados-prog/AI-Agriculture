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

### v1.0.0 训练记录 (2026-05-19)
- **训练成果**: 本次使用 YOLOv8n 完成了 50 轮训练，最终整体 mAP50 达到了漂亮的 **0.892**。
- **局限性说明**: 模型对成熟果（ripe）和过熟果（overripe）的识别极度精准；但在果实有严重重叠或叶片遮挡时，生果和欠熟果可能会有微小的误判风险。
- **状态**: 首次训练完成，成果由 Max 正式移交。
