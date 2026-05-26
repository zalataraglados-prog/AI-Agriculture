# models

## Current A0 Baseline

`models/oil_palm/a0_image_routing/` now contains the first trained A0 YOLO
baseline metadata:

- `metrics.json` records `oil_palm_a0_yolo_structure_detector_v1` metrics.
- `inference_config.yaml` records the intended real-model runtime settings.
- `training_config.example.yaml` records the reproducible training defaults.
- `runs/a0_yolo_structure_detector_v1/` contains local training artifacts and
  weights, but this directory is ignored by Git.

The runtime still uses the mock oil palm predictor until real YOLO predictor
wiring is implemented.

## Current UAV Training Prep

`models/oil_palm/uav_tree_crown/` now contains the first UAV YOLO training
configuration:

- `training_config.example.yaml` records Colab-ready YOLOv8n defaults.
- `model_card.md` documents the single-class crown-detection scope.
- `metrics.example.json` remains the template until the first training run.

The prepared dataset metadata is tracked under
`datasets/oil_palm/manifests/uav_tree_crown.json`; raw images, generated YOLO
files, model runs, and `.pt` weights remain ignored by Git.

用于存放模型相关文件（权重、标签、配置、Model Card、指标模板）。

## 目录结构

```text
models/
├── rice/
│   └── rice_leaf_classifier/
│       ├── best_model.pth      # 训练权重（不提交到 Git）
│       ├── labels.json         # 类别标签映射
│       ├── config.yaml         # 模型架构配置
│       └── advice_map.yaml     # 病害建议映射
└── oil_palm/
    ├── README.md               # 油棕模型总览
    ├── ffb_maturity/           # FFB 果串检测 + 成熟度分类
    │   ├── model_card.md
    │   ├── labels.json
    │   └── metrics.example.json
    ├── uav_tree_crown/         # UAV 树冠检测
    │   ├── model_card.md
    │   ├── labels.json
    │   └── metrics.example.json
    ├── ganoderma_risk/          # Ganoderma 风险分类
    │   ├── model_card.md
    │   ├── labels.json
    │   └── metrics.example.json
    └── a0_image_routing/        # A0 图片角色路由
        ├── model_card.md
        ├── labels.json
        └── metrics.example.json
```

## 说明

- 训练权重文件（`.pth`、`.pt`、`.onnx`）不提交，按 `.gitignore` 排除。
- 生产部署通过 Volume Mount 挂载：`-v ./models:/app/models:ro`。
- 这里只维护模型版本说明、标签、指标模板与 Model Card。
- 油棕模型通过 `OIL_PALM_MODEL_MODE` 环境变量控制 mock/real/hybrid 模式。
  详见 `models/oil_palm/README.md`。
