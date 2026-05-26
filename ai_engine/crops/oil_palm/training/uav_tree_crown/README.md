# UAV Tree Crown Training

## Goal

Train a UAV tile oil palm crown detector. The model only returns per-tile crown
bboxes. Cloud remains responsible for tile offset, global coordinate
reconstruction, cross-tile NMS, human confirmation, and stable `tree_code`
creation.

UAV v1 does not predict health status, Ganoderma risk, growth score, FFB
maturity, yield, or a whole-tree production decision.

## Recommended Framework

YOLOv8 (Ultralytics), starting with `yolov8n.pt` for the first Colab baseline.

## Labels

See `models/oil_palm/uav_tree_crown/labels.json`.

- `oil_palm_crown`

Source categories are normalized to `oil_palm_crown`. Health/status labels are
not part of this UAV model.

## Data Source

Tracked metadata:

- `datasets/oil_palm/manifests/uav_tree_crown.json`
- `datasets/oil_palm/uav_tree_crown/dataset_card.md`
- `datasets/oil_palm/uav_tree_crown/splits/`
- `datasets/oil_palm/uav_tree_crown/licenses/sources.csv`

Ignored generated data:

- `datasets/oil_palm/uav_tree_crown/raw/`
- `datasets/oil_palm/uav_tree_crown/yolo/`
- `models/oil_palm/uav_tree_crown/runs/`

Expected source stats from Roboflow project `doyles-workspace/uva_crown`,
version 1:

- Images: 1785
- Annotations: 3400+ bboxes
- Roboflow split: train 1470 / val 158 / test 157

## Dataset Preparation

```bash
python -m ai_engine.crops.oil_palm.training.uav_tree_crown.prepare_dataset \
  --source-root <roboflow-coco-export-root> \
  --copy-raw \
  --overwrite
```

If `--source-root` is omitted, the script reads `OIL_PALM_UAV_SOURCE_ROOT` or
falls back to `datasets/oil_palm/uav_tree_crown/raw/`.

The importer expects Roboflow COCO folders such as:

- `train/_annotations.coco.json`
- `valid/_annotations.coco.json`
- `test/_annotations.coco.json`

It converts COCO bboxes to project YOLO format under
`datasets/oil_palm/uav_tree_crown/yolo/` and preserves the Roboflow split.

Use `--dry-run` to parse and summarize without writing generated data.

## Training Command

```bash
python -m ai_engine.crops.oil_palm.training.uav_tree_crown.train_yolo \
  --config models/oil_palm/uav_tree_crown/training_config.example.yaml
```

Use `--dry-run` to validate training arguments without installing Ultralytics or
starting training.

Recommended first baseline:

- Model: `yolov8n.pt`
- Image size: `640`
- Epochs: `100`
- Early stopping patience: `20`
- Batch: `16` on Colab GPU, reduce if memory is tight
- Augmentation: disabled for v1 because Roboflow-side augmentation has already
  been applied

## Evaluation

After training in Colab, evaluate on the generated test split and record metrics
in `models/oil_palm/uav_tree_crown/metrics.json` before runtime integration.
The `.pt` weights and run plots must stay under ignored `runs/` or external
artifact storage.

## Notes

- Current source has no mission IDs, but it does provide train/valid/test splits.
- Future datasets should include plantation/block/mission identifiers so splits
  can be audited by mission and avoid spatial leakage.
- Adjacent tiles may overlap; Cloud-side NMS and confirmation remain required.
- GSD, altitude, and orthomosaic stitching quality should be recorded with each
  future source dataset.
