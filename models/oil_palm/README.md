# Oil Palm Models

This directory stores oil palm model metadata: labels, model cards, metrics
templates, and future checkpoint mount points. It does not store trained
weights in Git.

## Principles

- Model weights are not committed. Files such as `*.pt`, `*.pth`, and `*.onnx`
  are ignored by `.gitignore`.
- Production should mount model artifacts through deployment configuration.
- Every real model update should include a model card and metrics JSON.
- `OIL_PALM_MODEL_MODE` defines the runtime mode, but this foundation branch
  does not register real predictors yet.

## Directory Layout

```text
models/oil_palm/
  README.md
  ffb_maturity/
    model_card.md
    labels.json
    metrics.example.json
    best.pt              # ignored
  uav_tree_crown/
    model_card.md
    labels.json
    metrics.example.json
    best.pt              # ignored
  ganoderma_risk/
    model_card.md
    labels.json
    metrics.example.json
    best.pt              # ignored
  a0_image_routing/
    model_card.md
    labels.json
    metrics.example.json
    best.pt              # ignored
```

## Task Overview

| Task | Type | Suggested framework | Current status | Labels |
| --- | --- | --- | --- | --- |
| `ffb_maturity` | object detection + maturity class | YOLO family | mock | 6 |
| `uav_tree_crown` | object detection | YOLO family | mock | 1 |
| `ganoderma_risk` | image classification | ResNet / EfficientNet | mock | 3 |
| `a0_image_routing` | image classification | MobileNet / EfficientNet | placeholder only | 4 |

`growth_vigor` currently remains a mock pipeline task. The planned v1 is likely
an aggregation score rather than a single-image trained model, so there is no
`OIL_PALM_GROWTH_MODEL_PATH` in this foundation branch.

## Runtime Mode

| Variable | Default | Meaning |
| --- | --- | --- |
| `OIL_PALM_MODEL_MODE` | `mock` | Oil palm runtime mode: `mock`, `real`, or `hybrid` |
| `OIL_PALM_CONFIDENCE_THRESHOLD` | `0.5` | Future real predictor confidence threshold |

Future per-task branches will enable these path variables:

- `OIL_PALM_FFB_MODEL_PATH`
- `OIL_PALM_UAV_CROWN_MODEL_PATH`
- `OIL_PALM_GANODERMA_MODEL_PATH`
- `OIL_PALM_A0_MODEL_PATH`

Mode semantics in this foundation branch:

- `mock`: all registered oil palm tasks use mock predictors.
- `hybrid`: safe mock fallback. Real predictors are not registered yet.
- `real`: fail-fast with a clear message until per-task branches implement real
  predictors.
