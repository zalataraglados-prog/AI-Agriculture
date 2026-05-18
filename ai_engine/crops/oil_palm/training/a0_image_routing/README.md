# A0 Image Routing Training

## Goal

Train the A0 structure-detection routing model. A0 is the entry gatekeeper: it
detects supported oil palm structures, proposes bbox candidates, and validates
the user's selected `image_role`.

A0 does not predict FFB maturity, Ganoderma risk, growth status, yield, or any
tree-level conclusion.

## Recommended Framework

Use a lightweight YOLO-family detector. The first production candidate should be
a nano/small model that can return bbox candidates with low latency.

## Labels

See `models/oil_palm/a0_image_routing/labels.json`.

- `fruit_bunch`
- `trunk_base`
- `crown_region`

`unknown` is not a YOLO class. Unsupported or unclear images should be included
as empty-label negative samples. At inference time, the service should return a
route status such as `no_supported_structure_detected`, `role_mismatch`, or
`uncertain`.

## Data Sources

Training data can be bootstrapped from existing FFB, Ganoderma, and UAV crown
datasets by adding structure bboxes. Keep source, license, plantation/session,
and grouping metadata so train/val/test splits do not leak near-duplicate
evidence across sets.

## Training Command

```bash
python -m ai_engine.crops.oil_palm.training.a0_image_routing.prepare_dataset --source-root E:\a0 --copy-raw --overwrite
python -m ai_engine.crops.oil_palm.training.a0_image_routing.train_yolo --config models/oil_palm/a0_image_routing/training_config.example.yaml
```

Use `--dry-run` on either command to validate paths and arguments without
writing the dataset or starting training.

The preparation command writes YOLO files under
`datasets/oil_palm/a0_image_routing/yolo/`, which is ignored by Git. It also
writes tracked metadata under `splits/`, `licenses/`, and
`datasets/oil_palm/manifests/a0_image_routing.json`.

The first Roboflow bootstrap adapter maps source labels as follows:

- `ffb` -> `fruit_bunch`
- `Trunk_base` -> `trunk_base`
- `crown_region` -> `crown_region`

Roboflow root categories are ignored. Slight bbox rounding overflow is clipped
to image bounds before YOLO export.

## Current Baseline

The first A0 YOLOv8n baseline has been trained as
`oil_palm_a0_yolo_structure_detector_v1`.

- Dataset version: `roboflow_a0_2026_05_17`
- Images: 396
- Bboxes: 1174
- Best epoch: 76
- mAP50: 0.8988
- mAP50-95: 0.52789
- Precision: 0.78039
- Recall: 0.94318

Tracked metadata lives in:

- `models/oil_palm/a0_image_routing/metrics.json`
- `models/oil_palm/a0_image_routing/inference_config.yaml`
- `models/oil_palm/a0_image_routing/model_card.md`

Weights and training plots live under the ignored local run directory:
`models/oil_palm/a0_image_routing/runs/a0_yolo_structure_detector_v1/`.
They are not committed to Git.

## Design Principles

- A0 v1 does not replace the user's `image_role`; it validates the selected
  structure and proposes bbox candidates.
- Multiple candidates require user confirmation before downstream inference.
- Rejected candidates should be masked in a derived image before FFB/Ganoderma
  or growth analysis.
- When uncertain, return `route_status=uncertain` rather than a wrong route.
- A0 remains a routing gatekeeper, not a production diagnosis or maturity model.
