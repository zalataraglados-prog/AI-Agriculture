# Service

该目录包含 AI 推理模块的核心逻辑（四层架构的 L2 适配器层与 L3 核心层）。
不包含 L1 (FastAPI 路由) 

## 目录结构

*   `adapters/`: (L2) 负责处理外部输入（文件、字节流），转化为 core 层可直接使用的内部结构 (PIL.Image)。
*   `core/`: (L3) 核心推理引擎。仅包含纯 Python 与算法逻辑。所有的扩展模型都继承自 `base_predictor.py` 中的 `BasePredictor`。
*   `schemas/`: 契约优先的 Pydantic 数据结构定义。所有模块之间的数据传递均以此为准。
*   `infer.py`: 本地单图推理测试脚本。

## 示例

```bash
python service/infer.py \
--image-path test.jpg \
--checkpoint-path outputs/.../best_model.pth
```

输出格式
```json

{
"predicted_class": "...",
"confidence": 0.93,
"topk": [...]
}
```
注意事项
-----

* 这还不是一个网络服务

* 后续将添加FastAPI集成