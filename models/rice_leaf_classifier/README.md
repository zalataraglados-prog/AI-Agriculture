# rice_leaf_classifier

这是当前 AI 组第一版水稻叶片病害分类 baseline 的模型目录。

当前阶段说明：

- 任务类型：图像分类（不是目标检测）
- 模型结构：ResNet18
- 类别数量：8
- 输入尺寸：224
- 主要产物：
  - labels.json
  - config.json
  - 训练得到的 best_model.pth / final_model.pth（默认不直接提交到 Git）

注意：

- 当前 baseline 来自将检测标注转换为整图分类标签。
- 如果后续升级到病斑定位/多目标检测，需要新建任务路线，而不是直接复用当前分类输出。
