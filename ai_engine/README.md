# AI Engine

智慧农业 AI 推理引擎，采用模块化多作物架构。

## 目录结构

```
ai_engine/
├── main.py                          # FastAPI 入口（lifespan 预加载、CORS、路由挂载）
├── infer.py                         # 本地单图推理 CLI 工具
├── common/                          # 跨作物共享模块
│   ├── health.py                    # GET /api/v1/health（profile-safe）
│   ├── base_predictor.py            # 模型基类 BasePredictor
│   ├── adapters/
│   │   └── image_adapter.py         # L2: 图像解码适配器
│   └── schemas/
│       └── prediction.py            # Pydantic 数据契约
└── crops/                           # 作物隔离模块（禁止跨 crop 导入）
    ├── rice/
    │   ├── inference/
    │   │   ├── api.py               # POST /api/v1/predict, /api/v1/rice/predict
    │   │   └── rice_leaf_classifier.py  # L3: 水稻病害分类器
    │   └── training/
    │       ├── train_rice_leaf_classifier.py
    │       ├── evaluate.py
    │       └── prepare_rice_cls_dataset.py
    └── oil_palm/
        ├── inference/
        │   ├── api.py               # POST /api/v1/oil-palm/analyze-image 等 Mock 端点
        │   └── predictor.py         # YOLOv8 占位
        └── training/                # 待实现
```

## 架构规则

- `common/` 只放跨 crop 共享逻辑（适配器、Schema、健康检查）。
- `crops/<crop>/` 内的模块**禁止互相导入**。
- `main.py` 根据 `CROP_PROFILE` 环境变量决定加载哪个 crop 的模型。

## 启动服务

```bash
# 水稻模式（默认）
CROP_PROFILE=rice uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000

# 油棕模式
CROP_PROFILE=oil_palm uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000
```

## 环境变量

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `CROP_PROFILE` | 作物模式 (`rice` / `oil_palm`) | `rice` |
| `MODEL_CHECKPOINT_PATH` | 模型权重路径 | `models/rice/rice_leaf_classifier/best_model.pth` |
| `MODEL_LABELS_FILE` | 标签映射路径 | `models/rice/rice_leaf_classifier/labels.json` |
| `MODEL_CONFIG_FILE` | 模型配置路径 | `models/rice/rice_leaf_classifier/config.yaml` |
| `MODEL_ADVICE_FILE` | 病害建议路径 | `models/rice/rice_leaf_classifier/advice_map.yaml` |
| `CORS_ORIGINS` | 允许跨域源 | `http://localhost:8088,http://127.0.0.1:8088` |

## API 端点

### 通用

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/health` | Profile-safe 健康检查 |

### 水稻 (Rice)

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/predict` | 兼容旧端点（hidden in schema） |
| POST | `/api/v1/rice/predict` | 水稻病害分类 |
| GET | `/api/v1/rice/health` | 水稻模型健康检查 |

### 油棕 (Oil Palm) — Mock

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/oil-palm/analyze-image` | 单张分析 |
| POST | `/api/v1/oil-palm/analyze-session` | 会话分析 |
| POST | `/api/v1/oil-palm/analyze-uav-mission` | 无人机任务分析 |

## 本地 CLI 推理

```bash
python -m ai_engine.infer --help
python -m ai_engine.infer --image-path test.jpg
```