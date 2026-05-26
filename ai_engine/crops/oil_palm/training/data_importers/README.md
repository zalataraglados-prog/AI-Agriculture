# Oil Palm Data Importers

## 设计原则

**raw 层包容一切，processed/train 层统一一切。**

每个 importer 负责将一个外部数据源从 `raw/` 转换为项目内部统一的 `processed/` 格式。
添加新数据集只需要新增一个 importer 文件，不需要修改训练流程或标签定义。

## 命名规范

```text
import_{task}_{source_short_name}.py
```

示例：
- `import_ffb_mendeley_2025.py` — Mendeley FFB 数据集转换器
- `import_ffb_roboflow.py` — Roboflow FFB 数据集转换器
- `import_ganoderma_csv_dataset.py` — CSV 格式 Ganoderma 数据集转换器
- `import_uav_coco_dataset.py` — COCO 格式 UAV 树冠数据集转换器
- `import_uav_roboflow_coco.py` — Roboflow COCO 格式 UAV 树冠数据集转换器
- `import_uav_roboflow_yolo.py` — Roboflow YOLO 格式 UAV 树冠数据集转换器

## 实现规范

每个 importer 必须：

1. 继承 `BaseImporter`（见 `base_importer.py`）
2. 实现 `convert()` 方法
3. 接受 `raw_dir`、`output_dir`、`label_map` 参数
4. 使用 `map_label()` 将源标签映射到内部标签
5. 输出到 `processed/` 或 `yolo/` 或 `classification/` 目录
6. 返回 `ImportResult` 摘要

## 工作流

```text
原始数据 (任何格式)
    ↓
raw/ (照搬, 不修改)
    ↓
importer (标签映射 + 格式转换)
    ↓
processed/ (统一中间格式)
    ↓
yolo/ 或 classification/ (最终训练格式)
```
