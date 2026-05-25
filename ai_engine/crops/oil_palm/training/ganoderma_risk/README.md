# Ganoderma Risk Training

## Goal

Train a trunk-base RGB classifier for Ganoderma / BSR risk screening. Outputs
must remain suspected risk signals, never confirmed disease labels.

## Labels

Project-standard labels:

```text
healthy
suspected_risk
other_stress_unknown
```

v1 active training labels and class order:

```text
0 = healthy
1 = suspected_risk
```

`other_stress_unknown` is reserved for future data and is ignored by v1
training.

## Import Commands

Run the infected/risk source first if you want a clean output directory, then
append the healthy source.

```bash
python ai_engine/crops/oil_palm/training/data_importers/import_ganoderma_infected_roboflow.py \
  --raw-dir datasets/oil_palm/ganoderma_risk/raw/ganoderma_infected \
  --output-dir datasets/oil_palm/ganoderma_risk/classification \
  --summary-file datasets/oil_palm/ganoderma_risk/splits/ganoderma_infected_summary.json \
  --overwrite

python ai_engine/crops/oil_palm/training/data_importers/import_ganoderma_healthy_roboflow.py \
  --raw-dir datasets/oil_palm/ganoderma_risk/raw/ganoderma_healthy \
  --output-dir datasets/oil_palm/ganoderma_risk/classification \
  --summary-file datasets/oil_palm/ganoderma_risk/splits/ganoderma_healthy_summary.json
```

## Training Command

Actual v1 Colab command recorded from handoff:

```bash
python ai_engine/crops/oil_palm/training/ganoderma_risk/train.py \
  --data-dir datasets/oil_palm/ganoderma_risk/classification \
  --model-out models/oil_palm/ganoderma_risk \
  --labels-file models/oil_palm/ganoderma_risk/labels.json \
  --active-labels healthy,suspected_risk \
  --model-version oil_palm_ganoderma_resnet18_v1.0.0_offline \
  --dataset-version v1 \
  --dataset-manifest datasets/oil_palm/manifests/ganoderma_risk.json \
  --epochs 20 \
  --phase1-epochs 10 \
  --batch-size 32 \
  --learning-rate 1e-4 \
  --finetune-learning-rate 1e-5 \
  --image-size 224 \
  --seed 42 \
  --num-workers 2
```

Colab notes:

```python
from google.colab import drive
drive.mount("/content/drive")

import os
os.chdir("/content/drive/MyDrive/<project-folder>")

!pip install scikit-learn
!python ai_engine/crops/oil_palm/training/ganoderma_risk/train.py
```

## Training Configuration

```text
base model:          ResNet18
pretrained weights:  ResNet18_Weights.IMAGENET1K_V1
input size:          224 x 224
batch size:          32
epochs:              20 total
phase 1:             FC layer only, 10 epochs, lr=1e-4
phase 2:             full fine-tune, 10 epochs, lr=1e-5
optimizer:           Adam
scheduler:           StepLR
loss:                CrossEntropyLoss
class weights:       healthy=0.9263, suspected_risk=1.0864
seed:                42
augmentation:        flips, rotation 15, brightness/contrast jitter 0.3
```

## Split Summary

| Split | healthy | suspected_risk | Total |
|-------|---------|----------------|-------|
| train | 414 | 337 | 751 |
| val | 88 | 70 | 158 |
| test | 92 | 75 | 167 |
| total | 594 | 482 | 1076 |

Split method: each source importer applies stratified random split by class
with seed 42, then appends outputs into one `classification/` directory.

Known risks: source/domain bias exists, cross-dataset duplicate detection was
not performed, and same-tree or same-capture-batch leakage cannot be ruled out.

## Output Contract

- Weights: `models/oil_palm/ganoderma_risk/best.pth` (ignored by Git).
- Dataset manifest: `datasets/oil_palm/manifests/ganoderma_risk.json`.
- Template metrics: `models/oil_palm/ganoderma_risk/metrics.example.json`.
- Recorded handoff metrics: `models/oil_palm/ganoderma_risk/metrics.json`.
- Model card: `models/oil_palm/ganoderma_risk/model_card.md`.

## Runtime Semantics

```text
healthy        -> Healthy / no visible risk indicators
suspected_risk -> Ganoderma/BSR suspected risk; expert confirmation required
```

The model does not output confirmed disease. If runtime later uses a
`confirmed` status, that status is user-review workflow state, not agronomic
diagnosis.
