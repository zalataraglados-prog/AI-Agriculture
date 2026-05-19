import os
import shutil
from pathlib import Path

class RoboflowYoloImporter:
    def __init__(self):
        # 定义原材料(raw)和加工后(yolo)的路径
        # 根据你的实际路径进行对应
        self.raw_dir = Path("datasets/oil_palm/ffb_maturity/raw/roboflow_v1")
        self.output_dir = Path("datasets/oil_palm/ffb_maturity/yolo")
        
        # 核心翻译本：{ Roboflow的旧标签: 我们规定的新标签 }
        self.label_map = {
            "0": "4",  # overripe
            "1": "3",  # ripe
            "2": "2",  # underripe
            "3": "1"   # unripe
        }

    def convert(self):
        print("🚀 开始转换 FFB 数据集标签...")
        
        # 确保输出目录干净存在
        if self.output_dir.exists():
            shutil.rmtree(self.output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

        # 遍历 train, valid, test (如果有的话)
        for split in ['train', 'valid', 'test']:
            split_dir = self.raw_dir / split
            if not split_dir.exists():
                continue
                
            print(f"📦 正在处理 {split} 集...")
            
            # 在 yolo 目录下建立对应的结构
            out_split_images = self.output_dir / split / 'images'
            out_split_labels = self.output_dir / split / 'labels'
            out_split_images.mkdir(parents=True, exist_ok=True)
            out_split_labels.mkdir(parents=True, exist_ok=True)

            # 1. 复制图片 (直接拷贝)
            raw_images_dir = split_dir / 'images'
            if raw_images_dir.exists():
                for img_file in raw_images_dir.iterdir():
                    shutil.copy(img_file, out_split_images / img_file.name)

            # 2. 转换标签 (重点！)
            raw_labels_dir = split_dir / 'labels'
            if raw_labels_dir.exists():
                for txt_file in raw_labels_dir.iterdir():
                    self._convert_label_file(txt_file, out_split_labels / txt_file.name)
                    
        # 3. 生成符合我们系统要求的新 data.yaml
        self._generate_yaml()
        print("✅ 恭喜！数据集转换完成，已全部输出到 yolo 目录！")

    def _convert_label_file(self, old_file_path, new_file_path):
        """逐行读取旧标签，把第一位数字替换成新数字"""
        with open(old_file_path, 'r') as f_old, open(new_file_path, 'w') as f_new:
            for line in f_old:
                parts = line.strip().split()
                if not parts:
                    continue
                old_class_id = parts[0]
                # 如果这个旧标签在我们的字典里，就替换它；否则丢弃（过滤掉脏数据）
                if old_class_id in self.label_map:
                    new_class_id = self.label_map[old_class_id]
                    # 把新标签和后面的坐标拼接起来，写入新文件
                    parts[0] = new_class_id
                    f_new.write(" ".join(parts) + "\n")

    def _generate_yaml(self):
        """生成最终训练要用的 YAML 文件"""
        yaml_content = """train: ./train/images
val: ./valid/images

nc: 6
names: ['flower', 'unripe', 'underripe', 'ripe', 'overripe', 'abnormal']
"""
        yaml_path = self.output_dir / 'data.yaml'
        with open(yaml_path, 'w', encoding='utf-8') as f:
            f.write(yaml_content)

# ===== 运行入口 =====
if __name__ == "__main__":
    importer = RoboflowYoloImporter()
    importer.convert()