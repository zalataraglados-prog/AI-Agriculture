# AI Module Notes

This folder documents the AI module, including:

- API notes
- model/training notes
- data notes
- deployment notes

## Current Stage

- Task: image classification (not object detection)
- Dataset: Rice Leaf Spot Disease Annotated Dataset (YOLO format source, converted for classification)
- Baseline model: ResNet18
- Classes: 8
- Input size: 224

## Workflow

### 1) Prepare dataset (detection -> classification)

```bash
python scripts/prepare_rice_cls_dataset.py --dataset-root "local_data/rice_leaf_spot_disease_annotated_dataset"
```

### 2) Train

```bash
python scripts/train_rice_leaf_classifier.py
```

### 3) Run inference

```bash
python -m ai_engine.infer \
  --image-path "your_image.jpg" \
  --checkpoint-path "outputs/rice_leaf_classifier/checkpoints/best_model.pth"
```

## Notes

- Dataset files are not included in this repository.
- Place local datasets under `local_data/`.
- Current baseline ignores bounding boxes for classification stage.

## Future

- Object detection (YOLO / Faster R-CNN)
- Multi-label classification
- Multi-crop support
