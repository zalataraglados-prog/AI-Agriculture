# AI-Agriculture

智慧农业系统方案 - 新一代 AI + 农业平台。

## 项目概览

当前仓库核心为 `gateway` 子项目：

- 负责采集（固定负载）并通过 UDP 上报。
- 使用 Rust 编写，适配 Linux/WSL 场景。

## Quick Start（可直接执行）

### 1) 前置依赖

- Rust（建议 stable）
	- 安装：`https://www.rust-lang.org/tools/install`
- Linux/WSL 额外系统依赖（串口模式需要）

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libudev-dev
```

### 2) 编译与运行

进入 gateway 目录：

```bash
cd gateway
```

最小可运行命令（发送 1 个 UDP 包，不等待 ACK）：

```bash
cargo run -- --target 127.0.0.1:9000 --count 1 --no-wait-ack
```

固定负载连续发送（默认 payload 为 `success`）：

```bash
cargo run -- --target 127.0.0.1:9000
```

串口模式（MQ-7）：

```bash
cargo run -- --target 8.134.32.223:9000 --serial-port /dev/ttyUSB0 --serial-baud 115200 --expected-ack ack:mq7
```

### 3) 运行测试

```bash
cd gateway
cargo test
```

## 技术栈与依赖清单

- 语言：Rust 2021 edition
- 网关依赖：
	- `serialport = 4.8.1`（见 `gateway/Cargo.toml`）
- 系统依赖（Linux/WSL 串口场景）：
	- `pkg-config`
	- `libudev-dev`

说明：本仓库目前主要是 Rust 网关工程，因此依赖入口为 Cargo 生态（`Cargo.toml`），不是 `package.json` 或 `requirements.txt`。

## 目录结构

- `gateway/`：Rust UDP 网关
- `doc/`：文档

## 许可证

本项目采用 MIT License，详见根目录 `LICENSE`。
