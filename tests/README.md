# tests

## Oil Palm A0 Checks

- `test_oil_palm_foundation.py` validates oil palm manifests, labels, metrics
  templates, and model-mode safety.
- `test_oil_palm_routing.py` validates the A0 mock routing contract.
- `test_oil_palm_a0_dataset.py` validates the A0 Roboflow COCO adapter,
  grouped split output, YOLO label ranges, duplicate skipping, and training
  dry-run argument construction.

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
```
