# A0 Image Routing Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm A0 Image Role Router |
| Task | Image classification (routing gatekeeper) |
| Crop | Oil Palm |
| Framework | MobileNet / EfficientNet (planned) |
| Current Status | **mock** (not yet registered in pipeline) |
| Model Version | `oil_palm_a0_mock_v1` |

## Intended Use

Classify uploaded images by their `image_role`: fruit, trunk_base, crown, or unknown.
A0 is the "entry gatekeeper" — it does NOT predict disease, maturity, or growth.
Its job is to verify or correct the user-specified `image_role` before routing to the
appropriate downstream predictor (FFB, Ganoderma, Growth, UAV).

## Labels

See [labels.json](./labels.json): `fruit`, `trunk_base`, `crown`, `unknown`.

## Design Principles

- A0 does **not replace** the user's `image_role` selection in v1. It validates and suggests corrections.
- When A0 disagrees with the user's selection, it returns `needs_manual_role_confirmation`.
- Once stable, A0 may be promoted to `auto` routing mode where it selects `image_role` automatically.
- A0 should be lightweight (MobileNet-class) to minimize latency overhead.

## Training Data

Not yet trained. Training data can be bootstrapped from existing FFB, Ganoderma, UAV crown
datasets by re-labeling images according to their role category.

## Evaluation

Not yet evaluated. See [metrics.example.json](./metrics.example.json) for the metrics template.

## Ethical Considerations

- Misrouting can cause downstream predictors to receive wrong image types, producing misleading results.
- A0 errors should fail safely: when uncertain, default to `unknown` rather than a wrong role.

## Limitations

- v1 is a 4-class classifier; future versions may add finer distinctions (e.g., leaf, bark close-up).
- Quality assessment (blur, exposure, distance) is NOT part of A0 v1 scope.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| placeholder | 2026-05 | Directory structure established. No model, no predictor registered. |
