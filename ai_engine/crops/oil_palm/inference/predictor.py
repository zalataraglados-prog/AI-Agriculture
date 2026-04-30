from __future__ import annotations

import hashlib


def analyze_oil_palm_image(image_bytes: bytes) -> dict:
    digest = hashlib.md5(image_bytes).hexdigest()
    idx = int(digest[:2], 16) % 3
    classes = ["frond_healthy", "frond_nutrient_stress", "frond_suspected_blight"]
    predicted = classes[idx]
    conf = 0.58 + (int(digest[2:4], 16) / 255.0) * 0.4
    disease_risk = round(min(max(conf - 0.15, 0.1), 0.98), 4)

    return {
        "predicted_class": predicted,
        "confidence": round(min(conf, 0.99), 4),
        "model_version": "oil_palm_mock_v1",
        "topk": [
            {"predicted_class": predicted, "confidence": round(min(conf, 0.99), 4)},
            {"predicted_class": classes[(idx + 1) % 3], "confidence": 0.2},
        ],
        "metadata": {
            "disease_rate": disease_risk,
            "is_diseased": predicted != "frond_healthy",
            "growth_vigor_index": round(1.0 - disease_risk * 0.6, 4),
            "weather_risk_score": round(0.35 + disease_risk * 0.3, 4),
            "yield_risk_score": round(0.25 + disease_risk * 0.5, 4),
            "advice_code": "inspect_nutrients" if predicted != "frond_healthy" else "normal_monitoring",
        },
    }
