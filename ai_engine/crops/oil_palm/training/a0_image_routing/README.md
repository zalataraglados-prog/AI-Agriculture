# A0 Image Routing Training

## Goal

Train the A0 structure-detection routing model. A0 is the entry gatekeeper: it
detects supported oil palm structures, proposes bbox candidates, and validates
the user's selected `image_role`.

A0 does not predict FFB maturity, Ganoderma risk, growth status, yield, or any
tree-level conclusion.

## Recommended Framework

Use a lightweight YOLO-family detector. The first production candidate should be
a nano/small model that can return bbox candidates with low latency.

## Labels

See `models/oil_palm/a0_image_routing/labels.json`.

- `fruit_bunch`
- `trunk_base`
- `crown_region`

`unknown` is not a YOLO class. Unsupported or unclear images should be included
as empty-label negative samples. At inference time, the service should return a
route status such as `no_supported_structure_detected`, `role_mismatch`, or
`uncertain`.

## Data Sources

Training data can be bootstrapped from existing FFB, Ganoderma, and UAV crown
datasets by adding structure bboxes. Keep source, license, plantation/session,
and grouping metadata so train/val/test splits do not leak near-duplicate
evidence across sets.

## Training Command

```bash
# TODO: add once feature/oil-palm-a0-routing-model implements training scripts
```

## Design Principles

- A0 v1 does not replace the user's `image_role`; it validates the selected
  structure and proposes bbox candidates.
- Multiple candidates require user confirmation before downstream inference.
- Rejected candidates should be masked in a derived image before FFB/Ganoderma
  or growth analysis.
- When uncertain, return `route_status=uncertain` rather than a wrong route.
- A0 remains a routing gatekeeper, not a production diagnosis or maturity model.
