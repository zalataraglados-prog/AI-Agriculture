# Oil Palm Datasets

## Current A0 Dataset Baseline

`a0_image_routing` has a generated YOLO baseline dataset derived from the local
Roboflow COCO exports under `E:\a0`.

- Dataset version: `roboflow_a0_2026_05_17`
- Labels: `fruit_bunch`, `trunk_base`, `crown_region`
- Images: 396
- Bboxes: 1174
- Split: train 279 / val 58 / test 59
- Negative empty-label images: not included yet
- License: `Unknown` until source-specific permissions are verified

Tracked metadata:

- `datasets/oil_palm/manifests/a0_image_routing.json`
- `datasets/oil_palm/a0_image_routing/dataset_card.md`
- `datasets/oil_palm/a0_image_routing/splits/`
- `datasets/oil_palm/a0_image_routing/licenses/sources.csv`

Ignored training data:

- `datasets/oil_palm/a0_image_routing/yolo/`
- `datasets/oil_palm/a0_image_routing/raw/`

本目录管理油棕各 AI 任务的数据集。

## 核心原则

### raw 层包容一切，processed/train 层统一一切

- **raw/**：原始数据照搬，不做任何修改。任何公开数据集、任何格式、任何标注方式都可以放进来。
- **processed/**：经过 importer 转换后的统一中间格式。
- **yolo/** 或 **classification/**：最终训练格式（YOLO detection / 分类目录）。
- **splits/**：只存放 manifest 和 split 索引，不存放图片。

### 数据与权重不入库

所有 `raw/`、`processed/`、`yolo/`、`classification/` 目录通过 `.gitignore` 排除。
只提交 manifest、README、license 记录和 `.gitkeep` 占位文件。
大文件通过云端对象存储或本地 Volume 管理。

### 按 plantation/block/mission 分组划分

同一棵树、同一段视频、同一次 UAV mission 的高度相似图片
不能同时出现在 train 和 test 中。测试集应尽量模拟真实未知地块。

## 目录结构

```text
datasets/oil_palm/
├── README.md                              # 本文件
├── manifests/                             # 各任务数据集清单模板
│   ├── ffb_maturity.example.json
│   ├── uav_tree_crown.example.json
│   ├── ganoderma_risk.example.json
│   └── a0_image_routing.example.json
├── ffb_maturity/
│   ├── raw/              # gitignore — 原始数据
│   ├── processed/        # gitignore — 转换后中间格式
│   ├── yolo/             # gitignore — YOLO 训练格式
│   ├── splits/           # tracked — split 索引
│   └── licenses/         # tracked — 数据源 license 记录
├── uav_tree_crown/
│   ├── raw/
│   ├── processed/
│   ├── yolo/
│   ├── splits/
│   └── licenses/
├── ganoderma_risk/
│   ├── raw/
│   ├── processed/
│   ├── classification/   # gitignore — 分类目录格式
│   ├── splits/
│   └── licenses/
└── a0_image_routing/
    ├── raw/
    ├── processed/
    ├── yolo/
    ├── splits/
    └── licenses/
```

## 任务标签体系

### FFB 果串成熟度 (ffb_maturity)

目标检测 + 成熟度分类。

| Index | Label       | 说明                    |
|-------|-------------|------------------------|
| 0     | flower      | 花穗期                  |
| 1     | unripe      | 未成熟                  |
| 2     | underripe   | 接近成熟但未达采收标准    |
| 3     | ripe        | 成熟，可采收             |
| 4     | overripe    | 过熟                    |
| 5     | abnormal    | 异常/畸形果串            |

### UAV 树冠检测 (uav_tree_crown)

目标检测，单类。

| Index | Label           | 说明          |
|-------|-----------------|--------------|
| 0     | oil_palm_crown  | 油棕树冠       |

### Ganoderma 风险分类 (ganoderma_risk)

分类任务，v1 保守标签。

| Index | Label                 | 说明                    |
|-------|-----------------------|------------------------|
| 0     | healthy               | 健康                    |
| 1     | suspected_risk        | 疑似风险（未经专家确认）  |
| 2     | other_stress_unknown  | 其他胁迫/未知            |

> 没有专家或实验室确认时，标签使用 `suspected`，不写成 `confirmed`。

### A0 图片结构路由 (a0_image_routing)

YOLO bbox 检测任务，识别可进入后续模型的结构候选。

| Index | Label       | 说明              |
|-------|-------------|------------------|
| 0     | fruit_bunch | 果串候选           |
| 1     | trunk_base  | 树基部候选         |
| 2     | crown_region| 树冠/冠层候选       |

`unknown` 不作为 YOLO 类别；无有效结构的图片作为空标注负样本，推理时通过 `route_status` 表达。

## 数据准备工作流

1. 下载原始数据到 `{task}/raw/`
2. 编写或运行对应 importer（见 `ai_engine/crops/oil_palm/training/data_importers/`）
3. 转换结果输出到 `{task}/processed/` 和 `{task}/yolo/` 或 `{task}/classification/`
4. 运行 split 脚本，生成 `{task}/splits/` 下的 train/val/test 索引
5. 复制数据源 license 到 `{task}/licenses/`
6. 更新 manifest JSON

## 公开数据源参考

| 数据集 | 任务 | 链接 |
|-------|------|------|
| Scientific Data RGB-depth FFB 2025 | ffb_maturity | https://www.nature.com/articles/s41597-025-04953-6 |
| Oil Palm Fruits Dataset (Mendeley) | ffb_maturity | https://www.mendeley.com/catalogue/db2376f7-a002-33a7-8255-316aebfaa72b/ |
| MOPAD UAV Oil Palm | uav_tree_crown | https://www.sciencedirect.com/science/article/pii/S0924271621000083 |
| Gotani Ganoderma Dataset | ganoderma_risk | https://www.iradarx.com/gotani/datasets/ |

> 本项目不自行户外采集图片。优先使用公开数据集，可自行重新标注。
