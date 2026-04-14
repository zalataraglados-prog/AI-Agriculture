# Local Data Directory

This directory stores local datasets used for development, preprocessing, training, and evaluation.

The directory structure is kept in version control, but the actual dataset files remain ignored.

## Dataset Source

Primary dataset source:
- Kaggle: `hadiurrahmannabil/rice-leaf-spot-disease-annotated-dataset`
- Dataset page: <https://www.kaggle.com/datasets/hadiurrahmannabil/rice-leaf-spot-disease-annotated-dataset/data>

Relevant notes from the dataset-provided documentation:
- total images: `3567`
- annotation format: `YOLO v8 PyTorch`
- image preprocessing includes auto-orientation and resize to `640x640`
- the dataset package includes augmented images

License:
- `CC BY-NC-SA 4.0`

## Recommended Acquisition Method

There are several ways to obtain this dataset. For this repository, the recommended default is:

- Download from the Kaggle web page, then extract it under `local_data/`

Why this is the recommended option:
- simplest setup for most users
- no need to configure Kaggle API credentials first
- easy to verify the downloaded folder structure matches this repository
- works well with the current scripts, which expect a local dataset path

Other valid options:
- Kaggle CLI or Kaggle API
- `kagglehub` inside notebooks or scripted environments

Recommendation by scenario:
- if you are using this project manually on your own machine, prefer the Kaggle web download
- if you want reproducible automation in CI, notebooks, or batch jobs, prefer the Kaggle API
- if you are already working in Kaggle notebooks, `kagglehub` is convenient

Typical manual setup:

```text
1. Open the Kaggle dataset page.
2. Download the archive.
3. Extract the dataset so that the project contains:
   local_data/rice_leaf_spot_disease_annotated_dataset/
4. Verify that train/images and train/labels exist.
```

## Expected Structure

```text
local_data/
└─ rice_leaf_spot_disease_annotated_dataset/
   ├─ README.txt
   ├─ data.yaml
   ├─ train/
   │  ├─ images/
   │  └─ labels/
   ├─ valid/
   │  ├─ images/
   │  └─ labels/
   └─ test/
      ├─ images/
      └─ labels/
```

## Directory Meanings

### `rice_leaf_spot_disease_annotated_dataset/`

Dataset root directory.

Meaning:
- contains the rice leaf disease dataset in detection-style layout
- may come from Kaggle or another local export

### `README.txt`

Dataset-provided documentation file.

Meaning:
- usually describes dataset origin, usage notes, or labeling details

### `data.yaml`

YAML metadata file commonly used by detection pipelines.

Typical contents may include:
- dataset split paths
- class count
- class names

Meaning:
- useful when the dataset is also used in object detection workflows

### `train/images/`

Training images.

Meaning:
- raw image files used for model training

### `train/labels/`

Training labels.

Meaning:
- annotation files corresponding to `train/images/`
- in this dataset, labels are detection-style text files

### `valid/images/`

Validation images.

Meaning:
- raw image files used for validation

### `valid/labels/`

Validation labels.

Meaning:
- annotation files corresponding to `valid/images/`

### `test/images/`

Test images.

Meaning:
- raw image files used for testing or final evaluation

### `test/labels/`

Test labels.

Meaning:
- annotation files corresponding to `test/images/`

## Annotation Format

The current project expects detection-style label files in `.txt` format.

Typical line format:

```text
<class_id> <x_center> <y_center> <width> <height>
```

Meaning:
- `class_id`: zero-based class index
- `x_center`, `y_center`, `width`, `height`: normalized bounding box values

For the classification preparation script:
- an empty label file is treated as `HealthyLeaf`
- if one image contains multiple labels, the majority class is used

## Notes

- Files under `local_data/` are ignored by Git.
- `README.md` files are kept so collaborators can understand the expected dataset layout.
- Large datasets, images, labels, and downloaded assets should stay inside `local_data/` rather than the tracked source tree.
