import pytest
from pydantic import ValidationError
from ai_engine.common.schemas.prediction import (
    PredictionEnvelope,
    PredictionResponse,
    PredictionResult,
    TopKItem,
)


def test_prediction_response_schema():
    response = PredictionResponse(
        status="success",
        predicted_class="Leaf_Blast",
        confidence=0.95,
        topk=[
            TopKItem(predicted_class="Leaf_Blast", confidence=0.95),
            TopKItem(predicted_class="Brown_Spot", confidence=0.04),
        ],
        model_version="v1.0",
    )
    assert response.predicted_class == "Leaf_Blast"
    assert response.metadata == {}

    response_dict = response.model_dump()
    assert response_dict["status"] == "success"
    assert response_dict["topk"][0]["predicted_class"] == "Leaf_Blast"


def test_prediction_envelope_schema():
    result = PredictionResult(
        task="ffb_maturity",
        label="ripe",
        confidence=0.91,
        geometry={"type": "bbox", "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4},
        metadata={"crop": "oil_palm"},
    )

    response = PredictionEnvelope(
        status="success",
        results=[result],
        geometry=[result.geometry],
        model_version="oil_palm_ffb_mock_v1",
    )

    assert response.status == "success"
    assert len(response.results) == 1
    assert response.metadata == {}

    resp_dict = response.model_dump()
    assert resp_dict["status"] == "success"
    assert isinstance(resp_dict["results"], list)
    assert resp_dict["geometry"][0]["type"] == "bbox"


def test_prediction_response_rejects_invalid_confidence():
    with pytest.raises(ValidationError):
        PredictionResponse(
            predicted_class="Leaf_Blast",
            confidence=1.2,
            model_version="v1.0",
        )
