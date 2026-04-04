# AI-Agriculture
智慧农业系统方案 - 新一代 AI + 农业平台。

当前仓库以 `cloud` Rust 接收端为核心，支持基于配置的传感器数据规则校验与 ACK 回传。

## Quick Start

### 1. 环境要求

- Rust 工具链（建议 stable，Edition 2021）
- Cargo（随 Rust 一起安装）
- Bash（用于运行脚本；Windows 可用 Git Bash / WSL）

### 2. 本地运行 cloud 接收端

```bash
cd cloud
cargo run -- --config config/sensors.toml --bind 0.0.0.0:9000 --timeout-ms 0
```

### 3. 本地配置冒烟测试（脚本）

```bash
cd cloud
chmod +x scripts/local_config_smoke_test.sh
./scripts/local_config_smoke_test.sh
```

### 4. 运行自动化测试

```bash
cd cloud
cargo test
```

## 技术栈与依赖清单

### 语言与运行时

- Rust（Edition 2021）

### 主要依赖（`cloud/Cargo.toml`）

- `serde`（含 `derive`）
- `toml`

### 部署相关

- `cloud/deploy.sh`：Linux 服务器一键部署脚本
- systemd（可选，脚本会优先使用）

## 目录结构（核心）

- `cloud/`：云端 UDP 接收器（Rust）
- `cloud/config/sensors.toml`：传感器规则配置
- `cloud/scripts/local_config_smoke_test.sh`：本地脚本 smoke test
- `cloud/tests/`：集成测试目录
- `doc/`：协作与流程文档

## 测试说明

- 单元测试：位于 `cloud/src/main.rs` 中
- 集成 smoke 测试：位于 `cloud/tests/smoke_e2e.rs`

## License

本项目使用 MIT License，详见根目录 `LICENSE`。
