# A0 Image Routing Dataset

## Status

Local YOLO training dataset generated from Roboflow COCO exports.

## Labels

0. `fruit_bunch`
1. `trunk_base`
2. `crown_region`

`unknown` is not a YOLO class. This baseline does not include empty-label
negative samples yet.

## Counts

- Images: 396
- Annotations: 1174
- Split counts: {"test": 59, "train": 279, "val": 58}
- Label counts: {"crown_region": 352, "fruit_bunch": 708, "trunk_base": 114}
- Duplicate images skipped: 1
- Bboxes clipped to image bounds: 95

## Sources And License

Source exports are stored locally under `<external>/a0` and copied into
the ignored `raw/` layer when `--copy-raw` is used. License is currently
recorded as `Unknown`; verify source permissions before publishing trained
weights.

## Notes

- Generated version: `roboflow_a0_2026_05_17`
- Split method: grouped by source dataset and original media name.
- A0 detects structure candidates only; it does not infer maturity, disease, or
  tree-level conclusions.
