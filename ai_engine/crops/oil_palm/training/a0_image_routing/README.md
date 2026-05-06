# A0 Image Routing Training

## 目标

训练图片角色分类模型 (A0 Routing Model)。
A0 是"入口守门员"，负责判断上传图片属于哪个 `image_role`。

## 推荐框架

MobileNetV3 / EfficientNet-B0 (轻量级，低延迟)

## 标签

`fruit`, `trunk_base`, `crown`, `unknown`

详见 `models/oil_palm/a0_image_routing/labels.json`

## 数据来源

训练数据可以从现有 FFB、Ganoderma、UAV crown 数据集中按角色重新标注获得。

## 训练命令

```bash
# TODO: 等 feature/oil-palm-a0-routing-model 分支实现后补充
```

## 设计原则

- A0 v1 **不替代**用户的 `image_role` 选择，只做校验和建议
- 不确定时应返回 `unknown` 而非错误角色
- 等模型稳定后，再允许 `auto` 路由模式
- A0 不在本分支注册到 OilPalmPipeline
