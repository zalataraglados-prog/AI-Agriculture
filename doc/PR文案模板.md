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

