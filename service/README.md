# Service Layer

This directory contains model loading and inference logic.

## Files

- model_loader.py  
Load trained model

- infer.py  
Run inference on a single image

## Example

```bash
python service/infer.py \
--image-path test.jpg \
--checkpoint-path outputs/.../best_model.pth
```

Output Format
```json

{
"predicted_class": "...",
"confidence": 0.93,
"topk": [...]
}
```
Notes
-----

* This is NOT yet a web service

* FastAPI integration will be added later