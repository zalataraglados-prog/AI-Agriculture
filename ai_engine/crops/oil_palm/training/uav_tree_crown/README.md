# UAV Tree Crown Training

## 目标

训练 UAV tile 油棕树冠检测模型。只负责 tile 内 crown bbox/mask。
Cloud 继续负责 tile offset、全局坐标还原、NMS、tree confirmation。

## 推荐框架

YOLOv8 (ultralytics)

## 标签

`oil_palm_crown` (单类)

详见 `models/oil_palm/uav_tree_crown/labels.json`

## 数据来源

参考 `datasets/oil_palm/manifests/uav_tree_crown.example.json`

## 训练命令

```bash
# TODO: 等真实训练脚本就绪后补充
# yolo detect train data=datasets/oil_palm/uav_tree_crown/yolo/data.yaml ...
```

## 评估命令

```bash
# TODO: 等评估脚本就绪后补充
```

## 注意事项

- 相邻 tile 可能有空间重叠，需要 Cloud 端 NMS 处理
- Split 必须按 mission 分组，防止数据泄漏
- GSD 和正射图拼接质量影响检测性能
