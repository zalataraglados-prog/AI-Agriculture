# PR 鏂囨妯℃澘

## 閫氱敤妯℃澘锛堝彲鐩存帴澶嶅埗锛?

```md
## 鑳屾櫙 / Background
涓轰簡瑙ｅ喅 ______ 闂锛屾湰 PR 灏?______ 浠庘€淿_____鈥濆崌绾т负鈥淿_____鈥濄€?

## 鍙樻洿鎽樿 / Change Summary
1. 鏂板 / 璋冩暣 ______
2. 鏀寔 ______
3. 琛ュ厖 ______

## 娴嬭瘯璇存槑 / Testing
### 鏈湴楠岃瘉鍛戒护 / Local Verification Commands
- `cargo test`

### 缁撴灉鎽樿 / Result Summary
- [x] 鏈湴娴嬭瘯閫氳繃
- [x] 鍏抽敭璺緞鎵嬪伐楠岃瘉閫氳繃

## 閰嶇疆涓庨儴缃插彉鏇?/ Config & Deploy Changes
- 鏂板閰嶇疆鏂囦欢锛歚...`
- 鏂板鐜鍙橀噺锛歚...`
- 閮ㄧ讲鑴氭湰鍙樻洿锛歚...`

## 椋庨櫓涓庡洖婊?/ Risks & Rollback
- 椋庨櫓锛歘_____
- 鍥炴粴锛氬洖婊氬埌鎻愪氦 `xxxxxxx` 骞堕噸鍚湇鍔?

## 鍏宠仈 Issue / Related Issue
Closes #___
```

## 褰撳墠椤圭洰绀轰緥锛圕loud 閰嶇疆鍖栨帴鏀讹級

```md
## 鑳屾櫙 / Background
浜戠鎺ユ敹绋嬪簭鍘熷厛涓昏閽堝鍥哄畾 payload锛堝 success锛夊仛鍖归厤锛屼笉鍒╀簬鍚庣画鎺ュ叆澶氫紶鎰熷櫒銆?
鏈?PR 灏嗕簯绔帴鏀跺崌绾т负鈥滈厤缃┍鍔ㄨ鍒欏尮閰嶁€濓紝鏂板浼犳劅鍣ㄦ椂鍙渶鏀归厤缃紝涓嶉渶瑕佹敼 Rust 浠ｇ爜銆?

## 鍙樻洿鎽樿 / Change Summary
1. 鏂板 TOML 閰嶇疆鍔犺浇鑳藉姏锛屾敮鎸?exact payload 涓?sensor rule 涓ょ被瑙勫垯銆?
2. 鏂板瀛楁鏍￠獙鑳藉姏锛坮equired_fields + field_types锛夈€?
3. ACK 璺敱閰嶇疆鍖栵細match / mismatch / unknown sensor銆?
4. 鏇存柊 deploy 鑴氭湰锛岄儴缃叉椂鍚屾 `config/sensors.toml` 骞朵互 `--config` 鍚姩銆?
5. 琛ュ厖鏈湴 smoke test 鑴氭湰涓?README 鏂囨。銆?

## 娴嬭瘯璇存槑 / Testing
### 鏈湴楠岃瘉鍛戒护 / Local Verification Commands
- `cargo fmt`
- `cargo test`
- `./scripts/local_config_smoke_test.sh`

### 缁撴灉鎽樿 / Result Summary
- `success -> ack:success`
- `mq7:raw=206,voltage=0.166 -> ack:mq7`
- `mq7:raw=oops,voltage=0.166 -> ack:error`

## 閰嶇疆涓庨儴缃插彉鏇?/ Config & Deploy Changes
- 鏂板锛歚cloud/config/sensors.toml`
- 鏂板锛歚CONFIG_PATH` 鐜鍙橀噺锛堥粯璁?`${INSTALL_ROOT}/config/sensors.toml`锛?
- 鍙樻洿锛歚deploy.sh` 浣跨敤 `--config` 鏂瑰紡鍚姩 receiver

## 椋庨櫓涓庡洖婊?/ Risks & Rollback
- 椋庨櫓锛氶厤缃啓閿欎細瀵艰嚧鍖归厤澶辫触鎴?ACK 寮傚父銆?
- 鍥炴粴锛氬洖婊氭湰 PR 鎻愪氦骞堕噸鍚?`ai-agri-cloud-receiver` 鏈嶅姟銆?
```
