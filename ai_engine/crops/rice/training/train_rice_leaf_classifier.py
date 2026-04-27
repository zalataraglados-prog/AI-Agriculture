import argparse
import json
import random
import time
import warnings
import yaml
from pathlib import Path
from typing import List, Dict

import numpy as np
from PIL import Image
from sklearn.metrics import accuracy_score, f1_score
import torch
import torch.nn as nn
import torch.optim as optim
import torchvision
from torch.utils.data import Dataset, DataLoader
from torchvision import transforms
from tqdm import tqdm


def set_seed(seed: int = 42):
    random.seed(seed)
    np.random.seed(seed)
    torch.manual_seed(seed)
    torch.cuda.manual_seed_all(seed)

    torch.backends.cudnn.deterministic = True
    torch.backends.cudnn.benchmark = False


def load_json(path: str):
    with Path(path).open("r", encoding="utf-8") as f:
        return json.load(f)


def load_config(path: str):
    p = Path(path)
    if p.suffix == ".json":
        warnings.warn(f"Using .json config is deprecated. Please migrate to .yaml: {p}", DeprecationWarning)
        with p.open("r", encoding="utf-8") as f:
            return json.load(f)
    if p.suffix in (".yaml", ".yml"):
        if p.with_suffix(".json").exists():
            warnings.warn(f"Found legacy config file {p.with_suffix('.json')}. Please remove it to avoid confusion.", DeprecationWarning)
    with p.open("r", encoding="utf-8") as f:
        return yaml.safe_load(f)


class RiceLeafClassificationDataset(Dataset):
    def __init__(self, samples: List[Dict], transform=None):
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


def build_transforms(img_size: int):
    train_transform = transforms.Compose([
        transforms.Resize((256, 256)),
        transforms.RandomResizedCrop(img_size, scale=(0.8, 1.0)),
        transforms.RandomHorizontalFlip(p=0.5),
        transforms.RandomRotation(degrees=15),
        transforms.ColorJitter(
            brightness=0.2,
            contrast=0.2,
            saturation=0.2,
            hue=0.05
        ),
        transforms.ToTensor(),
        transforms.Normalize(
            mean=[0.485, 0.456, 0.406],
            std=[0.229, 0.224, 0.225]
        ),
    ])

    val_transform = transforms.Compose([
        transforms.Resize((256, 256)),
        transforms.CenterCrop(img_size),
        transforms.ToTensor(),
        transforms.Normalize(
            mean=[0.485, 0.456, 0.406],
            std=[0.229, 0.224, 0.225]
        ),
    ])

    return train_transform, val_transform


def build_model(num_classes: int, use_pretrained: bool):
    weights = torchvision.models.ResNet18_Weights.DEFAULT if use_pretrained else None
    model = torchvision.models.resnet18(weights=weights)
    model.fc = nn.Sequential(
        nn.Dropout(p=0.3),
        nn.Linear(model.fc.in_features, num_classes)
    )
    return model


def run_one_epoch(
    model,
    loader,
    criterion,
    device,
    optimizer=None,
    scaler=None,
    use_amp=False,
    max_grad_norm=1.0
):
    is_train = optimizer is not None
    model.train() if is_train else model.eval()

    running_loss = 0.0
    all_labels = []
    all_preds = []

    pbar = tqdm(loader, leave=False, desc="Train" if is_train else "Validate")

    for images, labels in pbar:
        images = images.to(device, non_blocking=True)
        labels = labels.to(device, non_blocking=True)

        if is_train:
            optimizer.zero_grad(set_to_none=True)

        with torch.set_grad_enabled(is_train):
            autocast_enabled = use_amp and device.type == "cuda"
            with torch.amp.autocast(device_type=device.type, enabled=autocast_enabled):
                logits = model(images)
                loss = criterion(logits, labels)

            if is_train:
                scaler.scale(loss).backward()
                scaler.unscale_(optimizer)
                nn.utils.clip_grad_norm_(model.parameters(), max_grad_norm)
                scaler.step(optimizer)
                scaler.update()

        probs = torch.softmax(logits, dim=1)
        preds = torch.argmax(probs, dim=1)

        running_loss += loss.item() * images.size(0)
        all_labels.extend(labels.detach().cpu().numpy().tolist())
        all_preds.extend(preds.detach().cpu().numpy().tolist())

        current_acc = accuracy_score(all_labels, all_preds)
        pbar.set_postfix(loss=f"{loss.item():.4f}", acc=f"{current_acc:.4f}")

    epoch_loss = running_loss / len(loader.dataset)
    epoch_acc = accuracy_score(all_labels, all_preds)
    epoch_f1 = f1_score(all_labels, all_preds, average="macro")

    return {
        "loss": epoch_loss,
        "acc": epoch_acc,
        "f1": epoch_f1
    }


def main():
    parser = argparse.ArgumentParser(description="Train rice leaf classifier")
    parser.add_argument(
        "--train-samples",
        default="outputs/rice_leaf_classifier/data/train_samples.json"
    )
    parser.add_argument(
        "--val-samples",
        default="outputs/rice_leaf_classifier/data/val_samples.json"
    )
    parser.add_argument(
        "--labels-file",
        default="models/rice_leaf_classifier/labels.json"
    )
    parser.add_argument(
        "--config-file",
        default="models/rice_leaf_classifier/config.yaml"
    )
    parser.add_argument(
        "--save-dir",
        default="outputs/rice_leaf_classifier/checkpoints"
    )

    args = parser.parse_args()

    config = load_config(args.config_file)
    class_names = load_json(args.labels_file)
    train_samples = load_json(args.train_samples)
    val_samples = load_json(args.val_samples)

    set_seed(config["seed"])

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    save_dir = Path(args.save_dir)
    save_dir.mkdir(parents=True, exist_ok=True)

    train_transform, val_transform = build_transforms(config["img_size"])

    train_dataset = RiceLeafClassificationDataset(train_samples, transform=train_transform)
    val_dataset = RiceLeafClassificationDataset(val_samples, transform=val_transform)

    train_loader = DataLoader(
        train_dataset,
        batch_size=config["batch_size"],
        shuffle=True,
        num_workers=config["num_workers"],
        pin_memory=torch.cuda.is_available(),
        persistent_workers=False
    )

    val_loader = DataLoader(
        val_dataset,
        batch_size=config["batch_size"],
        shuffle=False,
        num_workers=config["num_workers"],
        pin_memory=torch.cuda.is_available(),
        persistent_workers=False
    )

    model = build_model(
        num_classes=len(class_names),
        use_pretrained=config["use_pretrained"]
    ).to(device)

    train_labels = [s["label"] for s in train_samples]
    class_counts = np.bincount(train_labels, minlength=len(class_names))
    class_weights = len(train_labels) / (len(class_names) * np.maximum(class_counts, 1))
    class_weights = torch.tensor(class_weights, dtype=torch.float32, device=device)

    criterion = nn.CrossEntropyLoss(
        weight=class_weights,
        label_smoothing=config["label_smoothing"]
    )

    optimizer = optim.AdamW(
        model.parameters(),
        lr=config["learning_rate"],
        weight_decay=config["weight_decay"]
    )

    scheduler = optim.lr_scheduler.ReduceLROnPlateau(
        optimizer,
        mode="min",
        factor=0.5,
        patience=2
    )

    scaler = torch.amp.GradScaler(
        device.type,
        enabled=torch.cuda.is_available()
    )

    best_model_path = save_dir / "best_model.pth"
    final_model_path = save_dir / "final_model.pth"
    history_path = save_dir / "training_history.json"

    history = {
        "train_loss": [],
        "val_loss": [],
        "train_acc": [],
        "val_acc": [],
        "train_f1": [],
        "val_f1": [],
        "lr": []
    }

    best_val_f1 = -1.0
    best_epoch = -1
    patience_counter = 0

    start_time = time.time()

    for epoch in range(config["num_epochs"]):
        print(f"\nEpoch {epoch + 1}/{config['num_epochs']}")

        train_metrics = run_one_epoch(
            model=model,
            loader=train_loader,
            criterion=criterion,
            device=device,
            optimizer=optimizer,
            scaler=scaler,
            use_amp=torch.cuda.is_available(),
            max_grad_norm=config["max_grad_norm"]
        )

        val_metrics = run_one_epoch(
            model=model,
            loader=val_loader,
            criterion=criterion,
            device=device,
            optimizer=None,
            scaler=None,
            use_amp=False,
            max_grad_norm=config["max_grad_norm"]
        )

        scheduler.step(val_metrics["loss"])
        current_lr = optimizer.param_groups[0]["lr"]

        history["train_loss"].append(train_metrics["loss"])
        history["val_loss"].append(val_metrics["loss"])
        history["train_acc"].append(train_metrics["acc"])
        history["val_acc"].append(val_metrics["acc"])
        history["train_f1"].append(train_metrics["f1"])
        history["val_f1"].append(val_metrics["f1"])
        history["lr"].append(current_lr)

        print(
            f"Train Loss: {train_metrics['loss']:.4f} | "
            f"Train Acc: {train_metrics['acc']:.4f} | "
            f"Train F1: {train_metrics['f1']:.4f}"
        )
        print(
            f"Val   Loss: {val_metrics['loss']:.4f} | "
            f"Val   Acc: {val_metrics['acc']:.4f} | "
            f"Val   F1: {val_metrics['f1']:.4f} | "
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
                "class_names": class_names,
                "best_val_f1": best_val_f1,
                "config": config
            }
            torch.save(checkpoint, best_model_path)
            print(f"Best model saved to: {best_model_path}")
        else:
            patience_counter += 1

        if patience_counter >= config["early_stopping_patience"]:
            print("Early stopping triggered.")
            break

    torch.save(model.state_dict(), final_model_path)

    elapsed = time.time() - start_time
    print(f"\nTraining finished in {elapsed / 60:.2f} minutes")
    print(f"Best epoch: {best_epoch}")
    print(f"Best val F1: {best_val_f1:.4f}")

    history_to_save = {k: [float(x) for x in v] for k, v in history.items()}
    with history_path.open("w", encoding="utf-8") as f:
        json.dump(history_to_save, f, indent=2)

    print("Saved files:")
    print(f"- {best_model_path}")
    print(f"- {final_model_path}")
    print(f"- {history_path}")


if __name__ == "__main__":
    main()
