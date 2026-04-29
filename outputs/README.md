# Outputs Directory

This directory stores generated artifacts from dataset preparation, training, and inference-related workflows.

The directory structure is intentionally kept in version control, but the generated files inside it remain ignored.

## Expected Structure

```text
outputs/
└── rice_leaf_classifier/
   ├── data/
   │  ├── all_samples.json
   │  ├── train_samples.json
   │  ├── val_samples.json
   │  └── prepare_stats.json
   └── checkpoints/
      ├── best_model.pth
      ├── final_model.pth
      └── training_history.json
```

## File Meanings

### `data/all_samples.json`

JSON array. Each item represents one usable image converted into a classification sample.

Example shape:

```json
[
  {
    "image_path": "local_data/.../train/images/example.jpg",
    "label": 2,
    "label_name": "HealthyLeaf"
  }
]
```

Meaning:
- `image_path`: source image path
- `label`: numeric class index
- `label_name`: human-readable class name

### `data/train_samples.json`

JSON array with the same item format as `all_samples.json`.

Meaning:
- subset of samples used for model training

### `data/val_samples.json`

JSON array with the same item format as `all_samples.json`.

Meaning:
- subset of samples used for validation during training

### `data/prepare_stats.json`

JSON object containing dataset preparation statistics.

Example shape:

```json
{
  "total_images_found": 1000,
  "missing_label_files": 12,
  "empty_label_as_healthy": 180,
  "multi_class_images": 25,
  "invalid_label_lines": 3,
  "kept_samples": 988
}
```

Meaning:
- `total_images_found`: number of image files discovered
- `missing_label_files`: images skipped because no matching label file was found
- `empty_label_as_healthy`: empty annotation files interpreted as `HealthyLeaf`
- `multi_class_images`: images containing multiple classes before majority-label conversion
- `invalid_label_lines`: malformed annotation lines ignored during parsing
- `kept_samples`: final number of usable classification samples

### `checkpoints/best_model.pth`

PyTorch checkpoint file saved when validation F1 reaches the best value so far.

Typical contents:
- `epoch`
- `model_state_dict`
- `optimizer_state_dict`
- `class_names`
- `best_val_f1`
- `config`

Meaning:
- recommended checkpoint for later inference

### `checkpoints/final_model.pth`

PyTorch weight file saved at the end of training.

Typical contents:
- model state dict only

Meaning:
- final training result, not necessarily the best-performing checkpoint

### `checkpoints/training_history.json`

JSON object containing metric history across epochs.

Example shape:

```json
{
  "train_loss": [1.23, 0.98],
  "val_loss": [1.10, 0.95],
  "train_acc": [0.55, 0.68],
  "val_acc": [0.58, 0.70],
  "train_f1": [0.50, 0.66],
  "val_f1": [0.53, 0.69],
  "lr": [0.0003, 0.0003]
}
```

Meaning:
- records training and validation metrics for each epoch
- useful for plotting curves or comparing runs

## Notes

- Generated files under `outputs/` are ignored by Git.
- `README.md` files are kept so the directory purpose and file formats stay documented.
- If more tasks are added later, this document should be extended with any new output subdirectories or file formats.
