# Ganoderma Risk Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm Ganoderma Risk Classifier |
| Task | Image classification (risk screening) |
| Crop | Oil Palm |
| Framework | ResNet18 (torchvision, two-phase fine-tune) |
| Current Status | **v1 trained** |
| Model Version | `oil_palm_ganoderma_resnet18_v1` |

## Intended Use

Screen trunk base images for Ganoderma boninense / basal stem rot (BSR) risk indicators.
This is a **risk screening** model, NOT a diagnostic tool. All positive outputs are labeled as
`suspected`, never `confirmed`.

## Labels

See [labels.json](./labels.json): `healthy`, `suspected_risk`, `other_stress_unknown`.

v1 uses conservative 2-class labels (`healthy`, `suspected_risk`). `other_stress_unknown` is
reserved for future use when sufficient data is available. Future expansion (when data quality
supports it): `suspected_early`, `moderate`, `severe`, `dead_or_collapsed`.

## Training Data

| Dataset | Source | Images | Purpose |
|---------|--------|--------|---------|
| Oil Palm Ganoderma Infected | Roboflow | 470 | suspected_risk samples |
| Oil Palm Healthy | Roboflow | 303 | healthy samples |

- **Camera angle**: All images are trunk base / root-level photography
- **Split**: train 70% / val 15% / test 15%, stratified by class (seed=42)
- **Class imbalance**: ~333 suspected_risk vs ~207 healthy in train set; handled via weighted CrossEntropyLoss

See `datasets/oil_palm/manifests/ganoderma_risk.json` for full data source details.

## Training Configuration

| Parameter | Value |
|-----------|-------|
| Base model | ResNet18 (ImageNet pretrained) |
| Phase 1 | FC layer only, lr=1e-4, 10 epochs |
| Phase 2 | All layers fine-tuned, lr=1e-5, 10 epochs |
| Batch size | 32 |
| Image size | 224×224 |
| Augmentation | Random flip, rotation ±15°, brightness/contrast jitter |

## Evaluation

Test set results (n=167):

| Class | Precision | Recall | F1 | Support |
|-------|-----------|--------|----|---------|
| healthy | 0.94 | 1.00 | 0.97 | 92 |
| suspected_risk | 1.00 | 0.92 | 0.96 | 75 |
| **accuracy** | | | **0.96** | 167 |
| macro avg | 0.97 | 0.96 | 0.96 | 167 |

Confusion matrix:
```
                     Predicted healthy  Predicted suspected_risk
Actual healthy                      92                         0
Actual suspected_risk                6                        69
```

See [metrics.json](./metrics.json) for full metrics.

## Ethical Considerations

- **This model does NOT provide a medical or agronomic diagnosis.**
- All risk outputs must be labeled as `suspected_not_confirmed`.
- Expert or laboratory confirmation is REQUIRED before any disease management action.
- False negatives may delay treatment; false positives may cause unnecessary intervention.
- Model output should always be accompanied by `advice: recheck trunk base; expert confirmation required`.

## Limitations

- 6 out of 75 suspected_risk images were misclassified as healthy (recall 0.92); human re-inspection is recommended for all model outputs.
- Model is only suitable for trunk base / root-level photography; top-down or canopy-level images will produce unreliable results.
- Training dataset is relatively small (~773 images total); generalization across different plantations, palm ages, and environmental conditions has not been validated.
- RGB-only classification may miss early-stage infections not yet visible on the trunk surface.
- `other_stress_unknown` class is not supported in v1; the model cannot distinguish Ganoderma from other stress conditions.
- Ganoderma public data is sparse and often limited to hyperspectral or small samples.
- Performance may vary significantly across palm age, variety, and environmental conditions.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| v1.0.0 | 2025-05 | Initial version. ResNet18 two-phase fine-tune. Test accuracy 0.96. |
| mock_v1 | 2026-05 | Mock predictor established. No real weights. |
