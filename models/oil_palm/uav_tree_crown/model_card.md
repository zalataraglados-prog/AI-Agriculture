# UAV Tree Crown Detection Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm UAV Crown Detector |
| Task | Object detection (single-class) |
| Crop | Oil Palm |
| Framework | YOLOv8 (planned) |
| Current Status | **mock** |
| Model Version | `oil_palm_uav_tile_mock_v1` |

## Intended Use

Detect oil palm tree crowns in UAV orthomosaic tiles. This model only handles per-tile detection;
the Cloud pipeline is responsible for tile offset, global coordinate reconstruction, NMS, and tree confirmation.

## Labels

See [labels.json](./labels.json): `oil_palm_crown`.

## Training Data

Not yet trained. See `datasets/oil_palm/manifests/uav_tree_crown.example.json` for planned data sources.

## Evaluation

Not yet evaluated. See [metrics.example.json](./metrics.example.json) for the metrics template.

## Ethical Considerations

- Crown detection is a spatial asset registration step, not a production decision.
- False positives should be caught by human confirmation in the UAV pipeline.

## Limitations

- v1 is single-class: only oil palm crowns, no distinction of dead/other tree species.
- Performance depends on UAV altitude, GSD, and orthomosaic stitching quality.
- Adjacent tile overlap may cause duplicate detections; Cloud NMS handles this.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| mock_v1 | 2026-05 | Mock predictor established. No real weights. |
