# FFB Maturity Training

## 目标

训练 FFB 果串检测 + 成熟度分类模型。

## 推荐框架

YOLOv8 (ultralytics)

## 标签

`flower`, `unripe`, `underripe`, `ripe`, `overripe`, `abnormal`

详见 `models/oil_palm/ffb_maturity/labels.json`

## 数据来源

参考 `datasets/oil_palm/manifests/ffb_maturity.example.json`

## 训练命令

```bash
# TODO: 等真实训练脚本就绪后补充
# yolo detect train data=datasets/oil_palm/ffb_maturity/yolo/data.yaml ...
```

## 评估命令

```bash
# TODO: 等评估脚本就绪后补充
# yolo detect val data=datasets/oil_palm/ffb_maturity/yolo/data.yaml ...
```

## 输出

- 权重: `models/oil_palm/ffb_maturity/best.pt` (不入库)
- 指标: 更新 `models/oil_palm/ffb_maturity/metrics.example.json`
- Model Card: 更新 `models/oil_palm/ffb_maturity/model_card.md` Changelog
