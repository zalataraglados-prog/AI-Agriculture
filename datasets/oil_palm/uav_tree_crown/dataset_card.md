# UAV Tree Crown Dataset

## Status

Training preparation ready. The real image/annotation files are expected to
come from Roboflow project `doyles-workspace/uva_crown`, version 1, downloaded
as COCO. Raw exports and generated YOLO files remain outside Git under the
ignored `raw/` and `yolo/` layers.

## Labels

0. `oil_palm_crown`

UAV v1 is a single-class crown detector, not a health-status classifier. Source
categories are normalized to `oil_palm_crown`.

## Expected Counts

- Images: 1785
- Annotations: 3400+ bboxes
- Roboflow split: train 1470 / val 158 / test 157
- Label counts: `{"oil_palm_crown": "3400+"}`

## Sources And License

- Source: Roboflow project `doyles-workspace/uva_crown`, version 1
- URL without private key: `https://app.roboflow.com/doyles-workspace/uva_crown`
- License: `Unknown` until source permissions are verified

Do not commit the Roboflow API key, raw images, generated YOLO files, or model
weights. The Colab notebook on the Desktop may contain the private export
command because it is outside this Git repository.

## Notes

- Split method: preserve Roboflow train/valid/test.
- Current source limitation: no mission IDs in the export metadata.
- Roboflow-side augmentation has already been applied; project training config
  disables additional augmentation for the first baseline.
- Future dataset versions should include plantation/block/mission identifiers
  so highly similar UAV tiles do not leak across train/val/test.
- The model detects per-tile crowns only. Cloud handles tile offsets, global
  coordinate reconstruction, NMS, human confirmation, and tree_code creation.
