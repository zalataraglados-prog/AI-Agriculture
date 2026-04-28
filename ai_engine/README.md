# Service

该目录包含 AI 推理模块的完整四层架构实现。

## 目录结构

*   `api/`: **(L1) 接口层** — FastAPI 路由与 HTTP 状态码处理。仅做路由分发，严禁包含推理逻辑。
    *   `v1/predict.py`: `POST /api/v1/predict` 和 `GET /api/v1/health`。
*   `adapters/`: **(L2) 适配器层** — 处理外部输入（文件路径、字节流），转化为 core 层可直接使用的内部结构 (PIL.Image)。
*   `core/`: **(L3) 核心推理层** — 纯 Python 算法逻辑。所有模型继承自 `base_predictor.py` 中的 `BasePredictor`。严禁导入 FastAPI。
*   `schemas/`: **契约层** — Pydantic 数据结构定义。所有模块间的数据传递均以此为准。
*   `main.py`: FastAPI 应用入口。负责模型预加载、CORS 配置和全局异常处理。
*   `infer.py`: 本地单图推理 CLI 工具（不依赖 FastAPI）。

## 启动服务

```bash
# 开发模式（自动重载）
uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000
```

### 环境变量

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `MODEL_CHECKPOINT_PATH` | 模型权重文件路径 | `models/rice/rice_leaf_classifier/best_model.pth` |
| `MODEL_LABELS_FILE` | 标签文件路径 | `models/rice/rice_leaf_classifier/labels.json` |
| `MODEL_CONFIG_FILE` | 模型配置路径 | `models/rice/rice_leaf_classifier/config.yaml` |

## API 端点

### POST /api/v1/predict

上传一张 JPEG/PNG 图片，返回病害分类结果。

```bash
curl -X POST http://localhost:8000/api/v1/predict \
  -F "file=@test_image.jpg"
```

**成功响应 (200)**:
```json
{
  "status": "success",
  "results": [
    {
      "predicted_class": "Leaf_Blast",
      "confidence": 0.93,
      "topk": [
        {"label": "Leaf_Blast", "score": 0.93},
        {"label": "Brown_Spot", "score": 0.05},
        {"label": "HealthyLeaf", "score": 0.02}
      ],
      "model_version": "rice_cls_v0.1.0",
      "metadata": {},
      "geometry": null
    }
  ],
  "metadata": {}
}
```

**错误响应 (422/500)**:
```json
{
  "status": "error",
  "message": "Cannot decode image from provided bytes"
}
```

### GET /api/v1/health

健康检查，用于 Docker/K8s 探针。

```json
{
  "status": "ok",
  "service": "smart-farm-ai-engine",
  "version": "0.1.0",
  "model": {
    "model_name": "RiceLeafClassifier",
    "model_version": "rice_cls_v0.1.0",
    "architecture": "resnet18",
    "num_classes": "8"
  }
}
```

## 本地 CLI 推理

```bash
python ai_engine/infer.py \
  --image-path test.jpg \
  --checkpoint-path outputs/.../best_model.pth
```