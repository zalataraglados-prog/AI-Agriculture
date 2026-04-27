import kagglehub
hadiurrahmannabil_rice_leaf_spot_disease_annotated_dataset_path = kagglehub.dataset_download('hadiurrahmannabil/rice-leaf-spot-disease-annotated-dataset')

print('Data source import complete.')

import pandas as pd # data processing, CSV file I/O (e.g. pd.read_csv)
import os
import json
import time
import random
from pathlib import Path
from collections import Counter

import numpy as np
import matplotlib.pyplot as plt
from PIL import Image

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import Dataset, DataLoader

import torchvision
from torchvision import transforms

from tqdm import tqdm

from sklearn.model_selection import train_test_split
from sklearn.metrics import (
    accuracy_score,
    f1_score,
    confusion_matrix,
    classification_report,
    average_precision_score
)
from sklearn.preprocessing import label_binarize


# =========================
# Global config
# =========================
SEED = 42
DEVICE = torch.device("cuda" if torch.cuda.is_available() else "cpu")

# Kaggle-specific output dir
SAVE_DIR = "/kaggle/working/rice_leaf_classifier"
os.makedirs(SAVE_DIR, exist_ok=True)

CLASS_NAMES = [
    "Bacterial_Leaf_Blight",
    "Brown_Spot",
    "HealthyLeaf",
    "Leaf_Blast",
    "Leaf_Scald",
    "Narrow_Brown_Leaf_Spot",
    "Neck_Blast",
    "Rice_Hispa"
]
NUM_CLASSES = len(CLASS_NAMES)
HEALTHY_CLASS_NAME = "HealthyLeaf"
HEALTHY_CLASS_IDX = CLASS_NAMES.index(HEALTHY_CLASS_NAME)

IMG_SIZE = 224
BATCH_SIZE = 16
NUM_EPOCHS = 15
LEARNING_RATE = 3e-4
WEIGHT_DECAY = 1e-4
VAL_RATIO = 0.2
NUM_WORKERS = 0

# Safer default on Kaggle: no external weight download required
USE_PRETRAINED = False

LABEL_SMOOTHING = 0.05
EARLY_STOPPING_PATIENCE = 5
MAX_GRAD_NORM = 1.0
USE_AMP = torch.cuda.is_available()

BEST_MODEL_PATH = os.path.join(SAVE_DIR, "best_rice_leaf_classifier.pth")
FINAL_MODEL_PATH = os.path.join(SAVE_DIR, "final_rice_leaf_classifier.pth")
HISTORY_PATH = os.path.join(SAVE_DIR, "training_history.json")
METRICS_PATH = os.path.join(SAVE_DIR, "final_metrics.json")

print("Device:", DEVICE)
print("Save dir:", SAVE_DIR)

import kagglehub

# Download latest version
path = kagglehub.dataset_download("hadiurrahmannabil/rice-leaf-spot-disease-annotated-dataset")

print("Path to dataset files:", path)

def set_seed(seed=42):
    random.seed(seed)
    np.random.seed(seed)
    torch.manual_seed(seed)
    torch.cuda.manual_seed_all(seed)

    torch.backends.cudnn.deterministic = True
    torch.backends.cudnn.benchmark = False


set_seed(SEED)


def find_dataset_root():
    """
    Auto-discover dataset root under /kaggle/input.
    Looks for a directory that contains:
      train/images
      train/labels
    """
    input_root = Path("/kaggle/input")
    if not input_root.exists():
        raise FileNotFoundError("/kaggle/input not found. Are you running on Kaggle?")

    candidates = []
    for p in input_root.rglob("*"):
        if p.is_dir():
            images_dir = p / "train" / "images"
            labels_dir = p / "train" / "labels"
            if images_dir.exists() and labels_dir.exists():
                candidates.append(p)

    if len(candidates) == 0:
        raise FileNotFoundError(
            "No dataset root found under /kaggle/input that contains train/images and train/labels.\n"
            "Please click Add Input and attach the rice leaf dataset first."
        )

    # Prefer exact folder name if present
    preferred = [p for p in candidates if p.name == "RiceLeafAnnotatedDataset"]
    if len(preferred) > 0:
        return str(preferred[0])

    return str(candidates[0])


DATASET_PATH = find_dataset_root()
print("Detected dataset path:", DATASET_PATH)

print("\nTop-level /kaggle/input folders:")
for p in Path("/kaggle/input").iterdir():
    print("-", p)

def collect_classification_samples(dataset_root, class_names):
    """
    Convert detection-style annotations into image-level classification labels.

    Rules:
    1. Empty txt -> HealthyLeaf
    2. One or more boxes but same class -> that class
    3. Multiple classes in one image -> use majority class
    """
    dataset_root = Path(dataset_root)
    images_dir = dataset_root / "train" / "images"
    labels_dir = dataset_root / "train" / "labels"

    assert images_dir.exists(), f"Images directory not found: {images_dir}"
    assert labels_dir.exists(), f"Labels directory not found: {labels_dir}"

    image_paths = []
    for ext in ["*.jpg", "*.jpeg", "*.png", "*.bmp", "*.webp"]:
        image_paths.extend(images_dir.glob(ext))
    image_paths = sorted(image_paths)

    samples = []
    stats = {
        "total_images_found": 0,
        "missing_label_files": 0,
        "empty_label_as_healthy": 0,
        "multi_class_images": 0,
        "invalid_label_lines": 0,
        "kept_samples": 0,
    }

    for img_path in image_paths:
        stats["total_images_found"] += 1
        label_path = labels_dir / f"{img_path.stem}.txt"

        if not label_path.exists():
            stats["missing_label_files"] += 1
            continue

        with open(label_path, "r", encoding="utf-8") as f:
            lines = [line.strip() for line in f.readlines() if line.strip()]

        if len(lines) == 0:
            label_idx = HEALTHY_CLASS_IDX
            stats["empty_label_as_healthy"] += 1
        else:
            label_ids = []
            for line in lines:
                parts = line.split()
                if len(parts) < 5:
                    stats["invalid_label_lines"] += 1
                    continue

                try:
                    cls_id = int(parts[0])
                except:
                    stats["invalid_label_lines"] += 1
                    continue

                if cls_id < 0 or cls_id >= len(class_names):
                    stats["invalid_label_lines"] += 1
                    continue

                label_ids.append(cls_id)

            if len(label_ids) == 0:
                continue

            if len(set(label_ids)) > 1:
                stats["multi_class_images"] += 1

            label_idx = Counter(label_ids).most_common(1)[0][0]

        samples.append({
            "image_path": str(img_path),
            "label": label_idx
        })

    stats["kept_samples"] = len(samples)
    return samples, stats


def plot_class_distribution(samples, class_names, title="Class Distribution"):
    labels = [s["label"] for s in samples]
    counts = Counter(labels)
    values = [counts.get(i, 0) for i in range(len(class_names))]

    plt.figure(figsize=(12, 5))
    plt.bar(class_names, values)
    plt.xticks(rotation=45, ha="right")
    plt.title(title)
    plt.xlabel("Class")
    plt.ylabel("Number of Images")
    plt.tight_layout()
    plt.show()


samples, sample_stats = collect_classification_samples(DATASET_PATH, CLASS_NAMES)

print("Sample collection summary:")
for k, v in sample_stats.items():
    print(f"{k}: {v}")

assert len(samples) > 0, "No usable samples found."

plot_class_distribution(samples, CLASS_NAMES, title="All Samples Distribution")

def split_samples(samples, val_ratio=0.2, seed=42):
    labels = [s["label"] for s in samples]
    indices = np.arange(len(samples))

    try:
        train_idx, val_idx = train_test_split(
            indices,
            test_size=val_ratio,
            random_state=seed,
            stratify=labels
        )
    except ValueError:
        print("Stratified split failed. Falling back to random split.")
        train_idx, val_idx = train_test_split(
            indices,
            test_size=val_ratio,
            random_state=seed,
            shuffle=True
        )

    train_samples = [samples[i] for i in train_idx]
    val_samples = [samples[i] for i in val_idx]
    return train_samples, val_samples


train_samples, val_samples = split_samples(samples, val_ratio=VAL_RATIO, seed=SEED)

print(f"Train samples: {len(train_samples)}")
print(f"Val samples:   {len(val_samples)}")

plot_class_distribution(train_samples, CLASS_NAMES, title="Train Distribution")
plot_class_distribution(val_samples, CLASS_NAMES, title="Validation Distribution")


class RiceLeafClassificationDataset(Dataset):
    def __init__(self, samples, transform=None):
        self.samples = samples
        self.transform = transform

    def __len__(self):
        return len(self.samples)

    def __getitem__(self, idx):
        sample = self.samples[idx]
        img = Image.open(sample["image_path"]).convert("RGB")
        label = sample["label"]

        if self.transform is not None:
            img = self.transform(img)

        return img, label


train_transform = transforms.Compose([
    transforms.Resize((256, 256)),
    transforms.RandomResizedCrop(IMG_SIZE, scale=(0.8, 1.0)),
    transforms.RandomHorizontalFlip(p=0.5),
    transforms.RandomRotation(degrees=15),
    transforms.ColorJitter(brightness=0.2, contrast=0.2, saturation=0.2, hue=0.05),
    transforms.ToTensor(),
    transforms.Normalize(mean=[0.485, 0.456, 0.406],
                         std=[0.229, 0.224, 0.225]),
])

val_transform = transforms.Compose([
    transforms.Resize((256, 256)),
    transforms.CenterCrop(IMG_SIZE),
    transforms.ToTensor(),
    transforms.Normalize(mean=[0.485, 0.456, 0.406],
                         std=[0.229, 0.224, 0.225]),
])


train_dataset = RiceLeafClassificationDataset(train_samples, transform=train_transform)
val_dataset = RiceLeafClassificationDataset(val_samples, transform=val_transform)

train_loader = DataLoader(
    train_dataset,
    batch_size=BATCH_SIZE,
    shuffle=True,
    num_workers=NUM_WORKERS,
    pin_memory=torch.cuda.is_available(),
    persistent_workers=False
)

val_loader = DataLoader(
    val_dataset,
    batch_size=BATCH_SIZE,
    shuffle=False,
    num_workers=NUM_WORKERS,
    pin_memory=torch.cuda.is_available(),
    persistent_workers=False
)

print("Train loader batches:", len(train_loader))
print("Val loader batches:", len(val_loader))

def denormalize(img_tensor):
    mean = torch.tensor([0.485, 0.456, 0.406]).view(3, 1, 1)
    std = torch.tensor([0.229, 0.224, 0.225]).view(3, 1, 1)
    img = img_tensor.cpu() * std + mean
    img = torch.clamp(img, 0, 1)
    return img


def show_batch(dataset, class_names, num_images=9):
    plt.figure(figsize=(12, 12))
    indices = np.random.choice(len(dataset), size=min(num_images, len(dataset)), replace=False)

    for i, idx in enumerate(indices, 1):
        img, label = dataset[idx]
        img = denormalize(img).permute(1, 2, 0).numpy()

        plt.subplot(3, 3, i)
        plt.imshow(img)
        plt.title(class_names[label])
        plt.axis("off")

    plt.tight_layout()
    plt.show()


show_batch(train_dataset, CLASS_NAMES, num_images=9)

if USE_PRETRAINED:
    weights = torchvision.models.ResNet18_Weights.DEFAULT
else:
    weights = None

model = torchvision.models.resnet18(weights=weights)
model.fc = nn.Sequential(
    nn.Dropout(p=0.3),
    nn.Linear(model.fc.in_features, NUM_CLASSES)
)
model = model.to(DEVICE)

train_labels = [s["label"] for s in train_samples]
class_counts = np.bincount(train_labels, minlength=NUM_CLASSES)
class_weights = len(train_labels) / (NUM_CLASSES * np.maximum(class_counts, 1))
class_weights = torch.tensor(class_weights, dtype=torch.float32, device=DEVICE)

criterion = nn.CrossEntropyLoss(
    weight=class_weights,
    label_smoothing=LABEL_SMOOTHING
)

optimizer = optim.AdamW(
    model.parameters(),
    lr=LEARNING_RATE,
    weight_decay=WEIGHT_DECAY
)

scheduler = optim.lr_scheduler.ReduceLROnPlateau(
    optimizer,
    mode="min",
    factor=0.5,
    patience=2
)

AMP_DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
scaler = torch.amp.GradScaler(AMP_DEVICE, enabled=USE_AMP)

print("Model created: ResNet18 classifier")
print("Class weights:", class_weights.detach().cpu().numpy())

def compute_multiclass_map(y_true, y_prob, num_classes):
    y_true_bin = label_binarize(y_true, classes=list(range(num_classes)))
    ap_per_class = []

    for c in range(num_classes):
        if y_true_bin[:, c].sum() == 0:
            ap_per_class.append(np.nan)
            continue

        ap = average_precision_score(y_true_bin[:, c], y_prob[:, c])
        ap_per_class.append(ap)

    valid_aps = [x for x in ap_per_class if not np.isnan(x)]
    mean_ap = float(np.mean(valid_aps)) if len(valid_aps) > 0 else float("nan")
    return mean_ap, ap_per_class


def run_one_epoch(model, loader, criterion, optimizer=None, scaler=None, device="cpu"):
    is_train = optimizer is not None

    if is_train:
        model.train()
    else:
        model.eval()

    running_loss = 0.0
    all_labels = []
    all_preds = []
    all_probs = []

    pbar = tqdm(loader, leave=False, desc="Train" if is_train else "Validate")

    for images, labels in pbar:
        images = images.to(device, non_blocking=True)
        labels = labels.to(device, non_blocking=True)

        if is_train:
            optimizer.zero_grad(set_to_none=True)

        with torch.set_grad_enabled(is_train):
            with torch.amp.autocast(AMP_DEVICE, enabled=USE_AMP):
                logits = model(images)
                loss = criterion(logits, labels)

            if is_train:
                scaler.scale(loss).backward()
                scaler.unscale_(optimizer)
                nn.utils.clip_grad_norm_(model.parameters(), MAX_GRAD_NORM)
                scaler.step(optimizer)
                scaler.update()

        probs = torch.softmax(logits, dim=1)
        preds = torch.argmax(probs, dim=1)

        running_loss += loss.item() * images.size(0)
        all_labels.extend(labels.detach().cpu().numpy().tolist())
        all_preds.extend(preds.detach().cpu().numpy().tolist())
        all_probs.append(probs.detach().cpu().numpy())

        current_acc = accuracy_score(all_labels, all_preds)
        pbar.set_postfix(loss=f"{loss.item():.4f}", acc=f"{current_acc:.4f}")

    epoch_loss = running_loss / len(loader.dataset)
    all_probs = np.concatenate(all_probs, axis=0)

    epoch_acc = accuracy_score(all_labels, all_preds)
    epoch_f1 = f1_score(all_labels, all_preds, average="macro")
    epoch_map, ap_per_class = compute_multiclass_map(all_labels, all_probs, NUM_CLASSES)

    metrics = {
        "loss": epoch_loss,
        "acc": epoch_acc,
        "f1": epoch_f1,
        "map": epoch_map,
        "ap_per_class": ap_per_class,
        "labels": all_labels,
        "preds": all_preds,
        "probs": all_probs
    }
    return metrics


def train_model(model, train_loader, val_loader, criterion, optimizer, scheduler, scaler, num_epochs, device):
    history = {
        "train_loss": [],
        "val_loss": [],
        "train_acc": [],
        "val_acc": [],
        "train_f1": [],
        "val_f1": [],
        "train_map": [],
        "val_map": [],
        "lr": []
    }

    best_val_f1 = -1.0
    best_epoch = -1
    patience_counter = 0

    for epoch in range(num_epochs):
        print(f"\nEpoch {epoch+1}/{num_epochs}")

        train_metrics = run_one_epoch(
            model=model,
            loader=train_loader,
            criterion=criterion,
            optimizer=optimizer,
            scaler=scaler,
            device=device
        )

        val_metrics = run_one_epoch(
            model=model,
            loader=val_loader,
            criterion=criterion,
            optimizer=None,
            scaler=None,
            device=device
        )

        scheduler.step(val_metrics["loss"])
        current_lr = optimizer.param_groups[0]["lr"]

        history["train_loss"].append(train_metrics["loss"])
        history["val_loss"].append(val_metrics["loss"])
        history["train_acc"].append(train_metrics["acc"])
        history["val_acc"].append(val_metrics["acc"])
        history["train_f1"].append(train_metrics["f1"])
        history["val_f1"].append(val_metrics["f1"])
        history["train_map"].append(train_metrics["map"])
        history["val_map"].append(val_metrics["map"])
        history["lr"].append(current_lr)

        print(
            f"Train Loss: {train_metrics['loss']:.4f} | "
            f"Train Acc: {train_metrics['acc']:.4f} | "
            f"Train F1: {train_metrics['f1']:.4f} | "
            f"Train mAP: {train_metrics['map']:.4f}"
        )
        print(
            f"Val   Loss: {val_metrics['loss']:.4f} | "
            f"Val   Acc: {val_metrics['acc']:.4f} | "
            f"Val   F1: {val_metrics['f1']:.4f} | "
            f"Val   mAP: {val_metrics['map']:.4f} | "
            f"LR: {current_lr:.6f}"
        )

        improved = val_metrics["f1"] > best_val_f1

        if improved:
            best_val_f1 = val_metrics["f1"]
            best_epoch = epoch + 1
            patience_counter = 0

            checkpoint = {
                "epoch": best_epoch,
                "model_state_dict": model.state_dict(),
                "optimizer_state_dict": optimizer.state_dict(),
                "class_names": CLASS_NAMES,
                "best_val_f1": best_val_f1,
                "config": {
                    "img_size": IMG_SIZE,
                    "batch_size": BATCH_SIZE,
                    "num_epochs": NUM_EPOCHS,
                    "learning_rate": LEARNING_RATE,
                    "weight_decay": WEIGHT_DECAY,
                    "use_pretrained": USE_PRETRAINED
                }
            }
            torch.save(checkpoint, BEST_MODEL_PATH)
            print(f"Best model saved to: {BEST_MODEL_PATH}")
        else:
            patience_counter += 1

        if patience_counter >= EARLY_STOPPING_PATIENCE:
            print("Early stopping triggered.")
            break

    print(f"\nBest epoch: {best_epoch}")
    print(f"Best val F1: {best_val_f1:.4f}")

    best_checkpoint = torch.load(BEST_MODEL_PATH, map_location=device)
    model.load_state_dict(best_checkpoint["model_state_dict"])

    return model, history

start_time = time.time()

model, history = train_model(
    model=model,
    train_loader=train_loader,
    val_loader=val_loader,
    criterion=criterion,
    optimizer=optimizer,
    scheduler=scheduler,
    scaler=scaler,
    num_epochs=NUM_EPOCHS,
    device=DEVICE
)

torch.save(model.state_dict(), FINAL_MODEL_PATH)

elapsed = time.time() - start_time
print(f"\nTraining finished in {elapsed/60:.2f} minutes")
print("Best model path:", BEST_MODEL_PATH)
print("Final model path:", FINAL_MODEL_PATH)

epochs_range = range(1, len(history["train_loss"]) + 1)

plt.figure(figsize=(8, 5))
plt.plot(epochs_range, history["train_loss"], label="Train Loss")
plt.plot(epochs_range, history["val_loss"], label="Val Loss")
plt.xlabel("Epoch")
plt.ylabel("Loss")
plt.title("Loss Curve")
plt.legend()
plt.grid(True)
plt.show()

plt.figure(figsize=(8, 5))
plt.plot(epochs_range, history["train_acc"], label="Train Accuracy")
plt.plot(epochs_range, history["val_acc"], label="Val Accuracy")
plt.xlabel("Epoch")
plt.ylabel("Accuracy")
plt.title("Accuracy Curve")
plt.legend()
plt.grid(True)
plt.show()

plt.figure(figsize=(8, 5))
plt.plot(epochs_range, history["train_f1"], label="Train Macro-F1")
plt.plot(epochs_range, history["val_f1"], label="Val Macro-F1")
plt.xlabel("Epoch")
plt.ylabel("Macro-F1")
plt.title("Macro-F1 Curve")
plt.legend()
plt.grid(True)
plt.show()

plt.figure(figsize=(8, 5))
plt.plot(epochs_range, history["train_map"], label="Train mAP")
plt.plot(epochs_range, history["val_map"], label="Val mAP")
plt.xlabel("Epoch")
plt.ylabel("mAP")
plt.title("One-vs-Rest mAP Curve")
plt.legend()
plt.grid(True)
plt.show()

plt.figure(figsize=(8, 5))
plt.plot(epochs_range, history["lr"], label="Learning Rate")
plt.xlabel("Epoch")
plt.ylabel("LR")
plt.title("Learning Rate Schedule")
plt.legend()
plt.grid(True)
plt.show()

final_val_metrics = run_one_epoch(
    model=model,
    loader=val_loader,
    criterion=criterion,
    optimizer=None,
    scaler=None,
    device=DEVICE
)

print("Final validation metrics:")
print(f"Loss: {final_val_metrics['loss']:.4f}")
print(f"Accuracy: {final_val_metrics['acc']:.4f}")
print(f"Macro-F1: {final_val_metrics['f1']:.4f}")
print(f"mAP: {final_val_metrics['map']:.4f}")

print("\nPer-class AP:")
for cls_name, ap in zip(CLASS_NAMES, final_val_metrics["ap_per_class"]):
    if np.isnan(ap):
        print(f"{cls_name}: NaN")
    else:
        print(f"{cls_name}: {ap:.4f}")

cm = confusion_matrix(final_val_metrics["labels"], final_val_metrics["preds"])

plt.figure(figsize=(8, 6))
plt.imshow(cm, interpolation="nearest", cmap="Blues")
plt.title("Validation Confusion Matrix")
plt.colorbar()
tick_marks = np.arange(NUM_CLASSES)
plt.xticks(tick_marks, CLASS_NAMES, rotation=45, ha="right")
plt.yticks(tick_marks, CLASS_NAMES)
plt.xlabel("Predicted Label")
plt.ylabel("True Label")

for i in range(cm.shape[0]):
    for j in range(cm.shape[1]):
        plt.text(j, i, str(cm[i, j]), ha="center", va="center")

plt.tight_layout()
plt.show()

report = classification_report(
    final_val_metrics["labels"],
    final_val_metrics["preds"],
    target_names=CLASS_NAMES,
    digits=4,
    output_dict=False
)

print("\nClassification Report:")
print(report)

history_to_save = {
    k: [float(x) for x in v] for k, v in history.items()
}

with open(HISTORY_PATH, "w", encoding="utf-8") as f:
    json.dump(history_to_save, f, indent=2)

metrics_to_save = {
    "loss": float(final_val_metrics["loss"]),
    "accuracy": float(final_val_metrics["acc"]),
    "macro_f1": float(final_val_metrics["f1"]),
    "map": float(final_val_metrics["map"]),
    "ap_per_class": {
        CLASS_NAMES[i]: (None if np.isnan(v) else float(v))
        for i, v in enumerate(final_val_metrics["ap_per_class"])
    }
}

with open(METRICS_PATH, "w", encoding="utf-8") as f:
    json.dump(metrics_to_save, f, indent=2)

print("Saved files:")
print("-", BEST_MODEL_PATH)
print("-", FINAL_MODEL_PATH)
print("-", HISTORY_PATH)
print("-", METRICS_PATH)

def load_local_image(image_path):
    return Image.open(image_path).convert("RGB")


def predict_image_class(image_path, model, transform, class_names, device, top_k=3):
    img = load_local_image(image_path)
    input_tensor = transform(img).unsqueeze(0).to(device)

    model.eval()
    with torch.no_grad():
        logits = model(input_tensor)
        probs = torch.softmax(logits, dim=1)[0].cpu().numpy()

    top_indices = np.argsort(probs)[::-1][:top_k]

    plt.figure(figsize=(6, 6))
    plt.imshow(img)
    plt.axis("off")
    plt.title(f"Prediction: {class_names[top_indices[0]]} ({probs[top_indices[0]]:.2%})")
    plt.show()

    print("Top predictions:")
    for rank, idx in enumerate(top_indices, 1):
        print(f"{rank}. {class_names[idx]} - {probs[idx]:.4f}")


example_image = val_samples[0]["image_path"]
print("Example image:", example_image)

predict_image_class(
    image_path=example_image,
    model=model,
    transform=val_transform,
    class_names=CLASS_NAMES,
    device=DEVICE,
    top_k=3
)

!find /kaggle/working -maxdepth 2 -type f | sort
