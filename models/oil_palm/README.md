# Oil Palm Models

本目录存放油棕各 AI 任务的模型配置、标签、Model Card 和指标模板。

## 核心原则

- **权重不入库**：`*.pt`、`*.pth`、`*.onnx` 等模型权重文件通过 `.gitignore` 排除。
- **部署挂载**：生产环境通过 Volume Mount 挂载：`-v ./models:/app/models:ro`
- **版本追踪**：每次模型更新在 `model_card.md` 中记录 Changelog，并更新 `metrics.example.json`。
- **模式切换**：通过 `OIL_PALM_MODEL_MODE` 环境变量控制 mock/real/hybrid 模式。

## 目录结构

```text
models/oil_palm/
├── README.md                        # 本文件
├── ffb_maturity/
│   ├── model_card.md                # Model Card
│   ├── labels.json                  # 标签映射
│   ├── metrics.example.json         # 指标模板
│   └── best.pt                      # 权重 (gitignore)
├── uav_tree_crown/
│   ├── model_card.md
│   ├── labels.json
│   ├── metrics.example.json
│   └── best.pt                      # 权重 (gitignore)
├── ganoderma_risk/
│   ├── model_card.md
│   ├── labels.json
│   ├── metrics.example.json
│   └── best.pt                      # 权重 (gitignore)
└── a0_image_routing/
    ├── model_card.md
    ├── labels.json
    ├── metrics.example.json
    └── best.pt                      # 权重 (gitignore)
```

## 任务概览

| 任务 | 类型 | 推荐框架 | 当前状态 | 标签数 |
|------|------|---------|---------|--------|
| ffb_maturity | 目标检测 + 分类 | YOLOv8 | mock | 6 |
| uav_tree_crown | 目标检测 | YOLOv8 | mock | 1 |
| ganoderma_risk | 图像分类 | ResNet / EfficientNet | mock | 3 |
| a0_image_routing | 图像分类 | MobileNet / EfficientNet | mock | 4 |

## 环境变量

| 变量名 | 默认值 | 说明 |
|--------|-------|------|
| `OIL_PALM_MODEL_MODE` | `mock` | 模型运行模式：`mock` / `real` / `hybrid` |
| `OIL_PALM_FFB_MODEL_PATH` | - | FFB 果串模型权重路径 |
| `OIL_PALM_UAV_CROWN_MODEL_PATH` | - | UAV 树冠模型权重路径 |
| `OIL_PALM_GANODERMA_MODEL_PATH` | - | Ganoderma 风险模型权重路径 |
| `OIL_PALM_A0_MODEL_PATH` | - | A0 图片路由模型权重路径 |
| `OIL_PALM_CONFIDENCE_THRESHOLD` | `0.5` | 推理置信度阈值 |

### 模式语义

- **mock**：所有 task 强制走 mock predictor。适合 demo、CI、无权重环境。
- **real**：所有已声明为 real 的 task 必须加载真实权重。缺失则 fail fast。适合正式验证。
- **hybrid**：有权重走 real，无权重 fallback mock。适合分阶段上线。
