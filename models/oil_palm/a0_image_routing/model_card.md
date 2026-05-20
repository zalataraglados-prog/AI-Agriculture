# A0 Image Routing Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm A0 Structure Router |
| Task | Object detection (structure routing gatekeeper) |
| Crop | Oil Palm |
| Framework | YOLOv8 |
| Current Status | **trained baseline artifact available; runtime integration pending** |
| Model Version | `oil_palm_a0_yolo_structure_detector_v1` |

## Intended Use

Detect supported oil palm structures and generate bbox candidates for user
confirmation before downstream routing. A0 validates the user-selected
`image_role`, but it does not predict disease, maturity, growth, or yield.

## Labels

See [labels.json](./labels.json): `fruit_bunch`, `trunk_base`, `crown_region`.

`unknown` is not a detector class. It is an inference/route status used when no
supported structure is detected, confidence is too low, or the detected
structure does not match the requested role.

## Design Principles

- A0 does not replace the user's `image_role` selection in v1.
- A0 returns bbox candidates that the user can confirm or reject.
- Rejected candidates should be masked in a derived image before downstream
  inference.
- Multiple candidates should return `needs_user_confirmation`.
- Once stable, A0 may be promoted to an `auto` routing mode.

## Training Data

Trained on dataset version `roboflow_a0_2026_05_17`, generated from the A0
Roboflow COCO bootstrap exports and converted into project YOLO format.

- Images: 396
- Bboxes: 1174
- Split: train 279 / val 58 / test 59
- Labels: `fruit_bunch`, `trunk_base`, `crown_region`
- Empty-label negative samples: not included in this baseline

## Evaluation

See [metrics.json](./metrics.json) for the recorded training metrics.

Best validation epoch: 76.

| Metric | Value |
|--------|-------|
| mAP50 | 0.8988 |
| mAP50-95 | 0.52789 |
| Precision | 0.78039 |
| Recall | 0.94318 |

Route-level metrics such as `no_supported_structure_recall` and
`negative_sample_false_positive_rate` are not evaluated yet because this
baseline dataset does not include negative empty-label samples.

## Ethical Considerations

- Misrouting can cause downstream predictors to receive the wrong image type or
  the wrong tree's evidence.
- A0 errors should fail safely: when uncertain, return `uncertain` or
  `role_mismatch` rather than a wrong route.

## Limitations

- v1 is a 3-class structure detector.
- Quality assessment is not part of A0 v1 scope.
- A0 cannot prove that a candidate belongs to the scanned tree; user
  confirmation remains required for ambiguous or multi-object images.
- This artifact has not yet been wired into the runtime predictor registry.
- The first trained dataset lacks negative samples, so unsupported-image
  behavior still needs a follow-up evaluation set.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| placeholder | 2026-05 | Directory structure established. No real model, mock contract only. |
| training_bootstrap | 2026-05 | Added Roboflow COCO adapter, YOLO dataset preparation, and training config. |
| oil_palm_a0_yolo_structure_detector_v1 | 2026-05-18 | First Colab-trained YOLOv8n A0 baseline. Weights stored under ignored local runs directory; metadata recorded in metrics.json and inference_config.yaml. |
