"""Train the oil palm Ganoderma risk classifier.

This script is intentionally CLI-driven so local, Colab, and future CI runs can
record the same configuration. It trains active v1 classes only:

    healthy, suspected_risk

The project label set still reserves other_stress_unknown for future data, but
that reserved class is ignored during v1 training.
"""

from __future__ import annotations

import argparse
import json
import random
from pathlib import Path
from typing import Any


PROJECT_LABELS_FILE = Path("models/oil_palm/ganoderma_risk/labels.json")
DEFAULT_DATA_DIR = Path("datasets/oil_palm/ganoderma_risk/classification")
DEFAULT_MODEL_OUT = Path("models/oil_palm/ganoderma_risk")
DEFAULT_ACTIVE_LABELS = ["healthy", "suspected_risk"]
DEFAULT_MODEL_VERSION = "oil_palm_ganoderma_resnet18_v1.0.0_offline"


def load_project_labels(labels_file: str | Path = PROJECT_LABELS_FILE) -> list[str]:
    with Path(labels_file).open("r", encoding="utf-8") as source:
        labels = json.load(source)
    if not isinstance(labels, list) or not all(isinstance(item, str) for item in labels):
        raise ValueError(f"labels.json must contain a list of strings: {labels_file}")
    return labels


def parse_active_labels(raw: str) -> list[str]:
    labels = [item.strip() for item in raw.split(",") if item.strip()]
    if not labels:
        raise ValueError("At least one active label is required")
    return labels


def validate_active_labels(project_labels: list[str], active_labels: list[str]) -> None:
    missing = [label for label in active_labels if label not in project_labels]
    if missing:
        raise ValueError(f"Active labels are not in project labels: {missing}")
    if len(set(active_labels)) != len(active_labels):
        raise ValueError(f"Active labels must be unique: {active_labels}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Train Ganoderma risk classifier.")
    parser.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR))
    parser.add_argument("--model-out", default=str(DEFAULT_MODEL_OUT))
    parser.add_argument("--labels-file", default=str(PROJECT_LABELS_FILE))
    parser.add_argument("--active-labels", default=",".join(DEFAULT_ACTIVE_LABELS))
    parser.add_argument("--model-version", default=DEFAULT_MODEL_VERSION)
    parser.add_argument("--dataset-version", default="v1_pending_team_confirmation")
    parser.add_argument(
        "--dataset-manifest",
        default="datasets/oil_palm/manifests/ganoderma_risk.example.json",
    )
    parser.add_argument("--epochs", type=int, default=20)
    parser.add_argument("--phase1-epochs", type=int, default=10)
    parser.add_argument("--batch-size", type=int, default=32)
    parser.add_argument("--learning-rate", type=float, default=1e-4)
    parser.add_argument("--finetune-learning-rate", type=float, default=1e-5)
    parser.add_argument("--image-size", type=int, default=224)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--num-workers", type=int, default=2)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    project_labels = load_project_labels(args.labels_file)
    active_labels = parse_active_labels(args.active_labels)
    validate_active_labels(project_labels, active_labels)
    train(args, project_labels, active_labels)


def train(
    args: argparse.Namespace,
    project_labels: list[str],
    active_labels: list[str],
) -> None:
    import torch
    import torch.nn as nn
    import torch.optim as optim
    from sklearn.metrics import classification_report, confusion_matrix
    from torch.utils.data import DataLoader
    from torchvision import datasets, models, transforms

    torch.manual_seed(args.seed)
    random.seed(args.seed)

    class ActiveLabelImageFolder(datasets.ImageFolder):
        def __init__(self, root: str | Path, active: list[str], transform: Any) -> None:
            self.active = active
            super().__init__(root=str(root), transform=transform)

        def find_classes(self, directory: str) -> tuple[list[str], dict[str, int]]:
            missing = [label for label in self.active if not (Path(directory) / label).is_dir()]
            if missing:
                raise FileNotFoundError(
                    f"Missing active class directories under {directory}: {missing}"
                )
            return self.active, {label: index for index, label in enumerate(self.active)}

    image_size = args.image_size
    data_transforms = {
        "train": transforms.Compose([
            transforms.Resize((image_size, image_size)),
            transforms.RandomHorizontalFlip(),
            transforms.RandomVerticalFlip(),
            transforms.RandomRotation(15),
            transforms.ColorJitter(brightness=0.3, contrast=0.3),
            transforms.ToTensor(),
            transforms.Normalize([0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
        ]),
        "val": transforms.Compose([
            transforms.Resize((image_size, image_size)),
            transforms.ToTensor(),
            transforms.Normalize([0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
        ]),
        "test": transforms.Compose([
            transforms.Resize((image_size, image_size)),
            transforms.ToTensor(),
            transforms.Normalize([0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
        ]),
    }

    data_dir = Path(args.data_dir)
    image_datasets = {
        split: ActiveLabelImageFolder(data_dir / split, active_labels, data_transforms[split])
        for split in ["train", "val", "test"]
    }
    dataset_sizes = {split: len(dataset) for split, dataset in image_datasets.items()}
    if any(size == 0 for size in dataset_sizes.values()):
        raise ValueError(f"All train/val/test splits must contain data: {dataset_sizes}")

    train_labels = [label for _, label in image_datasets["train"].samples]
    class_counts = [train_labels.count(index) for index in range(len(active_labels))]
    if any(count == 0 for count in class_counts):
        raise ValueError(f"Each active class needs train samples: {dict(zip(active_labels, class_counts))}")

    dataloaders = {
        split: DataLoader(
            dataset,
            batch_size=args.batch_size,
            shuffle=(split == "train"),
            num_workers=args.num_workers,
        )
        for split, dataset in image_datasets.items()
    }

    total = sum(class_counts)
    class_weights = torch.tensor(
        [total / (len(active_labels) * count) for count in class_counts],
        dtype=torch.float,
    )

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    model = models.resnet18(weights=models.ResNet18_Weights.IMAGENET1K_V1)
    for param in model.parameters():
        param.requires_grad = False
    model.fc = nn.Linear(model.fc.in_features, len(active_labels))
    model = model.to(device)

    criterion = nn.CrossEntropyLoss(weight=class_weights.to(device))
    optimizer = optim.Adam(model.fc.parameters(), lr=args.learning_rate)
    scheduler = optim.lr_scheduler.StepLR(optimizer, step_size=7, gamma=0.1)

    model, history1 = train_model(
        model,
        dataloaders,
        dataset_sizes,
        criterion,
        optimizer,
        scheduler,
        device,
        args.phase1_epochs,
    )

    for param in model.parameters():
        param.requires_grad = True
    optimizer = optim.Adam(model.parameters(), lr=args.finetune_learning_rate)
    scheduler = optim.lr_scheduler.StepLR(optimizer, step_size=5, gamma=0.1)

    model, history2 = train_model(
        model,
        dataloaders,
        dataset_sizes,
        criterion,
        optimizer,
        scheduler,
        device,
        args.epochs - args.phase1_epochs,
    )

    report, matrix = evaluate_model(
        model,
        dataloaders["test"],
        device,
        active_labels,
        classification_report,
        confusion_matrix,
    )

    model_out = Path(args.model_out)
    model_out.mkdir(parents=True, exist_ok=True)
    weights_path = model_out / "best.pth"
    torch.save(model.state_dict(), weights_path)

    metrics = build_metrics(
        args=args,
        project_labels=project_labels,
        active_labels=active_labels,
        dataset_sizes=dataset_sizes,
        class_counts=class_counts,
        report=report,
        confusion=matrix,
        device=str(device),
        history={"phase1": history1, "phase2": history2},
    )
    (model_out / "metrics.json").write_text(
        json.dumps(metrics, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"Model saved to: {weights_path}")
    print(f"Metrics saved to: {model_out / 'metrics.json'}")


def train_model(
    model: Any,
    dataloaders: dict[str, Any],
    dataset_sizes: dict[str, int],
    criterion: Any,
    optimizer: Any,
    scheduler: Any,
    device: Any,
    num_epochs: int,
) -> tuple[Any, dict[str, list[float]]]:
    import copy
    import torch

    best_model_wts = copy.deepcopy(model.state_dict())
    best_acc = 0.0
    history = {"train_loss": [], "train_acc": [], "val_loss": [], "val_acc": []}

    for _epoch in range(num_epochs):
        for phase in ["train", "val"]:
            model.train() if phase == "train" else model.eval()
            running_loss = 0.0
            running_corrects = 0

            for inputs, labels in dataloaders[phase]:
                inputs = inputs.to(device)
                labels = labels.to(device)
                optimizer.zero_grad()

                with torch.set_grad_enabled(phase == "train"):
                    outputs = model(inputs)
                    _, preds = torch.max(outputs, 1)
                    loss = criterion(outputs, labels)
                    if phase == "train":
                        loss.backward()
                        optimizer.step()

                running_loss += loss.item() * inputs.size(0)
                running_corrects += torch.sum(preds == labels.data)

            if phase == "train":
                scheduler.step()

            epoch_loss = running_loss / dataset_sizes[phase]
            epoch_acc = (running_corrects.double() / dataset_sizes[phase]).item()
            history[f"{phase}_loss"].append(epoch_loss)
            history[f"{phase}_acc"].append(epoch_acc)

            if phase == "val" and epoch_acc > best_acc:
                best_acc = epoch_acc
                best_model_wts = copy.deepcopy(model.state_dict())

    model.load_state_dict(best_model_wts)
    return model, history


def evaluate_model(
    model: Any,
    test_loader: Any,
    device: Any,
    active_labels: list[str],
    classification_report_fn: Any,
    confusion_matrix_fn: Any,
) -> tuple[dict[str, Any], list[list[int]]]:
    import torch

    model.eval()
    all_preds = []
    all_labels = []
    with torch.no_grad():
        for inputs, labels in test_loader:
            inputs = inputs.to(device)
            outputs = model(inputs)
            _, preds = torch.max(outputs, 1)
            all_preds.extend(preds.cpu().numpy())
            all_labels.extend(labels.numpy())

    report = classification_report_fn(
        all_labels,
        all_preds,
        labels=list(range(len(active_labels))),
        target_names=active_labels,
        output_dict=True,
        zero_division=0,
    )
    matrix = confusion_matrix_fn(
        all_labels,
        all_preds,
        labels=list(range(len(active_labels))),
    ).tolist()
    return report, matrix


def build_metrics(
    args: argparse.Namespace,
    project_labels: list[str],
    active_labels: list[str],
    dataset_sizes: dict[str, int],
    class_counts: list[int],
    report: dict[str, Any],
    confusion: list[list[int]],
    device: str,
    history: dict[str, Any],
) -> dict[str, Any]:
    per_class = {}
    for label in project_labels:
        if label in active_labels:
            class_report = report[label]
            per_class[label] = {
                "precision": class_report["precision"],
                "recall": class_report["recall"],
                "f1": class_report["f1-score"],
                "support": class_report["support"],
            }
        else:
            per_class[label] = {
                "precision": None,
                "recall": None,
                "f1": None,
                "support": 0,
                "status": "reserved_not_trained_in_v1",
            }

    return {
        "model_version": args.model_version,
        "framework": "ResNet18",
        "task": "ganoderma_risk",
        "dataset_version": args.dataset_version,
        "dataset_manifest": args.dataset_manifest,
        "labels": project_labels,
        "active_training_labels": active_labels,
        "artifact": {
            "weights": str(Path(args.model_out) / "best.pth"),
            "weights_sha256": None,
            "status": "pending_team_confirmation",
        },
        "metrics": {
            "accuracy": report["accuracy"],
            "precision_macro": report["macro avg"]["precision"],
            "recall_macro": report["macro avg"]["recall"],
            "f1_macro": report["macro avg"]["f1-score"],
            "per_class": per_class,
            "confusion_matrix": confusion,
        },
        "inference": {
            "fps_gpu": None,
            "fps_cpu": None,
            "input_size": [args.image_size, args.image_size],
            "device": device,
        },
        "training": {
            "epochs": args.epochs,
            "phase1_epochs": args.phase1_epochs,
            "batch_size": args.batch_size,
            "optimizer": "Adam",
            "learning_rate": {
                "phase1": args.learning_rate,
                "phase2": args.finetune_learning_rate,
            },
            "augmentation": "RandomHorizontalFlip, RandomVerticalFlip, RandomRotation(15), ColorJitter",
            "seed": args.seed,
            "dataset_sizes": dataset_sizes,
            "train_class_counts": dict(zip(active_labels, class_counts)),
            "history": history,
        },
        "notes": [
            "All positive outputs are suspected risk, never confirmed disease.",
            "other_stress_unknown is a reserved project label and is not trained in v1.",
            "Dataset license, split grouping, and weight hash require teammate confirmation.",
        ],
    }


if __name__ == "__main__":
    main()
