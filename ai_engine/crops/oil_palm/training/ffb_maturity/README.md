# FFB Maturity Training

## Goal

Train a YOLO detector for visible oil palm fresh fruit bunches (FFB) and their
maturity stage. The model output must remain one piece of tree/session evidence,
not a full single-image yield decision.

## Labels

Project-standard labels:

```text
flower
unripe
underripe
ripe
overripe
abnormal
```

See `models/oil_palm/ffb_maturity/labels.json`.

## Roboflow Import

```bash
python ai_engine/crops/oil_palm/training/data_importers/import_ffb_roboflow.py \
  --raw-dir datasets/oil_palm/ffb_maturity/raw/roboflow_v1 \
  --output-dir datasets/oil_palm/ffb_maturity/yolo \
  --summary-file datasets/oil_palm/ffb_maturity/splits/roboflow_v1_summary.json \
  --overwrite
```

The importer refuses to overwrite existing output unless `--overwrite` is set.
It records unknown labels, malformed rows, invalid bboxes, and copied image
counts in the summary JSON.

## Training Command Template

The teammate must confirm the exact command used for the recorded v1 metrics.
Until then, this template is the expected shape:

```bash
yolo detect train \
  model=yolov8n.pt \
  data=datasets/oil_palm/ffb_maturity/yolo/data.yaml \
  imgsz=640 \
  epochs=50 \
  seed=42 \
  project=models/oil_palm/ffb_maturity/runs \
  name=ffb_maturity_yolov8n_v1
```

## Evaluation Command Template

```bash
yolo detect val \
  model=models/oil_palm/ffb_maturity/best.pt \
  data=datasets/oil_palm/ffb_maturity/yolo/data.yaml \
  imgsz=640
```

## Output Contract

- Weights: `models/oil_palm/ffb_maturity/best.pt` (ignored by Git).
- Template metrics: `models/oil_palm/ffb_maturity/metrics.example.json`.
- Recorded handoff metrics: `models/oil_palm/ffb_maturity/metrics.json`.
- Model card: `models/oil_palm/ffb_maturity/model_card.md`.

## Pending Team Confirmation

- Exact Roboflow export version and license.
- Source class id mapping used during training.
- Train/validation/test split counts and split method.
- Whether `flower` and `abnormal` were absent, filtered, or not trained.
- Weight file path, size, and SHA-256 hash.
