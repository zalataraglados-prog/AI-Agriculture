# tests

## Oil Palm A0 Checks

- `test_oil_palm_foundation.py` validates oil palm manifests, labels, metrics
  templates, and model-mode safety.
- `test_oil_palm_routing.py` validates A0 routing and Ganoderma runtime
  registration with fake model modules.
- `test_oil_palm_a0_dataset.py` validates the A0 Roboflow COCO adapter,
  grouped split output, YOLO label ranges, duplicate skipping, and training
  dry-run argument construction.
- `test_oil_palm_uav_dataset.py` validates the UAV Roboflow COCO adapter,
  source split preservation, single-class crown remapping, bbox clipping, and
  training dry-run argument construction.
- `cd cloud && cargo test` also checks UAV tile coordinate restoration,
  center-distance NMS behavior, and empty `uav_tile` AI responses.

用于存放 AI 模块测试代码。

## 测试范围

- **接口测试** (`test_api.py`)：通过 Mock Classifier 验证 `/api/v1/predict`、`/api/v1/rice/predict`、`/api/v1/health` 端点行为。
- **推理冒烟测试** (`test_infer_smoke.py`)：验证 `RiceLeafClassifier` 加载和推理流程。
- **Schema 测试** (`test_schemas.py`)：验证 Pydantic 数据模型的序列化/反序列化。
- **适配器测试** (`test_adapter.py`)：验证图像加载和格式转换。
- **数据集准备测试** (`test_prepare_dataset.py`)：验证数据集拆分逻辑。

## 运行方式

```bash
# 运行全部测试
python -m pytest -q

# 跳过需要 PyTorch 的测试（CI 环境）
python -m pytest -q -k "not torch"

# 油棕 A0 路由与模型模式测试
python -m pytest -q tests/test_oil_palm_routing.py tests/test_oil_palm_foundation.py

# 油棕 UAV 数据准备与训练 dry-run 测试
python -m pytest -q tests/test_oil_palm_uav_dataset.py

# Rust cloud 单元与 smoke 测试
cd cloud && cargo test
```

注意：A0 real predictor 的单元测试使用 fake `ultralytics` 模块验证
YOLO 输出到统一 envelope 的映射，不需要真实 `.pt` 权重。Cloud 的 session
流程默认使用本地 mock A0；只有设置 `AI_OIL_PALM_A0_DETECT_URL` 时才会调用
AI Engine 的 `/api/v1/oil-palm/a0/detect`。
