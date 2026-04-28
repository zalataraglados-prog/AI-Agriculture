# Service

鐠囥儳娲拌ぐ鏇炲瘶閸?AI 閹恒劎鎮婂Ο鈥虫健閻ㄥ嫬鐣弫鏉戞磽鐏炲倹鐏﹂弸鍕杽閻滆埇鈧?

## 閻╊喖缍嶇紒鎾寸€?

*   `api/`: **(L1) 閹恒儱褰涚仦?* 閳?FastAPI 鐠侯垳鏁辨稉?HTTP 閻樿埖鈧胶鐖滄径鍕倞閵嗗倷绮庨崑姘崇熅閻㈠崬鍨庨崣鎴礉娑撱儳顩﹂崠鍛儓閹恒劎鎮婇柅鏄忕帆閵?
    *   `v1/predict.py`: `POST /api/v1/predict` 閸?`GET /api/v1/health`閵?
*   `adapters/`: **(L2) 闁倿鍘ら崳銊ョ湴** 閳?婢跺嫮鎮婃径鏍劥鏉堟挸鍙嗛敍鍫熸瀮娴犳儼鐭惧鍕┾偓浣哥摟閼哄倹绁﹂敍澶涚礉鏉烆剙瀵叉稉?core 鐏炲倸褰查惄瀛樺复娴ｈ法鏁ら惃鍕敶闁劎绮ㄩ弸?(PIL.Image)閵?
*   `core/`: **(L3) 閺嶇绺鹃幒銊ф倞鐏?* 閳?缁?Python 缁犳纭堕柅鏄忕帆閵嗗倹澧嶉張澶嬆侀崹瀣埛閹佃儻鍤?`base_predictor.py` 娑擃厾娈?`BasePredictor`閵嗗倷寮楃粋浣割嚤閸?FastAPI閵?
*   `schemas/`: **婵傛垹瀹崇仦?* 閳?Pydantic 閺佺増宓佺紒鎾寸€€规矮绠熼妴鍌涘閺堝膩閸ф妫块惃鍕殶閹诡喕绱堕柅鎺戞綆娴犮儲顒濇稉鍝勫櫙閵?
*   `main.py`: FastAPI 鎼存梻鏁ら崗銉ュ經閵嗗倽绀嬬拹锝喣侀崹瀣暕閸旂姾娴囬妴涓哋RS 闁板秶鐤嗛崪灞藉弿鐏炩偓瀵倸鐖舵径鍕倞閵?
*   `infer.py`: 閺堫剙婀撮崡鏇炴禈閹恒劎鎮?CLI 瀹搞儱鍙块敍鍫滅瑝娓氭繆绂?FastAPI閿涘鈧?

## 閸氼垰濮╅張宥呭

```bash
# 瀵偓閸欐垶膩瀵骏绱欓懛顏勫З闁插秷娴囬敍?
uvicorn ai_engine.main:app --reload --host 0.0.0.0 --port 8000
```

### 閻滎垰顣ㄩ崣姗€鍣?

| 閸欐﹢鍣洪崥?| 鐠囧瓨妲?| 姒涙顓婚崐?|
|--------|------|--------|
| `MODEL_CHECKPOINT_PATH` | 濡€崇€烽弶鍐櫢閺傚洣娆㈢捄顖氱窞 | `models/rice/rice_leaf_classifier/best_model.pth` |
| `MODEL_LABELS_FILE` | 閺嶅洨顒烽弬鍥︽鐠侯垰绶?| `models/rice/rice_leaf_classifier/labels.json` |
| `MODEL_CONFIG_FILE` | 濡€崇€烽柊宥囩枂鐠侯垰绶?| `models/rice/rice_leaf_classifier/config.yaml` |

## API 缁旑垳鍋?

### POST /api/v1/predict

娑撳﹣绱舵稉鈧?JPEG/PNG 閸ュ墽澧栭敍宀冪箲閸ョ偟姊剧€瑰啿鍨庣猾鑽ょ波閺嬫嚎鈧?

```bash
curl -X POST http://localhost:8000/api/v1/predict \
  -F "file=@test_image.jpg"
```

**閹存劕濮涢崫宥呯安 (200)**:
```json
{
  "status": "success",
  "results": [
    {
      "predicted_class": "Leaf_Blast",
      "confidence": 0.93,
      "topk": [
        {"label": "Leaf_Blast", "score": 0.93},
        {"label": "Brown_Spot", "score": 0.05},
        {"label": "HealthyLeaf", "score": 0.02}
      ],
      "model_version": "rice_cls_v0.1.0",
      "metadata": {},
      "geometry": null
    }
  ],
  "metadata": {}
}
```

**闁挎瑨顕ら崫宥呯安 (422/500)**:
```json
{
  "status": "error",
  "message": "Cannot decode image from provided bytes"
}
```

### GET /api/v1/health

閸嬨儱鎮嶅Λ鈧弻銉礉閻劋绨?Docker/K8s 閹恒垽鎷￠妴?

```json
{
  "status": "ok",
  "service": "smart-farm-ai-engine",
  "version": "0.1.0",
  "model": {
    "model_name": "RiceLeafClassifier",
    "model_version": "rice_cls_v0.1.0",
    "architecture": "resnet18",
    "num_classes": "8"
  }
}
```

## 閺堫剙婀?CLI 閹恒劎鎮?

```bash
python service/infer.py \
  --image-path test.jpg \
  --checkpoint-path outputs/.../best_model.pth
```