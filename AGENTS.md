# AGENTS.md

本文件是 AI-Agriculture 项目的 Agent 必读文件。任何参与本仓库编程、审查、重构、补丁生成的 Agent，必须先阅读本文件，再阅读 README、ai_engine/README、cloud/README、tests/README 和相关代码。

## 1. 项目一句话目标

AI-Agriculture 不是单一“病害识别 Demo”，而是要从水稻 MVP 演进为一个面向真实农业生产的、多作物、多模型、树级资产化、可接入无人机与 OpenClaw 分析助手的一站式 AI 农业平台。

当前重点方向是油棕。油棕功能的最终目标不是简单识别图片，而是围绕真实生产动作回答：

* 这棵树是谁？
* 它在哪里？
* 它当前长势如何？
* 是否有病害或异常风险？
* 是否有成熟果串？
* 预计产量如何？
* 农民下一步应该巡检、采收、复查还是等待？

公开资料也支持这个方向：AI 与数字农业的价值应落在更高效、可持续、韧性的农食系统上，尤其要在真正有价值的场景中服务小农户，并重视治理、技能、包容和伦理。([FAOHome][1])

## 2. 当前分支状态

当前 dev/feature 分支已经不是最早的水稻硬编码 MVP。

当前仓库已经形成这些基础：

* `ai_engine/`：Python FastAPI AI 推理引擎。
* `ai_engine/common/`：跨作物共享模块。
* `ai_engine/crops/rice/`：水稻模块。
* `ai_engine/crops/oil_palm/`：油棕模块，目前主要是 mock / placeholder。
* `cloud/`：Rust 云端后端、静态前端服务、数据接收、OpenClaw/图像上传/UAV/树档案相关接口。
* `frontend/portal/`：入口门户。
* `frontend/rice/`：水稻前端。
* `frontend/oil_palm/`：油棕前端占位。
* `models/rice/rice_leaf_classifier/`：水稻模型配置与标签。
* `requirements/`：按 base/dev/rice/oil-palm/inference/training 拆分依赖。
* `cloud/sql/migrations/0004_uav_coordinate_foundation.sql`：UAV 坐标底座与树档案表。
* `tests/test_uav_api.py`：UAV mission → orthomosaic → mock detections → confirm/reject → tree_code 的端到端流程测试。

不要把本项目误判为“只需要修一个上传图片接口的分类网站”。

## 3. 最高优先级原则

### 3.1 先资产化，再模型化

油棕项目的核心资产是“树”，不是“图片”。

任何油棕功能都应尽量围绕 tree / tree_code / plantation / mission / orthomosaic / detection / session 建模，而不是只围绕单张图片建模。

正确方向：

* UAV 正射图生成候选树冠。
* 人工确认 detection。
* 系统生成稳定 tree_code，例如 `OP-000001`。
* 之后农民拍摄的多张图片、果串识别、病害记录、长势记录、产量预测都挂到同一棵树上。
* Tree Profile 成为长期档案入口。

错误方向：

* 上传一张图片，立即假装给出整棵树最终结论。
* 每次图片分析都孤立存在。
* 不保存树 ID、不保存地块、不保存来源任务、不保存历史。

### 3.2 模型是插件，场景是路由，结果是统一 Schema

不要让某个模型、某个作物、某个页面把系统写死。

正确原则：

* crop 独立：`ai_engine/crops/rice` 和 `ai_engine/crops/oil_palm` 不应互相导入。
* shared 逻辑放 `ai_engine/common`。
* FastAPI 根据 `CROP_PROFILE` 加载不同 crop profile。
* Rust cloud 通过配置和接口调用 AI 服务。
* 前端入口按 `/`、`/rice/`、`/oil_palm/` 分离。
* 结果结构应长期兼容，保留 `metadata`、`geometry`、`model_version`、`status` 等字段。

### 3.3 不为眼前 demo 破坏未来边界

本项目长期会包含：

* 水稻病害识别。
* 油棕树级资产系统。
* UAV 正射图与树冠检测。
* 油棕病害分析。
* 油棕长势分析。
* 油棕果串成熟度/产量评估。
* 条码/二维码绑定 Tree Profile。
* OpenClaw 聊天与数据分析助手。
* 多模型推理与多模态结果解释。

因此任何修改都必须避免后期返工。

严禁：

* 把 rice 路径硬编码到通用逻辑。
* 把 oil_palm 写进 rice 模块。
* 把模型路径写死在代码里。
* 把服务器 IP、端口、上传路径写死。
* 为了临时测试绕过 schema、绕过配置、绕过测试。
* 直接删除 legacy endpoint，除非用户明确要求。
* 修改 `.github/workflows/ci.yml` 覆盖 dev 分支；目前原则是 main 需要保护，dev 可直接 push。

## 4. 当前工程边界

### 4.1 AI Engine

`ai_engine/` 是 AI 推理微服务。

关键入口：

* `ai_engine/main.py`
* `ai_engine/infer.py`
* `ai_engine/common/`
* `ai_engine/crops/rice/`
* `ai_engine/crops/oil_palm/`

当前保留的端点：

* `GET /api/v1/health`
* `POST /api/v1/predict` legacy rice endpoint
* `POST /api/v1/rice/predict`
* `GET /api/v1/rice/health`
* `POST /api/v1/oil-palm/analyze-image`
* `POST /api/v1/oil-palm/analyze-session`
* `POST /api/v1/oil-palm/analyze-uav-mission`

油棕端点当前是 mock，不要把 mock 当最终业务完成。mock 的价值是先固定接口和前后端链路。

### 4.2 Rust Cloud

`cloud/` 是云端后端与静态服务核心。

重点关注：

* `cloud/src/http_server.rs`
* `cloud/src/db.rs`
* `cloud/src/uav.rs`
* `cloud/src/tree.rs`
* `cloud/src/image_upload.rs`
* `cloud/src/ai_client.rs`
* `cloud/sql/migrations/`

当前已经有 UAV/tree foundation：

* `POST /api/v1/uav/missions`
* `POST /api/v1/uav/missions/{mission_id}/orthomosaic`
* `POST /api/v1/uav/orthomosaics/{orthomosaic_id}/detections/mock`
* `GET /api/v1/uav/orthomosaics/{orthomosaic_id}/detections`
* `POST /api/v1/uav/detections/{detection_id}/confirm`
* `POST /api/v1/uav/detections/{detection_id}/reject`
* `GET /api/v1/trees/{tree_code}`

确认 detection 时应保持幂等：同一个 detection 重复 confirm，应返回同一个 tree_code，而不是创建重复树。

### 4.3 Frontend

前端分为：

* `frontend/portal/`
* `frontend/rice/`
* `frontend/oil_palm/`
* `cloud/dashboard/`

Rust 静态路由需要支持：

* `/`
* `/rice/`
* `/rice/rice_dashboard.html`
* `/oil_palm/`
* `/oil_palm/index.html`

不要把 oil_palm 页面塞进 rice 页面，不要让 portal 直接承担具体作物业务。

## 5. 油棕产品原则

### 5.1 农民真实使用流程

理想流程：

1. 管理员或团队上传 UAV 正射图。
2. 系统创建 UAV mission。
3. 系统登记 orthomosaic。
4. 模型或 mock detection 生成候选树冠。
5. 人工确认候选树。
6. 系统生成 tree_code / barcode。
7. 农民扫码或选择树进入 Tree Profile。
8. 农民围绕同一棵树上传多张图片，例如树基部、冠层、果串、叶片。
9. 系统先返回即时局部分析。
10. 当该树的图片证据足够时，再形成更完整的树级判断。
11. OpenClaw 可以基于历史记录、传感器、图片分析、巡检记录做解释和建议。

不要设计成“农民随便拍一张图，系统立刻给出完整产量和病害结论”。

### 5.2 油棕三大 AI 能力

油棕最终能力分三类：

#### 病害分析

重点不是武断确诊，而是：

* 疑似风险等级。
* 证据位置。
* 建议复查。
* 是否需要专家确认。
* 是否需要隔离或持续监测。

尤其是 Ganoderma / basal stem rot 相关风险，应谨慎表达。没有专家或实验室确认时，标签应使用 `suspected`，不要写成 `confirmed`。

#### 长势分析

长势分析应优先基于树级资产和 UAV/多图历史，而不是单张叶片。

长期应包含：

* 树冠位置。
* 树冠大小。
* 缺株/死株。
* 异常树。
* 长势等级。
* 历史变化。
* 巡检优先级。

#### 产量评估

产量评估不能只靠一张图。

长期应综合：

* FFB 果串检测数量。
* 成熟度分布。
* 树龄。
* 树种/品种。
* 地块。
* 历史采收。
* 长势。
* 病害风险。
* 天气/施肥/巡检记录。

输出应包含置信度或不确定性，不要假装精确。

## 6. 推荐模型路线

第一阶段不要急着训练所有模型。当前更重要的是先把资产底座、数据结构、接口和页面跑通。

长期推荐模型：

1. FFB 果串检测 + 成熟度模型。
2. BSR / 树体病害阶段分类模型。
3. 病害症状定位模型。
4. UAV 树冠检测/实例分割模型。
5. 长势/营养/胁迫评分模型。
6. 产量评估模型。
7. 图像质量/拍摄引导模型。

第一版优先级：

1. UAV tree detection mock → 真实模型替换点。
2. Tree registry / tree_code / barcode。
3. Tree Profile。
4. 多图 session。
5. FFB detection。
6. 病害 risk analysis。
7. 产量 estimate。


## 7. 数据原则

### 7.1 当前团队原则

团队不打算自己去户外采集图片。优先使用公开数据集训练模型。

允许：

* 使用公开数据集。
* 使用公开图片后自行重新标注。
* 将公开数据集转换为 YOLO/COCO/分类目录。
* 用 mock 数据先固定接口。

不应假设：

* 团队已经拥有真实油棕田间图片。
* 团队已经有 GPS 标注。
* 团队已经有真实 UAV orthomosaic。
* 团队已经有专家标注病害确认数据。

### 7.2 标签原则

油棕标签要区分任务场景。

FFB 树上采收场景：

* flower
* unripe
* underripe
* ripe
* overripe
* abnormal
* occluded_unknown 可选

采后分级场景：

* unripe
* ripe
* overripe
* empty_bunch
* damaged
* abnormal

病害场景：

* healthy
* suspected_early
* moderate
* severe
* dead_or_collapsed
* other_stress_unknown

没有专家证据时，不要把 suspected 写成 confirmed。

### 7.3 数据划分原则

不要随机乱分。

更好的划分方式：

* 按 plantation / block / mission / video / tree 分组划分。
* 同一棵树、同一段视频、同一次 UAV mission 的高度相似图片不要同时进入 train 和 test。
* 测试集应尽量模拟真实未知地块。
* 保存数据来源和 license。

## 8. API 与 Schema 原则

### 8.1 统一响应

AI 结果应尽量使用稳定 envelope：

* `status`
* `results`
* `metadata`
* `model_version`
* `geometry`

即使当前只有单图，也应保留 list 结构，避免未来 batch/session 引入破坏性变更。

### 8.2 geometry 的意义

`geometry` 是未来 bbox/mask/树冠/果串位置的容器。

不要删除它。

油棕任务强依赖 geometry，因为：

* 果串需要 bbox。
* 树冠需要 bbox/mask。
* 病害证据最好能定位。
* UAV detection 需要坐标。
* 前端需要 overlay。

### 8.3 metadata 的意义

`metadata` 用于承载：

* crop
* task
* session_id
* tree_code
* plantation_id
* mission_id
* orthomosaic_id
* model profile
* image quality
* advice
* uncertainty
* data source

不要把所有字段拍扁成一次性响应。

## 9. 配置原则

必须 configuration-first。

应通过环境变量或配置文件切换：

* `CROP_PROFILE`
* `MODEL_CHECKPOINT_PATH`
* `MODEL_LABELS_FILE`
* `MODEL_CONFIG_FILE`
* `MODEL_ADVICE_FILE`
* `AI_PREDICT_URL`
* `OPENCLAW_URL`
* `CLOUD_BIND_ADDR`
* `STATIC_SOURCE_FRONTEND`
* `STATIC_TARGET_FRONTEND`
* `TOKEN_STORE_PATH`
* `REGISTRY_PATH`
* `TELEMETRY_STORE_PATH`
* `IMAGE_STORE_PATH`
* `IMAGE_INDEX_PATH`

禁止：

* 代码里写死固定服务器 IP。
* 代码里写死本机绝对路径。
* 代码里写死某个开发者机器路径。
* 为了部署改源代码。

验收规则：

* 切换环境只改配置，不改代码。

## 10. 测试原则

任何修改都应尽量保持以下命令可运行：

Python:

* `python -m pytest -q`
* `python -m ai_engine.infer --help`

AI Engine:

* `CROP_PROFILE=rice uvicorn ai_engine.main:app --host 0.0.0.0 --port 8000`
* `CROP_PROFILE=oil_palm uvicorn ai_engine.main:app --host 0.0.0.0 --port 8000`

Docker:

* `docker build --build-arg CROP_PROFILE=rice .`
* `docker build --build-arg CROP_PROFILE=oil_palm .`

Rust cloud:

* `cd cloud && cargo test`
* `cd cloud && cargo run -- --config config/sensors.toml --bind 0.0.0.0:9000 --timeout-ms 0`

前端人工访问：

* `/`
* `/rice/`
* `/rice/rice_dashboard.html`
* `/oil_palm/`
* `/oil_palm/index.html`

UAV/tree flow：

* 创建 mission。
* 创建 orthomosaic。
* 创建 mock detections。
* 查询 detections。
* confirm detection。
* 重复 confirm 应幂等。
* reject detection。
* rejected detection 不应再 confirm。
* `GET /api/v1/trees/{tree_code}` 应能查到树。

## 11. 分支与协作原则

推荐分支命名：

* `feature/oil-palm-tree-registry`
* `feature/uav-coordinate-foundation`
* `feature/tree-profile`
* `feature/oil-palm-ffb-detection`
* `feature/oil-palm-session-analysis`
* `fix/rice-dashboard-encoding`
* `refactor/ai-engine-schema`
* `docs/update-agent-guide`

每个 PR 应明确：

* 本次目的。
* 改了哪些模块。
* 是否影响 legacy rice endpoint。
* 是否影响 cloud Rust 接口。
* 是否影响前端路由。
* 是否需要 migration。
* 如何测试。
* 下一步不做什么。

不要把多个大目标混在一个 PR 里。

## 12. 当前最重要的下一步

当前分支已经有 UAV coordinate foundation 的雏形。后续 Agent 优先做：

1. 巩固 UAV/tree registry 数据模型。
2. 完善 Tree Profile 页面。
3. 完善 barcode / tree_code 流程。
4. 让图片上传可以关联 tree_code / session。
5. 建立 oil_palm 多图 session schema。
6. 让 oil_palm mock 返回接近真实模型的结构。
7. 再替换真实 UAV tree detection / FFB / disease 模型。

不要一上来就做果串模型。没有树级资产底座，模型结果无法沉淀成农业生产价值。

## 13. 修改代码前必须检查

Agent 在改代码前必须先回答自己：

* 我改的是 rice、oil_palm、common、cloud、frontend 还是 docs？
* 这个改动会不会让作物之间重新耦合？
* 这个改动会不会破坏 legacy `/api/v1/predict`？
* 这个改动是否应该通过配置完成，而不是写死？
* 是否需要 migration？
* 是否需要更新 README / AGENTS.md / tests？
* 是否保留了未来 batch/session/geometry/metadata 扩展空间？
* 是否把 mock 和真实实现边界写清楚？
* 是否误把一张图片当成整棵树的最终结论？

## 14. 禁止事项清单

严禁：

* 删除 `ai_engine/common/schemas/prediction.py` 中的扩展字段。
* 删除 legacy rice endpoint，除非用户明确要求。
* 在 oil_palm 模块导入 rice 业务逻辑。
* 在 rice 模块导入 oil_palm 业务逻辑。
* 把模型权重提交进仓库，除非 README 明确允许。
* 把 local_data、outputs 的大文件提交进仓库。
* 把密钥、token、真实密码提交进仓库。
* 在代码里硬编码服务器地址。
* 为了让测试过而降低 schema 质量。
* 把 mock endpoint 包装成真实 AI 能力。
* 让 OpenClaw 直接替代结构化数据存储。
* 在没有证据时输出医学/农学式“确诊”。
* 修改 `.github/workflows/ci.yml` 的 main/dev 策略，除非用户明确要求。
* 禁止提交非 UTF-8 编码或带 BOM 的文本文件，确保跨平台兼容性。

## 15. OpenClaw 原则

OpenClaw 是分析助手，不是数据库，也不是业务事实源。

正确用法：

* 解释模型结果。
* 汇总树档案。
* 分析历史趋势。
* 根据结构化数据生成建议。
* 帮农民理解下一步动作。
* 辅助生成巡检计划。

错误用法：

* 让 OpenClaw 保存唯一事实。
* 让 OpenClaw 替代 schema。
* 让 OpenClaw 直接决定树是否患病。
* 让 OpenClaw 输出无法追溯的数据。
* 注意 Context 控制：OpenClaw 调用工具获取结构化数据时，应注意数据量大小，避免一次性加载数千个对象（如全量坐标）导致上下文窗口溢出。

## 16. 文档原则

文档要服务未来 Agent 和团队协作。

每次重要架构变化后，应同步更新：

* `README.md`
* `ai_engine/README.md`
* `cloud/README.md`
* `tests/README.md`
* `models/README.md`
* `AGENTS.md`

文档里要写清楚：

* 当前已经完成什么。
* 当前只是 mock 什么。
* 哪些是未来计划。
* 如何运行。
* 如何测试。
* 哪些东西不要改。

## 17. 最终判断标准

一个修改是好修改，当且仅当它让项目更接近：

* 多作物可扩展。
* 多模型可插拔。
* 前后端解耦。
* 配置优先。
* 树级资产可沉淀。
* UAV 与农民手机图片可合流。
* OpenClaw 可基于结构化事实分析。
* 真实农业动作可落地。
* 后续 Agent 更容易理解，不需要反复重新解释背景。

本项目最核心的一句话：

不要只把图片变成标签；要把农业现场数据变成可追踪、可解释、可行动的树级生产决策。
