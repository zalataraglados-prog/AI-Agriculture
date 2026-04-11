# AI API 说明

## 本地推理返回结构（第一版）

{
  "predicted_class": "Leaf_Blast",
  "confidence": 0.93,
  "topk": [
    {"label": "Leaf_Blast", "score": 0.93},
    {"label": "Brown_Spot", "score": 0.05},
    {"label": "HealthyLeaf", "score": 0.02}
  ],
  "model_version": "rice_cls_v0.1.0"
}

说明：
- 当前这是本地推理的统一输出结构
- 后续接入FastAPI时，保持字段名不变
