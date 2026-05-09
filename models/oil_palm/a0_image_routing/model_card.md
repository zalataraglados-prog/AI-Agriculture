# A0 Image Routing Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm A0 Structure Router |
| Task | Object detection (structure routing gatekeeper) |
| Crop | Oil Palm |
| Framework | YOLO family (planned) |
| Current Status | **mock** (real detector not trained yet) |
| Model Version | `oil_palm_a0_detector_mock_v1` |

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

Not yet trained. Training data can be bootstrapped from existing FFB,
Ganoderma, and UAV crown datasets by adding structure bboxes and empty-label
negative samples.

## Evaluation

Not yet evaluated. See [metrics.example.json](./metrics.example.json) for the
metrics template.

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

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| placeholder | 2026-05 | Directory structure established. No real model, mock contract only. |
