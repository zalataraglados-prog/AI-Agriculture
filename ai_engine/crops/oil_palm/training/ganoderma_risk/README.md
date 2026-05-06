# Ganoderma Risk Training

## 目标

训练 trunk_base 图片的 Ganoderma / BSR 风险分类模型。

**重要：输出必须是 `suspected` 风险等级，绝不输出"确诊"。**

## 推荐框架

ResNet18 / EfficientNet (torchvision / timm)

## 标签

v1 保守标签: `healthy`, `suspected_risk`, `other_stress_unknown`

详见 `models/oil_palm/ganoderma_risk/labels.json`

若数据质量足够，未来可扩展:
`suspected_early`, `moderate`, `severe`, `dead_or_collapsed`

## 数据来源

参考 `datasets/oil_palm/manifests/ganoderma_risk.example.json`

## 训练命令

```bash
# TODO: 等真实训练脚本就绪后补充
```

## 评估命令

```bash
# TODO: 等评估脚本就绪后补充
```

## 注意事项

- Ganoderma 公开数据稀缺，可能需要小样本/迁移学习策略
- RGB-only 分类可能无法检测早期感染
- 所有正向预测必须标记为 `suspected_not_confirmed`
- 混淆矩阵中应特别关注 false negative rate
