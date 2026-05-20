# Ganoderma Risk Model Card

## Model Details

| Field | Value |
|-------|-------|
| Model Name | Oil Palm Ganoderma Risk Classifier |
| Task | Image classification (risk screening) |
| Crop | Oil Palm |
| Framework | ResNet18 |
| Current Status | **offline trained v1; not integrated into AI Engine runtime** |
| Model Version | `oil_palm_ganoderma_resnet18_v1.0.0_offline` |

## Intended Use

Screen trunk-base oil palm images for visible Ganoderma / basal stem rot risk
signals. This is a risk-screening model, not a diagnostic tool. Positive
outputs must be described as suspected risk and should trigger recheck or expert
confirmation.

## Labels

Project-standard labels are `healthy`, `suspected_risk`, and
`other_stress_unknown`.

The recorded v1 model trains only `healthy` and `suspected_risk`.
`other_stress_unknown` is reserved for future data and is not produced by this
offline v1 classifier.

## Training Data

The handoff describes two Roboflow sources: one Ganoderma-risk source and one
healthy oil palm source. The source pages, dataset versions, licenses, download
dates, and split grouping still require teammate confirmation.

The current manifest intentionally records source URLs without temporary
Roboflow download keys.

## Evaluation

Recorded handoff metrics:

| Class | Precision | Recall | F1 | Support |
|-------|-----------|--------|----|---------|
| healthy | 0.94 | 1.00 | 0.97 | 92 |
| suspected_risk | 1.00 | 0.92 | 0.96 | 75 |
| accuracy | | | 0.96 | 167 |
| macro avg | 0.97 | 0.96 | 0.96 | 167 |

Confusion matrix:

```text
                     Predicted healthy  Predicted suspected_risk
Actual healthy                      92                         0
Actual suspected_risk                6                        69
```

See `metrics.json` for the normalized schema and pending fields.

## Runtime Status

This branch records offline training metadata only. AI Engine still uses the
safe mock Ganoderma predictor unless a future runtime branch adds a real
predictor and loads weights through `OIL_PALM_GANODERMA_MODEL_PATH`.

## Ethical Considerations

- This model does not confirm Ganoderma or any agronomic diagnosis.
- All positive outputs must be labeled as suspected risk.
- False negatives matter: 6 of 75 suspected-risk test samples were missed in
  the handoff metrics.
- Expert confirmation is required before disease management action.

## Limitations

- v1 is only suitable for trunk-base/root-level RGB photography.
- Generalization across plantations, palm ages, cameras, and environments has
  not been validated.
- The split strategy appears to be stratified random/source-limited; grouped
  tree/plantation leakage has not been ruled out.
- `other_stress_unknown` is not trained in v1.
- Weight hash, exact split counts, dataset versions, and licenses are pending.

## Changelog

| Version | Date | Notes |
|---------|------|-------|
| `oil_palm_ganoderma_resnet18_v1.0.0_offline` | 2026-05 | Offline ResNet18 handoff metrics recorded; runtime integration pending. |
| `oil_palm_ganoderma_mock_v1` | 2026-05 | Mock predictor established. No real weights. |
