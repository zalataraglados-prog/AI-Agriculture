# models

用于存放模型相关文件（权重、标签、配置）。

## 目录结构

```
models/
├── rice/
│   └── rice_leaf_classifier/
│       ├── best_model.pth      # 训练权重（不提交到 Git）
│       ├── labels.json         # 类别标签映射
│       ├── config.yaml         # 模型架构配置
│       └── advice_map.yaml     # 病害建议映射
└── oil_palm/                   # 油棕模型（待接入）
```

## 说明

- 训练得到的权重文件（`.pth`）默认不直接提交，按仓库 `.gitignore` 规范决定。
- 生产部署通过 Volume Mount 挂载：`-v ./models:/app/models:ro`。
- 这里只维护模型版本说明与占位文件。
