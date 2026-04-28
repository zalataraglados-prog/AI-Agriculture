# AI 妯″潡璇存槑

鏈洰褰曠敤浜庣淮鎶?AI 妯″潡鐩稿叧鏂囨。锛屽寘鎷細
- API 璇存槑
- 妯″瀷鏂规
- 鏁版嵁璇存槑
- 閮ㄧ讲璇存槑

## Current Stage
- Task: Image Classification (NOT object detection)
- Dataset: Rice Leaf Spot Disease Annotated Dataset (YOLO format)
- Model: ResNet18
- Classes: 8
- Input Size: 224
## Workflow
### 1. Prepare Dataset (Detection 鈫?Classification)
```bash
python scripts/prepare_rice_cls_dataset.py --dataset-root "local_data/rice_leaf_spot_disease_annotated_dataset"
```

### 2. Train Model

  python scripts/train_rice_leaf_classifier.py

### 3. Run Inference
```bash
python service/infer.py \
    --image-path "your_image.jpg" \
    --checkpoint-path "outputs/rice_leaf_classifier/checkpoints/best_model.pth"
```

Important Notes
---------------

* The dataset is NOT included in this repository.

* Place dataset under:
```
  local_data/
```
* This project currently uses classification baseline.

* Bounding boxes are ignored in this stage.

Future Work
-----------

* Object detection (YOLO / Faster R-CNN)

* Multi-label classification

* Multi-crop support