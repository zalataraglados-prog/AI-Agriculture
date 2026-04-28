"""Integration tests for the FastAPI prediction API.

These tests use a mock classifier to avoid depending on a real
PyTorch checkpoint, making them runnable in CI without GPU or
large model files.
"""

import io
import sys
from pathlib import Path
from unittest.mock import MagicMock

import httpx
import pytest
from PIL import Image

# Ensure the project root is importable
PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))


_MOCK_PREDICTION = {
    "predicted_class": "Leaf_Blast",
    "confidence": 0.93,
    "topk": [
        {"label": "Leaf_Blast", "score": 0.93},
        {"label": "Brown_Spot", "score": 0.05},
        {"label": "HealthyLeaf", "score": 0.02},
    ],
    "model_version": "rice_cls_v0.1.0",
    "metadata": {},
    "geometry": None,
}

_MOCK_MODEL_INFO = {
    "model_name": "RiceLeafClassifier",
    "model_version": "rice_cls_v0.1.0",
    "architecture": "resnet18",
    "num_classes": "8",
}


def _make_mock_classifier():
    """Create a mock classifier that implements the BasePredictor interface."""
    mock = MagicMock()
    mock.predict.return_value = _MOCK_PREDICTION.copy()
    mock.get_model_info.return_value = _MOCK_MODEL_INFO.copy()
    return mock


def _create_test_image_bytes() -> bytes:
    """Generate a valid JPEG image in memory."""
    img = Image.new("RGB", (224, 224), color=(128, 128, 128))
    buf = io.BytesIO()
    img.save(buf, format="JPEG")
    return buf.getvalue()


# ------------------------------------------------------------------
# Skip all tests if torch is not installed (CI without GPU deps)
# ------------------------------------------------------------------

torch_available = True
try:
    import torch  # noqa: F401
except ImportError:
    torch_available = False

pytestmark = pytest.mark.skipif(
    not torch_available,
    reason="PyTorch not installed 鈥?skipping API integration tests",
)


@pytest.fixture()
def anyio_backend():
    """Run async API tests on asyncio only."""
    return "asyncio"


# ------------------------------------------------------------------
# Fixtures
# ------------------------------------------------------------------

@pytest.fixture()
async def client():
    """Create an HTTPX ASGI client with the model injected."""
    mock_classifier = _make_mock_classifier()

    from ai_engine.crops.rice.inference.api import set_classifier
    from ai_engine.main import app

    set_classifier(mock_classifier)
    transport = httpx.ASGITransport(app=app)
    async with httpx.AsyncClient(
        transport=transport,
        base_url="http://testserver",
    ) as c:
        yield c


# ------------------------------------------------------------------
# Test: Health endpoint
# ------------------------------------------------------------------

class TestHealthEndpoint:
    @pytest.mark.anyio
    async def test_health_returns_200(self, client):
        response = await client.get("/api/v1/health")
        assert response.status_code == 200

    @pytest.mark.anyio
    async def test_health_returns_ok_status(self, client):
        data = (await client.get("/api/v1/health")).json()
        assert data["status"] == "ok"
        assert data["service"] == "smart-farm-ai-engine"
        assert "crop_profile" in data


# ------------------------------------------------------------------
# Test: Predict endpoint 鈥?happy path
# ------------------------------------------------------------------

class TestPredictEndpoint:
    @pytest.mark.anyio
    async def test_predict_returns_valid_schema(self, client):
        image_bytes = _create_test_image_bytes()
        response = await client.post(
            "/api/v1/predict",
            files={"file": ("test.jpg", image_bytes, "image/jpeg")},
        )
        assert response.status_code == 200

        data = response.json()
        assert data["status"] == "success"
        assert isinstance(data["results"], list)
        assert len(data["results"]) == 1

        result = data["results"][0]
        assert "predicted_class" in result
        assert "confidence" in result
        assert "topk" in result
        assert "model_version" in result
        assert "metadata" in result
        assert "geometry" in result

    @pytest.mark.anyio
    async def test_predict_topk_structure(self, client):
        image_bytes = _create_test_image_bytes()
        response = await client.post(
            "/api/v1/predict",
            files={"file": ("test.jpg", image_bytes, "image/jpeg")},
        )
        data = response.json()
        topk = data["results"][0]["topk"]
        assert len(topk) == 3
        for item in topk:
            assert "label" in item
            assert "score" in item

    @pytest.mark.anyio
    async def test_rice_predict_endpoint_works(self, client):
        image_bytes = _create_test_image_bytes()
        response = await client.post(
            "/api/v1/rice/predict",
            files={"file": ("test.jpg", image_bytes, "image/jpeg")},
        )
        assert response.status_code == 200
        assert response.json()["status"] == "success"


class TestOilPalmMockEndpoints:
    @pytest.mark.anyio
    async def test_oil_palm_analyze_image_mock(self, client):
        image_bytes = _create_test_image_bytes()
        response = await client.post(
            "/api/v1/oil-palm/analyze-image",
            files={"file": ("test.jpg", image_bytes, "image/jpeg")},
        )
        assert response.status_code == 200
        data = response.json()
        assert data["crop"] == "oil_palm"
        assert data["status"] == "mock"

    @pytest.mark.anyio
    async def test_oil_palm_analyze_session_mock(self, client):
        response = await client.post("/api/v1/oil-palm/analyze-session")
        assert response.status_code == 200
        assert response.json()["status"] == "mock"

    @pytest.mark.anyio
    async def test_oil_palm_analyze_uav_mission_mock(self, client):
        response = await client.post("/api/v1/oil-palm/analyze-uav-mission")
        assert response.status_code == 200
        assert response.json()["status"] == "mock"


# ------------------------------------------------------------------
# Test: Predict endpoint 鈥?error cases
# ------------------------------------------------------------------

class TestPredictErrors:
    @pytest.mark.anyio
    async def test_predict_invalid_file_returns_error(self, client):
        response = await client.post(
            "/api/v1/predict",
            files={"file": ("bad.txt", b"not an image", "text/plain")},
        )
        assert response.status_code == 422
        data = response.json()
        assert data["status"] == "error"
        assert "message" in data

    @pytest.mark.anyio
    async def test_predict_no_file_returns_422(self, client):
        response = await client.post("/api/v1/predict")
        assert response.status_code == 422
