# AI-Agriculture
智慧农业系统方案 - 新一代 AI + 农业平台。

当前仓库以 `cloud` Rust 接收端为核心，支持基于配置的传感器数据规则校验与 ACK 回传。

## Quick Start
- 负责采集（固定负载）并通过 UDP 上报。
- 使用 Rust 编写，适配 Linux/WSL 场景。

### 1. 环境要求

- Rust 工具链（建议 stable，Edition 2021）
- Cargo（随 Rust 一起安装）
- Bash（用于运行脚本；Windows 可用 Git Bash / WSL）

### 2. 本地运行 cloud 接收端

```bash
cd cloud
cargo run -- --config config/sensors.toml --bind ${CLOUD_BIND_ADDR:-0.0.0.0:9000} --timeout-ms 0
```
*(注：可通过配置 `CLOUD_BIND_ADDR` 环境变量或 `sensors.toml` 更改端口与IP。网关侧及串口波特率配置详见网关子仓文档。)*

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

## AI 模块补充说明

- `service/`：本地模型加载与单张图片推理逻辑，详见 `service/README.md`
- `scripts/`：数据集整理、训练等脚本，详见 `scripts/README.md`
- `tests/`：AI 模块的 `pytest` 测试代码
- `models/`：分类模型的配置文件、标签文件与说明
- `local_data/`：本地数据说明与数据集目录
- `outputs/`：训练输出与模型产物目录

## Python 依赖与测试补充

如需运行 AI 相关脚本或测试，可在仓库根目录执行：

```bash
pip install -r requirements.txt
pytest -q
```

补充说明：

- 根目录已提供 `pytest.ini`，可直接在仓库根目录运行 `pytest`，无需额外设置 `PYTHONPATH`
- 如仅需运行 AI 模块测试，可使用 `pytest -q tests`
- `tests/test_infer_smoke.py` 依赖 `torch` 与 `torchvision`

## Configuration-First Deployment Notes

Use environment variables or deployment config files to switch environments.
Do not hardcode a fixed server IP, serial path, or port in code.

Recommended variables:

- `CLOUD_BIND_ADDR` (example: `0.0.0.0:9000`)
- `AI_PREDICT_URL` (example: `http://ai-engine:8000/api/v1/predict`)
- `OPENCLAW_URL` (example: `http://openclaw:3000`)
- `TOKEN_STORE_PATH`, `REGISTRY_PATH`, `TELEMETRY_STORE_PATH`
- `IMAGE_STORE_PATH`, `IMAGE_INDEX_PATH`, `IMAGE_DB_ERROR_STORE_PATH`

Gateway-side variables (WSL/edge):

- `GATEWAY_CLOUD_TARGET`
- `GATEWAY_BAUD_LIST`
- `GATEWAY_MODBUS_PORT`
- `GATEWAY_IMAGE_UPLOAD_URL` (or compose via gateway config)

Acceptance rule:

- Switch target environment by changing configuration only, without code edits.

## License

本项目使用 MIT License，详见根目录 `LICENSE`。
