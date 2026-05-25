---
trigger: manual
---

# AI-Agriculture 项目排错与部署 Rules 精华版

## 1. 核心部署与架构规范
*   **部署路径**：源码在 `/opt/src/AI-Agriculture`。旧的二进制在 `/opt/ai-agriculture`。但**核心原则**是：当前优先使用自带的 `scripts/deploy_cloud.sh` 脚本进行一键式部署与进程管理。
*   **资源受限架构妥协**：在内存较小（< 4G）的单机云服务器上，**数据库放容器，AI 推理服务在宿主机原生运行（基于 Conda/Python）**，防止强制使用 Docker 构建庞大镜像（如 PyTorch）时引发 OOM (Exit 137) 或磁盘写满。
*   **国内网络优化**：国内部署务必第一时间配置 Pip 全局镜像（如 `pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple`）；无 GPU 时必须加 `--index-url https://download.pytorch.org/whl/cpu` 专属源下载 CPU 版 PyTorch 避免超大的 CUDA 依赖。

## 2. 端口与数据库基准
*   **端口分布**：
    *   `8000`：Python FastAPI AI 引擎
    *   `8088` / `9000`：Rust Cloud 网页后端服务
    *   `55432`：宿主机映射的数据库端口（Docker 内部为 5432）
*   **数据库连接串**：本项目真实生产用户为 `app`，库为 `appdb`。连接串必须为 `host=127.0.0.1 port=55432 user=app password=YOUR_DB_PASSWORD dbname=appdb`。严禁提交真实密码。
*   **排错关键**：看到 `127.0.0.1:8088` 或 `127.0.0.1:5432` 时，先分清是 VS Code 本地端口转发、云端宿主机，还是 Docker 容器内部。不要乱杀本地 8088 进程导致 VS Code SSH 断开。

## 3. 高频错误速查 (Troubleshooting)
*   **前端打不开**：不一定是前端代码报错。先查 Rust 8088 端口是否监听，Rust 后端是否因为连不上 DB 而 panic 退出了。
*   **DB init error / invalid configuration**：往往是因为缺失密码。必须使用 `eprintln!("[ERROR_DETAIL] {:?}", e)` 打印完整错误栈。
*   **password authentication failed for "postgres"**：本项目真实用户是 `app` 而非 `postgres`。检查 `.env` 配置或 `sensors.toml`。
*   **配置不生效**：检查当前 shell 是否有同名环境变量（如 `$DATABASE_URL`）覆盖了 `sensors.toml` 中的设定。

## 4. UAV 与前端业务“防坑”指南
*   **image_url 丢失**：前端看不了正射图，通常是因为 `image_url` 字段在 Request -> Handler -> DB Insert -> DB Select 的某一层漏传了，没有真实落库。新增字段必须“全链路穿透”检查。
*   **逻辑瓦片化**：当前系统只是将图“逻辑切片”并写入 DB，并未实际生成物理切分好的 `z/x/y.png` 瓦片文件。
*   **Detection 资产化**：只有被人工 confirm 的 Detection 才会转为 tree 资产。多次确认同一个 detection 必须**幂等**（返回同一个 tree_code），并按坐标就近挂载，绝不能重复创建新树。
*   **前端树形列表兜底**：树形地块列表必须提供 "All Plantations" 兜底选项，且地块名称后必须要带上对应的树木总数（如 `test_plantation (#1) - 45 trees`）。

## 5. 核心排错黄金法则 (TL;DR)
**凡是连不上、打不开、图不显，务必按以下顺序严格排查：**
1. 本地端口转发是否正常
2. 远程进程是否存活 (`ss -tunlp`) 
3. 启动执行目录是否正确（跑的是 `cloud` 还是根目录程序）
4. 配置文件与环境变量优先级
5. 数据库 55432 端口与 app 用户的密码对不对
6. 数据字段是否真正落库

> 提示：修改代码后，若是编译型服务（Rust），必须 `build release` 并在部署目录重启进程才会生效；目前 `scripts/deploy_cloud.sh` 已封装此过程。
