# Ganoderma Risk Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm Ganoderma Risk Classifier |
| Task | Image classification (risk screening) |
| Crop | Oil Palm |
| Framework | ResNet18 |
| Current Status | **offline trained v1; AI Engine runtime supported** |
| Model Version | `oil_palm_ganoderma_resnet18_v1.0.0_offline` |

## Intended Use

Screen trunk-base oil palm images for visible Ganoderma / basal stem rot risk
signals. The model output is a risk-screening signal for tree/session evidence,
not an agronomic diagnosis.

## Labels And Output Semantics

Project-standard labels are `healthy`, `suspected_risk`, and
`other_stress_unknown`.

The v1 model trains two active classes:

```text
0 = healthy
1 = suspected_risk
```

Runtime interpretation:

```text
healthy        -> Healthy / no visible risk indicators
suspected_risk -> Ganoderma/BSR suspected risk; expert confirmation required
```

`other_stress_unknown` is reserved for future data and is not produced by this
offline v1 classifier.

The model does not output confirmed disease. If a runtime workflow later uses a
`confirmed` status, that status means user-review workflow state, not agronomic
diagnosis.

## Training Data

The v1 handoff uses two Roboflow-derived sources: one Ganoderma-risk source and
one healthy oil palm source. Source URLs are not tracked in Git in this pass.

| Split | healthy | suspected_risk | Total |
|-------|---------|----------------|-------|
| train | 414 | 337 | 751 |
| val | 88 | 70 | 158 |
| test | 92 | 75 | 167 |
| total | 594 | 482 | 1076 |

Split method: each source importer applies stratified random split by class
with seed 42 and appends into the same `classification/` directory.

Known data risks:

- healthy and suspected-risk samples come from different Roboflow projects;
- suspected-risk samples include trunk-base/root and trunk views, while healthy
  samples are trunk views;
- cross-dataset duplicate detection was not performed;
- same-tree and same-capture-batch leakage cannot be ruled out;
- domain bias may inflate test performance.

## Training Configuration

| Parameter | Value |
|-----------|-------|
| Base model | ResNet18 |
| Pretraining | ImageNet `ResNet18_Weights.IMAGENET1K_V1` |
| Input size | 224 x 224 |
| Batch size | 32 |
| Epochs | 20 |
| Phase 1 | FC layer only, 10 epochs, lr=1e-4 |
| Phase 2 | Full fine-tune, 10 epochs, lr=1e-5 |
| Optimizer | Adam |
| Scheduler | StepLR; phase1 step_size=7 gamma=0.1, phase2 step_size=5 gamma=0.1 |
| Loss | CrossEntropyLoss |
| Class weights | healthy=0.9263, suspected_risk=1.0864 |
| Augmentation | Random flips, RandomRotation(15), ColorJitter brightness/contrast 0.3 |
| Seed | 42 |

Preprocessing:

```text
Resize to 224 x 224
ToTensor
Normalize(mean=[0.485, 0.456, 0.406], std=[0.229, 0.224, 0.225])
```

## Evaluation

Results source: test set.

| Class | Precision | Recall | F1 | Support |
|-------|-----------|--------|----|---------|
| healthy | 0.94 | 1.00 | 0.97 | 92 |
| suspected_risk | 1.00 | 0.92 | 0.96 | 75 |
| accuracy | | | 0.96 | 167 |
| macro avg | 0.97 | 0.96 | 0.96 | 167 |
| weighted avg | 0.97 | 0.96 | 0.96 | 167 |

Confusion matrix:

```text
                     Predicted healthy  Predicted suspected_risk
Actual healthy                      92                         0
Actual suspected_risk                6                        69
```

Best validation accuracy during training: `0.9684`. Per-class validation
metrics were not recorded separately.

The six suspected-risk false negatives have not been manually reviewed. v2
should output misclassified sample paths for follow-up inspection.

## Artifact

```text
Format: PyTorch state_dict (.pth)
Recommended filename: best.pth
Path: models/oil_palm/ganoderma_risk/best.pth
Architecture: ResNet18 with final fc layer 512 -> 2
Class order: 0=healthy, 1=suspected_risk
```

Weights remain outside Git.

## Runtime Status

AI Engine can load this classifier when `CROP_PROFILE=oil_palm` and
`OIL_PALM_MODEL_MODE=hybrid` or `real` are set. Mount the ignored weight file
and point `OIL_PALM_GANODERMA_MODEL_PATH` at it:

```bash
OIL_PALM_GANODERMA_MODEL_PATH=/opt/ai-agriculture/models/oil_palm/ganoderma_risk/best.pth
OIL_PALM_GANODERMA_LABELS_FILE=models/oil_palm/ganoderma_risk/labels.json
OIL_PALM_GANODERMA_METRICS_FILE=models/oil_palm/ganoderma_risk/metrics.json
OIL_PALM_GANODERMA_DEVICE=cpu
```

Cloud enables the tree-profile path by setting
`AI_OIL_PALM_ANALYZE_URL=http://ai-engine:8000/api/v1/oil-palm/analyze`.
Confirmed `trunk_base` session images then call this runtime after the A0
confirmation mask is created.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| `oil_palm_ganoderma_resnet18_v1.0.0_offline` | 2026-05 | AI Engine runtime loader added; weights still mounted outside Git. |
| `oil_palm_ganoderma_mock_v1` | 2026-05 | Mock predictor established. No real weights. |
