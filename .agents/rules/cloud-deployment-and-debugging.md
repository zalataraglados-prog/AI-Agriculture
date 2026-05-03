---
trigger: manual
---

# AI-Agriculture 项目排错与部署 Rules 精华版

以下是基于以前试错之后总结的经验，有参考价值的，但不一定一直正确，请以实际情况为准。

## 0. 项目基本结构

这是智慧农业项目 `AI-Agriculture`。在Linux云端服务器的路径(而不是我的本地电脑的项目结构)：

```text
源码路径：
/opt/src/AI-Agriculture

部署路径：
/opt/ai-agriculture

cloud 后端源码：
/opt/src/AI-Agriculture/cloud

部署后二进制：
/opt/ai-agriculture/cloud/bin/ai-agri-cloud-receiver

部署配置：
/opt/ai-agriculture/cloud/config/sensors.toml
```

注意：
**源码路径和部署路径不是同一个。**
修改源码后，不代表部署服务已经更新。必须重新编译、复制二进制、重启服务。

项目组件大致如下(这是很久以前的结构了，仅供参考，请以实时情况为准)：

```text
frontend_v2_premium/     前端静态页面
frontend/                部分前端静态资源
cloud/                   Rust 云端接收器 + 前端服务 + UAV API
service/                 Python FastAPI AI 引擎
docker-compose.yml       AI 引擎、TimescaleDB 等容器
Cargo.toml               根目录 Rust 项目，不是 cloud 后端
src/main.rs              根目录程序，不是网页服务
```

---

## 1. 端口含义必须区分

```text
8000：Python FastAPI AI 引擎
8088 或 9000：Rust Cloud Receiver + 前端网页服务
55432：宿主机访问 TimescaleDB/PostgreSQL 的端口
5432：PostgreSQL 容器内部端口
```

如果：

```text
http://127.0.0.1:8000 可以打开
http://localhost:8088 打不开
```

这只说明 **AI 引擎启动了**，不说明前端启动了。

前端网页由 Rust `cloud` 后端提供，不是 Python 8000 提供。

正确启动 cloud 后端：

```bash
cd /opt/src/AI-Agriculture/cloud
cargo run -- run --config config/sensors.toml
```

成功时应该看到类似：

```text
Running `target/debug/cloud run --config config/sensors.toml`
[cloud-http] Listening on ...
```

如果看到的是：

```text
Running `target/debug/cicsic_project ...`
```

说明你在项目根目录跑错了，启动的是根目录程序，不是 cloud 网页后端。

---

## 2. 不要随便杀 Windows 本地 8088 进程

在 VS Code Remote SSH 场景下，Windows 本地执行：

```powershell
netstat -ano | findstr :8088
```

看到：

```text
127.0.0.1:8088 LISTENING
```

不一定是项目后端占用。它很可能是：

```text
VS Code Remote SSH 本地端口转发进程
```

如果执行：

```powershell
stop-process -Id <PID> -Force
```

可能会导致远程服务器连接断开。

正确做法：

```text
1. 不要乱杀本地 8088 PID。
2. 打开 VS Code 的 Ports 面板。
3. 在 Ports 面板里停止、重新转发或修改端口。
4. 在远程 Linux 终端检查真正的服务端口。
```

远程 Linux 检查端口：

```bash
ss -tunlp | grep -E '8088|9000'
netstat -tunlp | grep -E '8088|9000'
```

本地 `localhost:8088`、远程服务器 `localhost:8088`、Docker 容器内部 `localhost:5432` 不是同一个东西。
看到 localhost 时必须先问：**是谁的 localhost？**

---

## 3. Docker 端口映射必须这样读

执行：

```bash
docker ps
```

如果看到：

```text
timescale/timescaledb:latest-pg16   127.0.0.1:55432->5432/tcp
```

含义是：

```text
宿主机端口：55432
容器内部端口：5432
```

如果 Rust cloud 程序运行在 Linux 宿主机上，连接数据库必须用：

```text
127.0.0.1:55432
```

不能用：

```text
127.0.0.1:5432
```

`5432` 是容器内部端口，不是宿主机端口。

---

## 4. 数据库连接最终正确配置

最终确认数据库连接信息为：

```text
host = 127.0.0.1
port = 55432
user = app
password = app_password_change_me
dbname = appdb
```

连接串格式：

```toml
database_url = "postgres://app:YOUR_DB_PASSWORD@127.0.0.1:55432/appdb"
```

真实服务器上密码是：

```text
app_password_change_me
```

但不要把真实密码提交进公开仓库。Rules 中记住：
**生产配置中把 `YOUR_DB_PASSWORD` 替换成真实密码。**

错误示例：

```toml
database_url = "postgres://postgres@127.0.0.1:55432/ai_agriculture"
database_url = "postgres://postgres:password@127.0.0.1:55432/ai_agriculture"
database_url = "postgres://postgres:app_password_change_me@127.0.0.1:55432/ai_agriculture"
database_url = "postgres://postgres:app_password_change_me@127.0.0.1:55432/CICSIC"
```

这些都可能错，因为真实应用用户不是 `postgres`，真实数据库不是 `ai_agriculture` / `CICSIC`，而是：

```text
user = app
dbname = appdb
```

---

## 5. 经典数据库报错与含义

### 报错 1

```text
DB init error: failed to connect postgres: error connecting to server
```

常见原因：

```text
1. 数据库容器没启动。
2. 端口错，用了 5432 而不是 55432。
3. 后端连错 host。
4. PostgreSQL 服务不可达。
```

先执行：

```bash
docker ps
ss -tunlp | grep 55432
```

---

### 报错 2

```text
DB init error: failed to connect postgres: invalid configuration
```

不要立刻猜 URL 格式错。
本项目中这个错误的真实原因曾经是：

```text
password missing
```

也就是连接串缺少密码。

必须打印完整错误结构：

```rust
eprintln!("[ERROR_DETAIL] {:?}", e);
```

简短错误：

```text
invalid configuration
```

可能会误导。

完整错误曾显示：

```text
Error { kind: Config, cause: Some("password missing") }
```

这才是真相。

---

### 报错 3

```text
password authentication failed for user "postgres"
SqlState(E28P01)
```

含义：

```text
密码字段有了，但用户名/密码组合错误。
```

本项目中原因是：

```text
用了 postgres 用户，但真实用户是 app。
```

不要继续猜密码，要检查容器环境变量：

```bash
docker inspect <container_id> | grep POSTGRES
```

重点看：

```text
POSTGRES_USER
POSTGRES_PASSWORD
POSTGRES_DB
```

---

### 报错 4

```text
database "xxx" does not exist
```

含义：

```text
host、port、user、password 可能都对了，但 dbname 错了。
```

本项目正确 dbname 是：

```text
appdb
```

---

## 6. 数据库排错标准流程

以后 cloud 后端连不上数据库，按这个顺序，不要乱猜：

```bash
# 1. 看数据库容器是否运行
docker ps

# 2. 看端口映射
# 找 127.0.0.1:55432->5432/tcp

# 3. 查数据库环境变量
docker inspect <container_id> | grep POSTGRES

# 4. 用 psql 直接验证
PGPASSWORD='app_password_change_me' \
psql -h 127.0.0.1 -p 55432 -U app -d appdb -c '\dt'

# 5. 再运行 cloud
cd /opt/src/AI-Agriculture/cloud
cargo run -- run --config config/sensors.toml
```

如果 psql 都连不上，Rust 后端一定也连不上。

Linux 写法：

```bash
PGPASSWORD='xxx' psql ...
```

PowerShell 写法不同：

```powershell
$env:PGPASSWORD='xxx'
psql ...
```

不要把 PowerShell 语法拿到 Linux 用，也不要把 Linux 语法拿到 PowerShell 用。

---

## 7. 配置文件与环境变量优先级

如果改了 `sensors.toml` 但似乎没生效，先检查：

```bash
echo $DATABASE_URL
```

可能存在环境变量覆盖配置文件。

一般配置来源可能有优先级：

```text
命令行参数
环境变量 DATABASE_URL
配置文件 sensors.toml
```

如果 `DATABASE_URL` 有旧值，会覆盖你改的配置文件。

---

## 8. 正确启动与部署流程

### 手动测试

```bash
cd /opt/src/AI-Agriculture/cloud
cargo run -- run --config config/sensors.toml
```

### 编译 release

```bash
cd /opt/src/AI-Agriculture/cloud
cargo build --release
```

### 更新部署服务

```bash
pkill -9 -f ai-agri-cloud-receiver
sleep 2

cp ../target/release/cloud /opt/ai-agriculture/cloud/bin/ai-agri-cloud-receiver

cd /opt/src/AI-Agriculture

CLOUD_MIGRATION_DIR=cloud/sql/migrations \
/opt/ai-agriculture/cloud/bin/ai-agri-cloud-receiver \
  --config /opt/ai-agriculture/cloud/config/sensors.toml \
  --bind 0.0.0.0:9000 \
  --timeout-ms 0 \
  > /opt/ai-agriculture/cloud/server.log 2>&1 &
```

### 看日志

```bash
tail -f /opt/ai-agriculture/cloud/server.log
```

### 查服务是否监听

```bash
ss -tunlp | grep -E '8088|9000'
```

注意：
如果你只修改了 `/opt/src/AI-Agriculture/cloud/config/sensors.toml`，但部署服务用的是 `/opt/ai-agriculture/cloud/config/sensors.toml`，那正式服务不会受影响。

---

## 9. 前端打不开时的分层排查

不要一上来改前端。按层排查：

```text
1. 浏览器访问的是哪个端口？
2. VS Code Ports 是否转发了该端口？
3. 远程 Linux 上该端口是否监听？
4. cloud 后端有没有启动？
5. cloud 是否卡在 DB init？
6. 数据库容器是否运行？
7. 数据库 host/port/user/password/dbname 是否正确？
8. 静态资源是否真的被 cloud 服务到？
9. API 返回的数据是否包含前端需要的字段？
```

核心判断：

```text
浏览器打不开 ≠ 前端代码一定错
8088 没监听 ≠ 端口被占用，可能是后端没启动
8000 能打开 ≠ 前端能打开
```

---

## 10. UAV / 正射影像功能核心链路

UAV 功能涉及：

```text
Mission：无人机任务
Orthomosaic：正射影像
Tile：瓦片
Detection：检测点
Tree：树资产
Plantation：地块
```

正射图显示链路：

```text
图片文件
  ↓
静态资源 URL
  ↓
注册 orthomosaic
  ↓
后端 uav.rs 接收 image_url
  ↓
db.rs 写入 image_url
  ↓
数据库保存 image_url / width / height / resolution
  ↓
viewer 查询 orthomosaic
  ↓
前端根据 image_url 加载图片
```

如果页面看不到正射图，不要只查图片文件，要查：

```text
数据库里的 orthomosaic.image_url 是否真的有值
API 返回 JSON 是否带 image_url
前端是否读取了正确字段
```

---

## 11. 经典 UAV bug：image_url 没有落库

曾经出现过：

```text
前端提交了 image_url
后端 API 接收了 image_url
但数据库 INSERT 没有写入 image_url
导致 viewer 查不到图片地址
页面无法显示正射图
```

修复点：

```text
cloud/src/uav.rs
cloud/src/db.rs
```

必须确保：

```text
1. request struct 包含 image_url
2. handler 读取 image_url
3. insert_uav_orthomosaic 函数参数包含 image_url
4. SQL INSERT 包含 image_url 字段
5. SELECT / response 返回 image_url
6. 前端 viewer 使用 image_url 加载图片
```

这个 bug 的本质是：

```text
字段没有全链路穿透。
```

以后新增字段时都要检查：

```text
前端请求体
后端 DTO / struct
handler
db function 参数
SQL INSERT
SQL SELECT
返回 JSON
前端读取
```

---

## 12. 真实正射图测试流程

曾用真实油棕俯视图测试：

```text
/opt/images/oil_palm/ortho_test.png
尺寸：658×438
```

大致流程：

```bash
# 放到静态资源目录
mkdir -p /opt/src/AI-Agriculture/frontend/static
cp /opt/images/oil_palm/ortho_test.png /opt/src/AI-Agriculture/frontend/static/
```

创建 mission：

```bash
curl -X POST http://localhost:8088/api/v1/uav/missions \
  -H "Content-Type: application/json" \
  -d '{"mission_name":"Real_Image_Test","plantation_name":"Demo Farm"}'
```

注册正射图：

```bash
curl -X POST http://localhost:8088/api/v1/uav/missions/<mission_id>/orthomosaic \
  -H "Content-Type: application/json" \
  -d '{"width":658,"height":438,"resolution":0.05,"image_url":"/static/ortho_test.png"}'
```

打开 viewer：

```text
http://localhost:8088/oil_palm/ortho_viewer.html?ortho_id=<orthomosaic_id>
```

如果看不到图，优先查：

```sql
SELECT id, image_url, width, height, resolution FROM uav_orthomosaics;
```

---

## 13. 瓦片化当前状态

当前系统主要是：

```text
逻辑瓦片化
```

不是完整的物理切图。

逻辑瓦片化做的是：

```text
根据 orthomosaic 的 width、height、tile_size、overlap
计算每个 tile 的 x、y、width、height
把 tile 元数据写入数据库
```

它不一定真的生成：

```text
tile_0_0.png
tile_0_1.png
z/x/y.png
```

如果要做 Google Maps 那种真实瓦片，需要额外做：

```text
GDAL / gdal2tiles.py
Pillow / OpenCV 切图
对象存储
tile_url 生成
多 zoom level 金字塔
```

不要把当前逻辑瓦片误认为已经完成了物理瓦片服务。

---

## 14. Detection / Tree 资产化规则

Mock Detection / AI Detection 产生的是候选检测点。

流程：

```text
orthomosaic
  ↓
tile / image region
  ↓
detection
  ↓
人工 confirm
  ↓
tree asset
```

应支持：

```text
POST /api/v1/uav/detections/{id}/confirm
POST /api/v1/uav/detections/{id}/reject
```

确认后：

```text
detection 转为 tree
```

重要规则：

```text
1. 未确认的 detection 不应直接污染 tree asset。
2. 重复 confirm 同一个 detection 不应创建重复 tree。
3. 应尽量复用已有 tree_code。
4. 后续重复扫描同一地块时，要根据坐标/距离匹配已有树，而不是每次创建新树。
```

目标：

```text
同一棵树多次扫描后仍然是同一个资产 ID。
```

---

## 15. Plantation / Tree 列表前端优化规则

如果树数量对不上，或者不知道哪个地块有树，前端应增加：

```text
All Plantations
```

让用户默认查看所有树。

地块下拉框建议显示：

```text
地块名 (#id) - N trees
```

例如：

```text
test_plantation (#1) - 45 trees
```

不要只显示模糊的 `test_plantation`，否则用户不知道哪个地块有数据。

---

## 16. Git / Commit 规则

用户希望以后修改代码后主动提交。

但注意：

```text
1. 可以 commit 调试代码，但最终合并前要清理。
2. 不要提交真实数据库密码。
3. 不要长期保留 DEBUG println。
4. commit message 要说明具体修复点。
```

常见 commit 示例：

```bash
git add cloud/config/sensors.toml
git commit -m "fix: update postgres port to 55432 in sensors config"

git add cloud/src/db.rs
git commit -m "debug: print detailed postgres connection error"

git add cloud/src/db.rs cloud/src/uav.rs
git commit -m "fix(cloud): ensure image_url is stored when registering orthomosaic"

git add cloud/src/db.rs cloud/config/sensors.toml
git commit -m "chore: clean up debug code and use database password placeholder"
```

如果用户说“帮我提交”，就直接执行：

```bash
git status
git add <changed-files>
git commit -m "<clear message>"
```

不要把命令写错，不要在 commit message 后面误加奇怪字符。

---

## 17. 最重要的经验总结

### 经验 1：不要只看表面现象

```text
localhost:8088 打不开
```

不等于前端坏了。可能是：

```text
Rust 后端没启动
后端卡在 DB init
数据库端口错
数据库密码错
VS Code 端口没转发
运行了错误 crate
部署服务还是旧二进制
```

---

### 经验 2：运行目录非常重要

```bash
cd /opt/src/AI-Agriculture
cargo run
```

运行的是根目录项目。

```bash
cd /opt/src/AI-Agriculture/cloud
cargo run -- run --config config/sensors.toml
```

运行的才是 cloud 后端。

看日志中的二进制名：

```text
target/debug/cloud              正确
target/debug/cicsic_project     跑错
```

---

### 经验 3：`invalid configuration` 要打印完整 Debug

不要一直猜。
直接加：

```rust
eprintln!("[ERROR_DETAIL] {:?}", e);
```

曾经最终发现：

```text
password missing
```

比表面的：

```text
invalid configuration
```

有用得多。

---

### 经验 4：数据库连接必须五项同时正确

```text
host
port
user
password
dbname
```

本项目最终正确：

```text
127.0.0.1
55432
app
app_password_change_me
appdb
```

---

### 经验 5：`docker ps` 是排错关键

看到：

```text
127.0.0.1:55432->5432/tcp
```

就要知道：

```text
宿主机程序连 55432
容器内部才是 5432
```

---

### 经验 6：字段必须全链路检查

`image_url` 曾经传了但没落库，导致正射图看不见。

新增字段必须检查：

```text
前端
API request
handler
db 参数
SQL INSERT
SQL SELECT
response
前端读取
```

---

### 经验 7：部署路径和源码路径不同

修改源码后必须：

```text
build release
复制二进制到部署路径
重启服务
确认服务用的是新二进制和正确 config
```

否则你以为修了，实际运行的还是旧版本。

---

## 18. 最终一句话 Rules

在 AI-Agriculture 项目中，如果前端或 UAV 功能出问题，先不要乱改前端。必须按层排查：本地端口转发、远程端口监听、是否进入 cloud 目录、Rust cloud 是否启动、DB init 是否失败、Docker 端口是否为 `55432->5432`、数据库连接是否使用 `app/app_password_change_me@appdb`、源码路径和部署路径是否混淆、字段是否真正写入数据库。经典错误包括：杀掉 VS Code 本地端口转发导致远程断开、在根目录运行了 `cicsic_project` 而不是 `cloud`、把 PostgreSQL 端口写成 5432、用 postgres 用户而不是 app 用户、看到 `invalid configuration` 却不打印完整 Debug、注册正射图时漏写 `image_url` 导致 viewer 无法显示图片。
