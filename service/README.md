# Service

该目录包含模型加载和推理逻辑。

## 文件

- model_loader.py  
加载训练好的模型

- infer.py  
对单张图片进行推理

## 模型来源

* 根据要求训练出来
* 可直接用已经训练好的示例模型：[Google Drive](https://drive.google.com/drive/folders/1o4xnzu5JXQk3l8xcMJAPr4Hta2LkkROZ?usp=sharing)

## 示例

```bash
python service/infer.py \
--image-path test.jpg \
--checkpoint-path outputs/.../best_model.pth
```

输出格式
```json

{
"predicted_class": "...",
"confidence": 0.93,
"topk": [...]
}
```
注意事项
-----

* 这还不是一个网络服务

* 后续将添加FastAPI集成