# UAV Tree Crown Detection Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm UAV Crown Detector |
| Task | Object detection (single-class) |
| Crop | Oil Palm |
| Framework | YOLOv8 |
| Current Status | **trained baseline available; runtime remains mock** |
| Model Version | `oil_palm_uav_yolo_tree_crown_detector_v1` |

## Intended Use

Detect oil palm tree crowns in UAV orthomosaic tiles. This model only handles
per-tile detection; the Cloud pipeline is responsible for tile offset, global
coordinate reconstruction, NMS, human confirmation, and stable `tree_code`
creation.

This model is not a health, disease, maturity, growth, or yield predictor.

## Labels

See [labels.json](./labels.json): `oil_palm_crown`.

## Training Data

The first baseline was trained from Roboflow project
`doyles-workspace/uva_crown`, version 1, downloaded as COCO and converted to
project YOLO layout.

- Dataset version: `roboflow_uva_crown_v1_2026_05_26`
- Images: 1785
- Source bboxes: 4111
- Effective YOLO bboxes: 4110
- Roboflow split: train 1470 / val 158 / test 157
- Labels: `oil_palm_crown`
- Roboflow-side augmentation has already been applied, so the first project
  training config disables extra augmentation.
- One invalid bbox was skipped during conversion.
- Source metadata reports `CC BY 4.0`.

See:

- `datasets/oil_palm/manifests/uav_tree_crown.json`
- `datasets/oil_palm/uav_tree_crown/dataset_card.md`
- `models/oil_palm/uav_tree_crown/training_config.example.yaml`

## Evaluation

See [metrics.json](./metrics.json) for the recorded Colab handoff metrics.

| Metric | Value |
|--------|-------|
| Test mAP50 | 0.9717 |
| Test mAP50-95 | 0.7834 |
| Test precision | 0.9582 |
| Test recall | 0.9378 |
| Validation mAP50 | 0.9766 |

## Ethical Considerations

- Crown detection is a spatial asset registration step, not a production decision.
- False positives should be caught by human confirmation in the UAV pipeline.

## Limitations

- v1 is single-class: only oil palm crowns, no distinction of dead palms, other
  tree species, or health status.
- Performance depends on UAV altitude, GSD, and orthomosaic stitching quality.
- Adjacent tile overlap may cause duplicate detections; Cloud NMS handles this.
- The current Roboflow export has no mission IDs; the source split can be
  preserved, but mission-level leakage auditing is still not possible.
- Runtime predictor integration is not included in this handoff. AI Engine
  should continue using mock UAV fallback until a real predictor is wired and
  tested.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| mock_v1 | 2026-05 | Mock predictor established. No real weights. |
| training_prep | 2026-05-26 | Added Roboflow COCO importer, dataset prep CLI, training config, and Colab handoff plan. Runtime still uses mock fallback. |
| dataset_v1_fix | 2026-05-26 | Updated preparation for `uva_crown` v1 with real bbox annotations, preserved Roboflow train/valid/test split, and disabled extra training augmentation. |
| oil_palm_uav_yolo_tree_crown_detector_v1 | 2026-05-26 | First Colab-trained YOLOv8n baseline. Metadata recorded in `metrics.json`; weights stored under ignored local `runs/` directory. |
