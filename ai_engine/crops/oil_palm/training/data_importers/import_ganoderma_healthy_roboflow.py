import shutil
import csv
import random
from pathlib import Path
from collections import defaultdict


class BaseImporter:
    def convert(self):
        raise NotImplementedError


class GanodermaHealthyImporter(BaseImporter):
    """
    Dataset 2: Healthy Oil Palm Images (Filenames starting with Screenshot/20250316)
    CSV Format: filename, healthy, unhealthy
    - healthy=1 => healthy
    - unhealthy=1 => suspected_risk (This dataset is mostly healthy=1, unhealthy=0)

    Original Structure:
        raw/ganoderma_healthy/train/*.jpg
        raw/ganoderma_healthy/train/classes.csv

    Output to:
        classification/train/healthy/   (Append mode, does not overwrite existing files)
        classification/val/healthy/
        classification/test/healthy/
    """

    STANDARD_LABELS = ["healthy", "suspected_risk", "other_stress_unknown"]
    SPLIT_RATIOS = {"train": 0.70, "val": 0.15, "test": 0.15}
    SEED = 42

    def __init__(self, raw_dir: str, output_dir: str):
        self.raw_dir = Path(raw_dir)
        self.output_dir = Path(output_dir)

    def _map_label(self, row: dict) -> str:
        healthy = int(row.get("healthy", 0))
        unhealthy = int(row.get("unhealthy", 0))
        if unhealthy == 1:
            return "suspected_risk"
        return "healthy"

    def _split_data(self, items: list) -> dict:
        random.seed(self.SEED)
        by_label = defaultdict(list)
        for filename, label in items:
            by_label[label].append(filename)

        result = {"train": [], "val": [], "test": []}
        for label, files in by_label.items():
            random.shuffle(files)
            n = len(files)
            n_train = int(n * self.SPLIT_RATIOS["train"])
            n_val = int(n * self.SPLIT_RATIOS["val"])
            
            for f in files[:n_train]:
                result["train"].append((f, label))
            for f in files[n_train:n_train + n_val]:
                result["val"].append((f, label))
            for f in files[n_train + n_val:]:
                result["test"].append((f, label))
        return result

    def convert(self):
        csv_path = self.raw_dir / "train" / "_classes.csv"
        img_dir = self.raw_dir / "train"

        if not csv_path.exists():
            raise FileNotFoundError(f"Cannot find classes.csv: {csv_path}")

        items = []
        with open(csv_path, newline="", encoding="utf-8") as f:
            reader = csv.DictReader(f)
            for row in reader:
                filename = row.get("filename", "").strip()
                if not filename:
                    continue
                if not (img_dir / filename).exists():
                    print(f"[WARNING] Image does not exist, skipping: {filename}")
                    continue
                label = self._map_label(row)
                items.append((filename, label))

        print(f"[Importer-Healthy] Total images read: {len(items)}")

        splits = self._split_data(items)
        for split, data in splits.items():
            print(f"[Importer-Healthy] {split}: {len(data)} images")

        label_counts = defaultdict(int)
        for split, data in splits.items():
            for label in self.STANDARD_LABELS:
                (self.output_dir / split / label).mkdir(parents=True, exist_ok=True)
            
            for filename, label in data:
                src = img_dir / filename
                dst = self.output_dir / split / label / filename
                
                # Add prefix if filename conflict exists to avoid overwriting
                if dst.exists():
                    dst = self.output_dir / split / label / f"healthy2_{filename}"
                
                shutil.copy2(src, dst)
                label_counts[f"{split}/{label}"] += 1

        print("\n[Importer-Healthy] Done! split/label distribution:")
        for key, count in sorted(label_counts.items()):
            print(f"  {key}: {count} images")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--raw_dir", default="datasets/oil_palm/ganoderma_risk/raw/ganoderma_healthy")
    parser.add_argument("--output_dir", default="datasets/oil_palm/ganoderma_risk/classification")
    args = parser.parse_args()
    
    GanodermaHealthyImporter(raw_dir=args.raw_dir, output_dir=args.output_dir).convert()