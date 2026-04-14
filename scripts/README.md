# Scripts

This directory contains data preparation and training scripts.

## Files

- prepare_rice_cls_dataset.py  
  Convert YOLO detection dataset to classification samples

- train_rice_leaf_classifier.py  
  Train classification model

## Usage

### Prepare Dataset

```python
python scripts/prepare_rice_cls_dataset.py --dataset-root <path>
```

### Train Model

python scripts/train_rice_leaf_classifier.py

Notes
-----

* Dataset must follow YOLO structure:  
train/images + train/labels

* Output is stored in:  
outputs/rice_leaf_classifier/
