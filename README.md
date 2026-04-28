# AI-Agriculture
鏅烘収鍐滀笟绯荤粺鏂规 - 鏂颁竴浠?AI + 鍐滀笟骞冲彴銆?

褰撳墠浠撳簱浠?`cloud` Rust 鎺ユ敹绔负鏍稿績锛屾敮鎸佸熀浜庨厤缃殑浼犳劅鍣ㄦ暟鎹鍒欐牎楠屼笌 ACK 鍥炰紶銆?

## Quick Start
- 璐熻矗閲囬泦锛堝浐瀹氳礋杞斤級骞堕€氳繃 UDP 涓婃姤銆?
- 浣跨敤 Rust 缂栧啓锛岄€傞厤 Linux/WSL 鍦烘櫙銆?

### 1. 鐜瑕佹眰

- Rust 宸ュ叿閾撅紙寤鸿 stable锛孍dition 2021锛?
- Cargo锛堥殢 Rust 涓€璧峰畨瑁咃級
- Bash锛堢敤浜庤繍琛岃剼鏈紱Windows 鍙敤 Git Bash / WSL锛?

### 2. 鏈湴杩愯 cloud 鎺ユ敹绔?

```bash
cd cloud
cargo run -- --config config/sensors.toml --bind ${CLOUD_BIND_ADDR:-0.0.0.0:9000} --timeout-ms 0
```
*(娉細鍙€氳繃閰嶇疆 `CLOUD_BIND_ADDR` 鐜鍙橀噺鎴?`sensors.toml` 鏇存敼绔彛涓嶪P銆傜綉鍏充晶鍙婁覆鍙ｆ尝鐗圭巼閰嶇疆璇﹁缃戝叧瀛愪粨鏂囨。銆?*

### 3. 鏈湴閰嶇疆鍐掔儫娴嬭瘯锛堣剼鏈級

```bash
cd cloud
chmod +x scripts/local_config_smoke_test.sh
./scripts/local_config_smoke_test.sh
```

### 4. 杩愯鑷姩鍖栨祴璇?

```bash
cd cloud
cargo test
```

## 鎶€鏈爤涓庝緷璧栨竻鍗?

### 璇█涓庤繍琛屾椂

- Rust锛圗dition 2021锛?

### 涓昏渚濊禆锛坄cloud/Cargo.toml`锛?

- `serde`锛堝惈 `derive`锛?
- `toml`

### 閮ㄧ讲鐩稿叧

- `cloud/deploy.sh`锛歀inux 鏈嶅姟鍣ㄤ竴閿儴缃茶剼鏈?
- systemd锛堝彲閫夛紝鑴氭湰浼氫紭鍏堜娇鐢級

## 鐩綍缁撴瀯锛堟牳蹇冿級

- `cloud/`锛氫簯绔?UDP 鎺ユ敹鍣紙Rust锛?
- `cloud/config/sensors.toml`锛氫紶鎰熷櫒瑙勫垯閰嶇疆
- `cloud/scripts/local_config_smoke_test.sh`锛氭湰鍦拌剼鏈?smoke test
- `cloud/tests/`锛氶泦鎴愭祴璇曠洰褰?
- `doc/`锛氬崗浣滀笌娴佺▼鏂囨。

## 娴嬭瘯璇存槑

- 鍗曞厓娴嬭瘯锛氫綅浜?`cloud/src/main.rs` 涓?
- 闆嗘垚 smoke 娴嬭瘯锛氫綅浜?`cloud/tests/smoke_e2e.rs`

## AI 妯″潡琛ュ厖璇存槑

- `service/`锛氭湰鍦版ā鍨嬪姞杞戒笌鍗曞紶鍥剧墖鎺ㄧ悊閫昏緫锛岃瑙?`service/README.md`
- `scripts/`锛氭暟鎹泦鏁寸悊銆佽缁冪瓑鑴氭湰锛岃瑙?`scripts/README.md`
- `tests/`锛欰I 妯″潡鐨?`pytest` 娴嬭瘯浠ｇ爜
- `models/`锛氬垎绫绘ā鍨嬬殑閰嶇疆鏂囦欢銆佹爣绛炬枃浠朵笌璇存槑
- `local_data/`锛氭湰鍦版暟鎹鏄庝笌鏁版嵁闆嗙洰褰?
- `outputs/`锛氳缁冭緭鍑轰笌妯″瀷浜х墿鐩綍

## Python 渚濊禆涓庢祴璇曡ˉ鍏?

濡傞渶杩愯 AI 鐩稿叧鑴氭湰鎴栨祴璇曪紝鍙湪浠撳簱鏍圭洰褰曟墽琛岋細

```bash
pip install -r requirements.txt
pytest -q
```

琛ュ厖璇存槑锛?

- 鏍圭洰褰曞凡鎻愪緵 `pytest.ini`锛屽彲鐩存帴鍦ㄤ粨搴撴牴鐩綍杩愯 `pytest`锛屾棤闇€棰濆璁剧疆 `PYTHONPATH`
- 濡備粎闇€杩愯 AI 妯″潡娴嬭瘯锛屽彲浣跨敤 `pytest -q tests`
- `tests/test_infer_smoke.py` 渚濊禆 `torch` 涓?`torchvision`

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

鏈」鐩娇鐢?MIT License锛岃瑙佹牴鐩綍 `LICENSE`銆?
