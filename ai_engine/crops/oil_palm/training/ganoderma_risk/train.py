"""
Ganoderma Risk Classifier - Training Script
Framework: torchvision (ResNet18)
Environment: Google Colab (GPU)

Usage:
1. Upload the entire project folder to Google Drive.
2. After mounting Drive in Colab, run the following from the project root:
   !python ai_engine/crops/oil_palm/training/ganoderma_risk/train.py
"""

import os
import copy
import json
import time
from pathlib import Path

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import DataLoader
from torchvision import datasets, models, transforms
from sklearn.metrics import classification_report, confusion_matrix

# --- Configuration -----------------------------------------------------------

DATA_DIR = "datasets/oil_palm/ganoderma_risk/classification"  # Importer output directory
MODEL_OUT = "models/oil_palm/ganoderma_risk"                  # Weights output directory
NUM_CLASSES = 2          # healthy / suspected_risk
BATCH_SIZE = 32
NUM_EPOCHS = 20
LEARNING_RATE = 1e-4
IMAGE_SIZE = 224
SEED = 42

# Class order (should match labels.json)
CLASS_NAMES = ["healthy", "suspected_risk"]

# --- Fix Random Seed ---------------------------------------------------------

torch.manual_seed(SEED)

# --- Data Augmentation & Preprocessing ---------------------------------------

data_transforms = {
    "train": transforms.Compose([
        transforms.Resize((IMAGE_SIZE, IMAGE_SIZE)),
        transforms.RandomHorizontalFlip(),
        transforms.RandomVerticalFlip(),
        transforms.RandomRotation(15),
        transforms.ColorJitter(brightness=0.3, contrast=0.3),
        transforms.ToTensor(),
        transforms.Normalize([0.485, 0.456, 0.406],
                             [0.229, 0.224, 0.225]),
    ]),
    "val": transforms.Compose([
        transforms.Resize((IMAGE_SIZE, IMAGE_SIZE)),
        transforms.ToTensor(),
        transforms.Normalize([0.485, 0.456, 0.406],
                             [0.229, 0.224, 0.225]),
    ]),
    "test": transforms.Compose([
        transforms.Resize((IMAGE_SIZE, IMAGE_SIZE)),
        transforms.ToTensor(),
        transforms.Normalize([0.485, 0.456, 0.406],
                             [0.229, 0.224, 0.225]),
    ]),
}

# --- Load Datasets -----------------------------------------------------------

print("Loading datasets...")
image_datasets = {
    split: datasets.ImageFolder(
        root=os.path.join(DATA_DIR, split),
        transform=data_transforms[split],
    )
    for split in ["train", "val", "test"]
}

dataloaders = {
    split: DataLoader(
        image_datasets[split],
        batch_size=BATCH_SIZE,
        shuffle=(split == "train"),
        num_workers=2,
    )
    for split in ["train", "val", "test"]
}

dataset_sizes = {split: len(image_datasets[split]) for split in ["train", "val", "test"]}
print(f"Dataset sizes: {dataset_sizes}")
print(f"Classes detected: {image_datasets['train'].classes}")

# --- Handle Class Imbalance (Weighted Loss) ----------------------------------

train_labels = [label for _, label in image_datasets["train"].samples]
class_counts = [train_labels.count(i) for i in range(NUM_CLASSES)]
total = sum(class_counts)
class_weights = torch.tensor(
    [total / (NUM_CLASSES * c) for c in class_counts], dtype=torch.float
)
print(f"Class counts (train): {dict(zip(image_datasets['train'].classes, class_counts))}")
print(f"Class weights: {dict(zip(image_datasets['train'].classes, class_weights.tolist()))}")

# --- Device Setup ------------------------------------------------------------

device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
print(f"Using device: {device}")

# --- Model: ResNet18 Transfer Learning ---------------------------------------

model = models.resnet18(weights=models.ResNet18_Weights.IMAGENET1K_V1)

# Freeze early layers: only train the final fully connected layer (Phase 1)
for param in model.parameters():
    param.requires_grad = False

# Replace the final layer
model.fc = nn.Linear(model.fc.in_features, NUM_CLASSES)
model = model.to(device)

criterion = nn.CrossEntropyLoss(weight=class_weights.to(device))
optimizer = optim.Adam(model.fc.parameters(), lr=LEARNING_RATE)
scheduler = optim.lr_scheduler.StepLR(optimizer, step_size=7, gamma=0.1)

# --- Training Function -------------------------------------------------------

def train_model(model, criterion, optimizer, scheduler, num_epochs):
    best_model_wts = copy.deepcopy(model.state_dict())
    best_acc = 0.0
    history = {"train_loss": [], "train_acc": [], "val_loss": [], "val_acc": []}

    for epoch in range(num_epochs):
        print(f"\nEpoch {epoch+1}/{num_epochs} " + "-" * 30)

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
            epoch_acc = running_corrects.double() / dataset_sizes[phase]
            print(f"{phase:5s} Loss: {epoch_loss:.4f}  Acc: {epoch_acc:.4f}")

            history[f"{phase}_loss"].append(epoch_loss)
            history[f"{phase}_acc"].append(epoch_acc.item())

            if phase == "val" and epoch_acc > best_acc:
                best_acc = epoch_acc
                best_model_wts = copy.deepcopy(model.state_dict())
                print(f"  ✓ New best val acc: {best_acc:.4f}")

    print(f"\nBest val accuracy: {best_acc:.4f}")
    model.load_state_dict(best_model_wts)
    return model, history

# --- Phase 1: Training FC Layer Only -----------------------------------------

print("\n=== Phase 1: Training FC layer only ===")
model, history = train_model(model, criterion, optimizer, scheduler, num_epochs=10)

# --- Phase 2: Unfreeze All Layers (Fine-tuning) ------------------------------

print("\n=== Phase 2: Fine-tuning all layers ===")
for param in model.parameters():
    param.requires_grad = True

optimizer = optim.Adam(model.parameters(), lr=1e-5)  # Lower learning rate for fine-tuning
scheduler = optim.lr_scheduler.StepLR(optimizer, step_size=5, gamma=0.1)

model, history2 = train_model(model, criterion, optimizer, scheduler, num_epochs=NUM_EPOCHS - 10)

# --- Test Set Evaluation -----------------------------------------------------

print("\n=== Test Set Evaluation ===")
model.eval()
all_preds = []
all_labels = []

with torch.no_grad():
    for inputs, labels in dataloaders["test"]:
        inputs = inputs.to(device)
        outputs = model(inputs)
        _, preds = torch.max(outputs, 1)
        all_preds.extend(preds.cpu().numpy())
        all_labels.extend(labels.numpy())

report = classification_report(
    all_labels, all_preds,
    target_names=image_datasets["test"].classes,
    output_dict=True,
)
print(classification_report(
    all_labels, all_preds,
    target_names=image_datasets["test"].classes,
))
print("Confusion Matrix:")
print(confusion_matrix(all_labels, all_preds))

# --- Save Weights and Metrics ------------------------------------------------

Path(MODEL_OUT).mkdir(parents=True, exist_ok=True)

# Save model weights
best_pt_path = os.path.join(MODEL_OUT, "best.pth")
torch.save(model.state_dict(), best_pt_path)
print(f"\nModel saved to: {best_pt_path}")

# Save metrics.json (for hand-over/reporting)
metrics = {
    "model": "ResNet18",
    "framework": "torchvision",
    "num_classes": NUM_CLASSES,
    "classes": image_datasets["test"].classes,
    "test_accuracy": report["accuracy"],
    "healthy": {
        "precision": report["healthy"]["precision"],
        "recall": report["healthy"]["recall"],
        "f1": report["healthy"]["f1-score"],
        "support": report["healthy"]["support"],
    },
    "suspected_risk": {
        "precision": report["suspected_risk"]["precision"],
        "recall": report["suspected_risk"]["recall"],
        "f1": report["suspected_risk"]["f1-score"],
        "support": report["suspected_risk"]["support"],
    },
    "macro_avg": {
        "precision": report["macro avg"]["precision"],
        "recall": report["macro avg"]["recall"],
        "f1": report["macro avg"]["f1-score"],
    },
}

metrics_path = os.path.join(MODEL_OUT, "metrics.json")
with open(metrics_path, "w") as f:
    json.dump(metrics, f, indent=2)
print(f"Metrics saved to: {metrics_path}")