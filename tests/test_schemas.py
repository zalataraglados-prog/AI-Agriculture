import pytest
from pydantic import ValidationError
from service.schemas.prediction import PredictionResult, PredictionResponse, PredictionItem


def test_prediction_result_schema():
    # Valid creation
    result = PredictionResult(
        predicted_class="Leaf_Blast",
        confidence=0.95,
        topk=[
            PredictionItem(label="Leaf_Blast", score=0.95),
            PredictionItem(label="Brown_Spot", score=0.04),
        ],
        model_version="v1.0"
    )
    assert result.predicted_class == "Leaf_Blast"
    assert result.metadata == {}
    assert result.geometry is None

    # Test serialization
    result_dict = result.model_dump()
    assert "metadata" in result_dict
    assert "geometry" in result_dict


def test_prediction_response_schema():
    result = PredictionResult(
        predicted_class="Healthy",
        confidence=0.99,
        topk=[PredictionItem(label="Healthy", score=0.99)],
        model_version="v1.0"
    )
    
    response = PredictionResponse(
        status="success",
        results=[result]
    )
    
    assert response.status == "success"
    assert len(response.results) == 1
    assert response.metadata == {}
    
    resp_dict = response.model_dump()
    assert resp_dict["status"] == "success"
    assert isinstance(resp_dict["results"], list)
