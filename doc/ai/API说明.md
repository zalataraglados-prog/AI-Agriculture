# AI API 说明

## 当前接口
- POST /api/v1/ai/infer

## 输入
- task_id
- image_url
- crop_type
- source
- captured_at

## 输出
- status
- model_version
- predicted_class
- confidence
- topk
- advice_code
- latency_ms
