# AI-ag Agent Skill（协作版）

## 1. 目标与边界
- 目标：让 Agent 通过统一命令完成“服务巡检、日志定位、数据库检查、网关管理、Token管理、应急封锁”。
- 边界：仅允许执行白名单命令；不在白名单内的命令必须拒绝并返回引导性错误。
- 前端协作约定：当前完整前端在 `frontend_v2_premium/`，不是 `cloud/dashboard/`。

## 2. 对话与执行规则
- 默认输出简短结论 + 关键证据（命令回显、状态码、时间戳）。
- 涉及停机、注销、封锁等高风险命令，先二次确认再执行。
- 所有数据库类操作必须显式带路径/范围参数，禁止“全库模糊扫描”。
- 若发现服务未运行，优先给出可恢复路径：`status -> logs -> start/restart`。

## 3. 命令白名单（AI-ag）
- `AI-ag server`：输出服务健康摘要（是否运行、是否卡顿、关键端口）。
- `AI-ag start`：启动服务（替代 `cargo run`）。
- `AI-ag stop`：停止服务。
- `AI-ag restart`：重启服务（提示会触发落盘动作，避免滥用）。
- `AI-ag log -t <N>`：最近日志，`-t` 为行数，默认 50。
- `AI-ag database -p <PATH> -t <RANGE>`：数据库检查（`-p/-t` 必填）。
- `AI-ag check-database`：检查分块文件/时间主轴缺失。
- `AI-ag token`：输出当前 token（新设备注册用）。
- `AI-ag refresh-token`：立即废弃旧 token 并生成新 token。
- `AI-ag ls-port`：列出占用端口（重点校验 UDP 与前端端口）。
- `AI-ag ls-gateway`：列出已注册网关（设备ID、注册时间、在线时长）。
- `AI-ag check-gateway -n <DEVICE_ID> -t <SEC>`：连续观察指定网关日志（`-n` 必填）。
- `AI-ag deregister-gateway -n <DEVICE_ID>`：注销网关（`-n` 必填）。
- `AI-ag lockdown -token <TOKEN>`：停止服务并封锁端口（`-token` 必填）。
- `AI-ag V`：输出版本号（当前 Git 提交哈希）。
- `AI-ag help`：输出全部命令用法。

## 4. 后端对接契约（给 Rust 后端 + Agent）
- Agent（OpenClaw）对内服务建议：`127.0.0.1:3000`，不对公网暴露。
- 统一 chat 契约：
  - Request: `{ "message": "...", "context": { ... } }`
  - Response: `{ "reply": "..." }`
- Rust 后端提供代理：`POST /api/v1/chat`
  - 作用：前端只访问 cloud，不直连 Agent，避免 CORS 与安全暴露。
  - 推荐：转发前注入最新遥测与图传诊断上下文（可选但强烈建议）。

## 5. 前端协作注意事项
- 本轮前端改动主目录：`frontend_v2_premium/`。
- 若线上仍在服务 `cloud/dashboard/`，则视为“未接入前端成果”。
- 验收时必须同时提供：
  - `/api/v1/telemetry`、`/api/v1/image/uploads`、`/api/v1/sensor/schema` 响应样例；
  - 前端实际加载入口与静态目录映射证据。
