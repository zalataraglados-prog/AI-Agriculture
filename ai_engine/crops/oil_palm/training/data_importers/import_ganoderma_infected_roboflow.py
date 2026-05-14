import shutil
import csv
import random
from pathlib import Path
from collections import defaultdict


class BaseImporter:
    def convert(self):
        raise NotImplementedError


class GanodermaInfectedImporter(BaseImporter):
    """
    Dataset 1: Contains Ganoderma infected images (filenames starting with IMG-20241010)
    CSV Format: filename, Ganoderma, Ganoderma Fungus
    - Ganoderma=1 or Ganoderma Fungus=1 => suspected_risk
    - Both columns = 0 => healthy

    Original Structure:
        raw/ganoderma_infected/train/*.jpg
        raw/ganoderma_infected/train/classes.csv

    Output to:
        classification/train/suspected_risk/
        classification/train/healthy/
        classification/val/...
        classification/test/...
    """

    STANDARD_LABELS = ["healthy", "suspected_risk", "other_stress_unknown"]
    SPLIT_RATIOS = {"train": 0.70, "val": 0.15, "test": 0.15}
    SEED = 42

    def __init__(self, raw_dir: str, output_dir: str):
        self.raw_dir = Path(raw_dir)
        self.output_dir = Path(output_dir)

    def _map_label(self, row: dict) -> str:
        ganoderma = int(row.get("Ganoderma", 0))
        fungus = int(row.get("Ganoderma Fungus", 0))
        if ganoderma == 1 or fungus == 1:
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
            raise FileNotFoundError(f"Could not find classes.csv: {csv_path}")

        items = []
        with open(csv_path, newline="", encoding="utf-8") as f:
            reader = csv.DictReader(f)
            for row in reader:
                filename = row.get("filename", "").strip()
                if not filename:
                    continue
                if not (img_dir / filename).exists():
                    print(f"[WARNING] Image not found, skipping: {filename}")
                    continue
                label = self._map_label(row)
                items.append((filename, label))

        print(f"[Importer-Infected] Total images read: {len(items)}")

        splits = self._split_data(items)
        for split, data in splits.items():
            print(f"[Importer-Infected] {split}: {len(data)} images")

        label_counts = defaultdict(int)
        for split, data in splits.items():
            # Ensure directories for all standard labels exist
            for label in self.STANDARD_LABELS:
                (self.output_dir / split / label).mkdir(parents=True, exist_ok=True)
            
            for filename, label in data:
                src = img_dir / filename
                dst = self.output_dir / split / label / filename
                shutil.copy2(src, dst)
                label_counts[f"{split}/{label}"] += 1

        print("\n[Importer-Infected] Done! Distribution by split/label:")
        for key, count in sorted(label_counts.items()):
            print(f"  {key}: {count} images")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Import and split Ganoderma infected dataset.")
    parser.add_argument("--raw_dir", default="datasets/oil_palm/ganoderma_risk/raw/ganoderma_infected")
    parser.add_argument("--output_dir", default="datasets/oil_palm/ganoderma_risk/classification")
    args = parser.parse_args()
    
    GanodermaInfectedImporter(raw_dir=args.raw_dir, output_dir=args.output_dir).convert()