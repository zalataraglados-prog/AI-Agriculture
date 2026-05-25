# Oil Palm Models

This directory stores oil palm model metadata: labels, model cards, metrics
templates, and future checkpoint mount points. It does not store trained
weights in Git.

## Principles

- Model weights are not committed. Files such as `*.pt`, `*.pth`, and `*.onnx`
  are ignored by `.gitignore`.
- Production should mount model artifacts through deployment configuration.
- Every real model update should include a model card and metrics JSON.
- `OIL_PALM_MODEL_MODE=hybrid` can register configured A0 and Ganoderma
  predictors while keeping unfinished tasks on mock fallback.

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
    metrics.json          # recorded offline handoff metrics when available
    best.pth             # ignored
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
| `ganoderma_risk` | image classification | ResNet18 | trained v1; AI Engine runtime supported | 3 |
| `a0_image_routing` | structure detection + routing | YOLOv8 | trained baseline artifact; AI Engine runtime supported | 3 |

`growth_vigor` currently remains a mock pipeline task. The planned v1 is likely
an aggregation score rather than a single-image trained model, so there is no
`OIL_PALM_GROWTH_MODEL_PATH` in this foundation branch.

## Runtime Mode

| Variable | Default | Meaning |
| --- | --- | --- |
| `OIL_PALM_MODEL_MODE` | `mock` | Oil palm runtime mode: `mock`, `real`, or `hybrid` |
| `OIL_PALM_CONFIDENCE_THRESHOLD` | `0.5` | A0 real predictor confidence threshold |
| `OIL_PALM_GANODERMA_MODEL_PATH` | `models/oil_palm/ganoderma_risk/best.pth` | Ganoderma ResNet18 state_dict path |
| `OIL_PALM_GANODERMA_LABELS_FILE` | `models/oil_palm/ganoderma_risk/labels.json` | Ganoderma project labels |
| `OIL_PALM_GANODERMA_METRICS_FILE` | `models/oil_palm/ganoderma_risk/metrics.json` | Ganoderma runtime metadata |
| `OIL_PALM_GANODERMA_DEVICE` | `cpu` | Ganoderma runtime device |

Future per-task branches will enable these path variables:

- `OIL_PALM_FFB_MODEL_PATH`
- `OIL_PALM_UAV_CROWN_MODEL_PATH`

The first trained A0 baseline records its local ignored artifact path in
`models/oil_palm/a0_image_routing/inference_config.yaml`. Runtime code can load
it through `OIL_PALM_A0_MODEL_PATH`; the weight file itself remains outside Git.

The Ganoderma v1 handoff records class order, split counts, training
configuration, preprocessing, and known source-bias risks in
`models/oil_palm/ganoderma_risk/metrics.json`. Runtime code can load its
ignored `best.pth` artifact through `OIL_PALM_GANODERMA_MODEL_PATH`.

Mode semantics:

- `mock`: all registered oil palm tasks use mock predictors.
- `hybrid`: configured real A0/Ganoderma predictors run; missing tasks remain
  mock.
- `real`: fail-fast if the currently integrated A0 or Ganoderma real predictor
  cannot load.
