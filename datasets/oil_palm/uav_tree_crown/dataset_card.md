# UAV Tree Crown Dataset

## Status

Training preparation ready. The real image/label files are expected to come
from the Roboflow export and remain outside Git under the ignored `raw/` and
`yolo/` layers.

## Labels

0. `oil_palm_crown`

The dataset metadata mentioned `Healthy-BSR-Non-BSR`, but the reviewed export
contains 100% `oil_palm_crown` instances. UAV v1 is therefore a single-class
crown detector, not a health-status classifier.

## Expected Counts

- Images: 1050
- Annotations: 2411
- Split target: train 735 / val 158 / test 157
- Label counts: `{"oil_palm_crown": 2411}`

## Sources And License

- Source: Roboflow UAV oil palm crown export
- URL without private key: `https://app.roboflow.com/ds/zKMDyUsHc9`
- License: `Unknown` until source permissions are verified

Do not commit the Roboflow key, raw images, generated YOLO files, or model
weights. The Colab notebook on the Desktop may contain the private export
command because it is outside this Git repository.

## Notes

- Split method: deterministic 70/15/15 split, seed `42`.
- Current source limitation: no validation/test split and no mission IDs.
- Future dataset versions should include plantation/block/mission identifiers
  so highly similar UAV tiles do not leak across train/val/test.
- The model detects per-tile crowns only. Cloud handles tile offsets, global
  coordinate reconstruction, NMS, human confirmation, and tree_code creation.
