# PR 文案模板

## 通用模板（可直接复制）

```md
## 背景 / Background
为了解决 ______ 问题，本 PR 将 ______ 从“______”升级为“______”。

## 变更摘要 / Change Summary
1. 新增 / 调整 ______
2. 支持 ______
3. 补充 ______

## 测试说明 / Testing
### 本地验证命令 / Local Verification Commands
- `cargo test`

### 结果摘要 / Result Summary
- [x] 本地测试通过
- [x] 关键路径手工验证通过

## 配置与部署变更 / Config & Deploy Changes
- 新增配置文件：`...`
- 新增环境变量：`...`
- 部署脚本变更：`...`

## 风险与回滚 / Risks & Rollback
- 风险：______
- 回滚：回滚到提交 `xxxxxxx` 并重启服务

## 关联 Issue / Related Issue
Closes #___
```

## 当前项目示例（Cloud 配置化接收）

```md
## 背景 / Background
云端接收程序原先主要针对固定 payload（如 success）做匹配，不利于后续接入多传感器。
本 PR 将云端接收升级为“配置驱动规则匹配”，新增传感器时只需改配置，不需要改 Rust 代码。

## 变更摘要 / Change Summary
1. 新增 TOML 配置加载能力，支持 exact payload 与 sensor rule 两类规则。
2. 新增字段校验能力（required_fields + field_types）。
3. ACK 路由配置化：match / mismatch / unknown sensor。
4. 更新 deploy 脚本，部署时同步 `config/sensors.toml` 并以 `--config` 启动。
5. 补充本地 smoke test 脚本与 README 文档。

## 测试说明 / Testing
### 本地验证命令 / Local Verification Commands
- `cargo fmt`
- `cargo test`
- `./scripts/local_config_smoke_test.sh`

### 结果摘要 / Result Summary
- `success -> ack:success`
- `mq7:raw=206,voltage=0.166 -> ack:mq7`
- `mq7:raw=oops,voltage=0.166 -> ack:error`

## 配置与部署变更 / Config & Deploy Changes
- 新增：`cloud/config/sensors.toml`
- 新增：`CONFIG_PATH` 环境变量（默认 `${INSTALL_ROOT}/config/sensors.toml`）
- 变更：`deploy.sh` 使用 `--config` 方式启动 receiver

## 风险与回滚 / Risks & Rollback
- 风险：配置写错会导致匹配失败或 ACK 异常。
- 回滚：回滚本 PR 提交并重启 `ai-agri-cloud-receiver` 服务。
```
