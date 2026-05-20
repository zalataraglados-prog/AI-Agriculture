# Ganoderma Risk Training

## Goal

Train a trunk-base RGB classifier for Ganoderma / BSR risk screening. Outputs
must remain `suspected` risk signals, never confirmed disease labels.

## Labels

Project-standard labels:

```text
healthy
suspected_risk
other_stress_unknown
```

The v1 training script uses active labels only:

```text
healthy
suspected_risk
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

## Training Command Template

```bash
python ai_engine/crops/oil_palm/training/ganoderma_risk/train.py \
  --data-dir datasets/oil_palm/ganoderma_risk/classification \
  --model-out models/oil_palm/ganoderma_risk \
  --labels-file models/oil_palm/ganoderma_risk/labels.json \
  --active-labels healthy,suspected_risk \
  --epochs 20 \
  --phase1-epochs 10 \
  --batch-size 32 \
  --image-size 224 \
  --seed 42
```

## Output Contract

- Weights: `models/oil_palm/ganoderma_risk/best.pth` (ignored by Git).
- Template metrics: `models/oil_palm/ganoderma_risk/metrics.example.json`.
- Recorded handoff metrics: `models/oil_palm/ganoderma_risk/metrics.json`.
- Model card: `models/oil_palm/ganoderma_risk/model_card.md`.

## Pending Team Confirmation

- Dataset source pages, licenses, versions, and download dates.
- Whether split leakage exists across tree, plantation, video, or source.
- Exact train/validation/test counts; current handoff metrics report 167 test
  samples.
- Weight file path, size, and SHA-256 hash.
- Whether v1 is officially a two-active-class model with
  `other_stress_unknown` reserved for future data.
