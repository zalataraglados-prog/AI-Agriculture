from __future__ import annotations

import io
import sys
import types

from fastapi import FastAPI
from fastapi.testclient import TestClient
from PIL import Image

from ai_engine.common.schemas.prediction import PredictionEnvelope
from ai_engine.common.predictors.base import PredictorContext
from ai_engine.crops.oil_palm.inference.a0_yolo_predictor import A0YoloPredictor
from ai_engine.crops.oil_palm.inference.api import router


PNG_BYTES = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"


def build_client() -> TestClient:
    app = FastAPI()
    app.include_router(router, prefix="/api/v1")
    return TestClient(app)


def test_oil_palm_analyze_routes_each_image_role_to_mock_predictor():
    client = build_client()
    expected_tasks = {
        "fruit": "ffb_maturity",
        "trunk_base": "ganoderma_risk",
        "crown": "growth_vigor",
        "uav_tile": "uav_tree_crown",
    }

    for image_role, task in expected_tasks.items():
        response = client.post(
            "/api/v1/oil-palm/analyze",
            data={
                "image_role": image_role,
                "tree_code": "OP-000001",
                "session_id": "OS-000001",
            },
            files={"file": (f"{image_role}.png", PNG_BYTES, "image/png")},
        )

        assert response.status_code == 200
        payload = response.json()
        envelope = PredictionEnvelope.model_validate(payload)
        assert envelope.status == "success"
        assert envelope.results[0].task == task
        assert envelope.results[0].geometry
        assert envelope.metadata["crop"] == "oil_palm"
        assert envelope.metadata["image_role"] == image_role
        assert envelope.metadata["tree_code"] == "OP-000001"
        assert envelope.metadata["session_id"] == "OS-000001"
        assert envelope.metadata["mock"] is True
        assert envelope.model_version.startswith("oil_palm_")


def test_oil_palm_analyze_rejects_unknown_image_role():
    client = build_client()
    response = client.post(
        "/api/v1/oil-palm/analyze",
        data={"image_role": "leaf"},
        files={"file": ("leaf.png", PNG_BYTES, "image/png")},
    )

    assert response.status_code == 400
    body = response.json()
    assert "supported_image_roles" in body["detail"]
    assert "fruit" in body["detail"]["supported_image_roles"]


def test_oil_palm_route_exposes_registered_mock_capabilities():
    client = build_client()
    response = client.get("/api/v1/oil-palm/route")

    assert response.status_code == 200
    payload = response.json()
    assert "fruit" in payload["supported_image_roles"]
    tasks = {item["task"] for item in payload["registered_capabilities"]}
    assert {
        "a0_structure_detection",
        "ffb_maturity",
        "ganoderma_risk",
        "growth_vigor",
        "uav_tree_crown",
    } <= tasks


def test_oil_palm_a0_detect_returns_bbox_candidates_for_fruit():
    client = build_client()
    response = client.post(
        "/api/v1/oil-palm/a0/detect",
        data={
            "image_role": "fruit",
            "tree_code": "OP-000001",
            "session_id": "OS-000001",
        },
        files={"file": ("fruit.png", PNG_BYTES, "image/png")},
    )

    assert response.status_code == 200
    payload = response.json()
    envelope = PredictionEnvelope.model_validate(payload)
    assert envelope.status == "success"
    assert envelope.model_version == "oil_palm_a0_detector_mock_v1"
    assert envelope.metadata["route_status"] == "needs_user_confirmation"
    assert envelope.metadata["requires_user_confirmation"] is True
    assert len(envelope.metadata["a0_candidates"]) >= 2
    assert {item.label for item in envelope.results} == {"fruit_bunch"}
    assert all(item.geometry["type"] == "bbox" for item in envelope.results)


def test_oil_palm_a0_detect_reports_role_mismatch():
    client = build_client()
    response = client.post(
        "/api/v1/oil-palm/a0/detect",
        data={
            "image_role": "fruit",
            "mock_detect_role": "trunk_base",
        },
        files={"file": ("mismatch.png", PNG_BYTES, "image/png")},
    )

    assert response.status_code == 200
    payload = response.json()
    assert payload["metadata"]["route_status"] == "role_mismatch"
    assert payload["results"][0]["label"] == "trunk_base"


def test_a0_yolo_predictor_maps_boxes_to_candidate_envelope(tmp_path, monkeypatch):
    _install_fake_ultralytics(monkeypatch, cls_values=[0], conf_values=[0.92])
    labels = tmp_path / "labels.json"
    labels.write_text('["fruit_bunch", "trunk_base", "crown_region"]', encoding="utf-8")
    config = tmp_path / "inference_config.yaml"
    config.write_text(
        "model_version: oil_palm_a0_test_v1\n"
        "input_size: 128\n"
        "confidence_threshold: 0.5\n"
        "iou_threshold: 0.7\n"
        "max_detections: 10\n",
        encoding="utf-8",
    )
    weights = tmp_path / "best.pt"
    weights.write_bytes(b"fake weights")

    predictor = A0YoloPredictor(
        weights_path=weights,
        labels_path=labels,
        config_path=config,
    )
    payload = predictor.predict(
        _valid_png_bytes(),
        PredictorContext(
            crop="oil_palm",
            task="a0_structure_detection",
            image_role="fruit",
            tree_code="OP-000001",
        ),
    )

    envelope = PredictionEnvelope.model_validate(payload)
    assert envelope.model_version == "oil_palm_a0_test_v1"
    assert envelope.metadata["mock"] is False
    assert envelope.metadata["route_status"] == "needs_user_confirmation"
    assert envelope.metadata["requires_user_confirmation"] is True
    assert envelope.metadata["detected_roles"] == ["fruit"]
    assert envelope.results[0].label == "fruit_bunch"
    assert envelope.results[0].geometry == {
        "type": "bbox",
        "x": 0.1,
        "y": 0.2,
        "w": 0.4,
        "h": 0.6,
    }


def test_a0_yolo_predictor_reports_role_mismatch(tmp_path, monkeypatch):
    _install_fake_ultralytics(monkeypatch, cls_values=[1], conf_values=[0.91])
    labels = tmp_path / "labels.json"
    labels.write_text('["fruit_bunch", "trunk_base", "crown_region"]', encoding="utf-8")
    weights = tmp_path / "best.pt"
    weights.write_bytes(b"fake weights")

    predictor = A0YoloPredictor(weights_path=weights, labels_path=labels, config_path=None)
    payload = predictor.predict(
        _valid_png_bytes(),
        PredictorContext(
            crop="oil_palm",
            task="a0_structure_detection",
            image_role="fruit",
        ),
    )

    assert payload["metadata"]["route_status"] == "role_mismatch"
    assert payload["results"][0]["label"] == "trunk_base"
    assert payload["metadata"]["detected_roles"] == ["trunk_base"]


def test_oil_palm_hybrid_mode_registers_real_a0_when_available(
    tmp_path,
    monkeypatch,
):
    _install_fake_ultralytics(monkeypatch, cls_values=[0], conf_values=[0.9])
    labels = tmp_path / "labels.json"
    labels.write_text('["fruit_bunch", "trunk_base", "crown_region"]', encoding="utf-8")
    weights = tmp_path / "best.pt"
    weights.write_bytes(b"fake weights")

    monkeypatch.setenv("OIL_PALM_MODEL_MODE", "hybrid")
    monkeypatch.setenv("OIL_PALM_A0_MODEL_PATH", str(weights))
    monkeypatch.setenv("OIL_PALM_A0_LABELS_FILE", str(labels))
    monkeypatch.setenv(
        "OIL_PALM_GANODERMA_MODEL_PATH",
        str(tmp_path / "missing_ganoderma_weights.pth"),
    )
    monkeypatch.delenv("OIL_PALM_A0_CONFIG_FILE", raising=False)

    import importlib
    import ai_engine.crops.oil_palm.pipeline as pipeline_mod

    pipeline_mod = importlib.reload(pipeline_mod)
    pipeline = pipeline_mod.build_default_oil_palm_pipeline()

    capabilities = pipeline.registry.capabilities("oil_palm")
    a0 = [item for item in capabilities if item["task"] == "a0_structure_detection"]
    assert a0[0]["mode"] == "real"
    assert {item["mode"] for item in capabilities if item["task"] != "a0_structure_detection"} == {
        "mock"
    }


def test_ganoderma_resnet_predictor_returns_suspected_risk_envelope(
    tmp_path,
    monkeypatch,
):
    _install_fake_torchvision_resnet(monkeypatch, probabilities=[0.18, 0.82])
    labels = tmp_path / "labels.json"
    labels.write_text(
        '["healthy", "suspected_risk", "other_stress_unknown"]',
        encoding="utf-8",
    )
    metrics = tmp_path / "metrics.json"
    metrics.write_text(
        "{"
        '"model_version":"oil_palm_ganoderma_test_v1",'
        '"labels":["healthy","suspected_risk","other_stress_unknown"],'
        '"active_training_labels":["healthy","suspected_risk"],'
        '"class_order":{"0":"healthy","1":"suspected_risk"},'
        '"preprocessing":{"resize":[224,224],"normalize":{"mean":[0.485,0.456,0.406],"std":[0.229,0.224,0.225]}}'
        "}",
        encoding="utf-8",
    )
    weights = tmp_path / "best.pth"
    weights.write_bytes(b"fake weights")

    from ai_engine.crops.oil_palm.inference.ganoderma_resnet_predictor import (
        GanodermaResNetPredictor,
    )

    predictor = GanodermaResNetPredictor(
        weights_path=weights,
        labels_path=labels,
        metrics_path=metrics,
    )
    payload = predictor.predict(
        _valid_png_bytes(),
        PredictorContext(
            crop="oil_palm",
            task="ganoderma_risk",
            image_role="trunk_base",
            tree_code="OP-000001",
            session_id="OS-000001",
        ),
    )

    envelope = PredictionEnvelope.model_validate(payload)
    assert envelope.model_version == "oil_palm_ganoderma_test_v1"
    assert envelope.results[0].task == "ganoderma_risk"
    assert envelope.results[0].label == "suspected_risk"
    assert envelope.results[0].confidence == 0.82
    assert envelope.results[0].geometry == {"type": "whole_image"}
    assert envelope.metadata["mock"] is False
    assert envelope.metadata["diagnosis_status"] == "not_confirmed"
    assert envelope.metadata["user_review_status"] == "confirmed_by_default"
    assert envelope.metadata["probabilities"] == {
        "healthy": 0.18,
        "suspected_risk": 0.82,
    }


def test_oil_palm_hybrid_mode_registers_real_ganoderma_when_available(
    tmp_path,
    monkeypatch,
):
    _install_fake_torchvision_resnet(monkeypatch, probabilities=[0.74, 0.26])
    labels = tmp_path / "labels.json"
    labels.write_text(
        '["healthy", "suspected_risk", "other_stress_unknown"]',
        encoding="utf-8",
    )
    metrics = tmp_path / "metrics.json"
    metrics.write_text(
        '{"model_version":"oil_palm_ganoderma_test_v1","class_order":{"0":"healthy","1":"suspected_risk"}}',
        encoding="utf-8",
    )
    weights = tmp_path / "best.pth"
    weights.write_bytes(b"fake weights")

    monkeypatch.setenv("OIL_PALM_MODEL_MODE", "hybrid")
    monkeypatch.setenv("OIL_PALM_GANODERMA_MODEL_PATH", str(weights))
    monkeypatch.setenv("OIL_PALM_GANODERMA_LABELS_FILE", str(labels))
    monkeypatch.setenv("OIL_PALM_GANODERMA_METRICS_FILE", str(metrics))
    monkeypatch.delenv("OIL_PALM_A0_MODEL_PATH", raising=False)

    import importlib
    import ai_engine.crops.oil_palm.pipeline as pipeline_mod

    pipeline_mod = importlib.reload(pipeline_mod)
    pipeline = pipeline_mod.build_default_oil_palm_pipeline()

    capabilities = pipeline.registry.capabilities("oil_palm")
    ganoderma = [item for item in capabilities if item["task"] == "ganoderma_risk"]
    a0 = [item for item in capabilities if item["task"] == "a0_structure_detection"]
    assert ganoderma[0]["mode"] == "real"
    assert ganoderma[0]["model_version"] == "oil_palm_ganoderma_test_v1"
    assert a0[0]["mode"] == "mock"


def _valid_png_bytes() -> bytes:
    image = Image.new("RGB", (100, 100), color=(120, 130, 140))
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return buffer.getvalue()


def _install_fake_ultralytics(monkeypatch, *, cls_values, conf_values):
    fake_module = types.ModuleType("ultralytics")

    class FakeBoxes:
        xyxy = [[10.0, 20.0, 50.0, 80.0]]
        cls = cls_values
        conf = conf_values

    class FakeResult:
        boxes = FakeBoxes()

    class FakeModel:
        def __init__(self, weights_path):
            self.weights_path = weights_path

        def predict(self, **kwargs):
            return [FakeResult()]

    fake_module.YOLO = FakeModel
    monkeypatch.setitem(sys.modules, "ultralytics", fake_module)


def _install_fake_torchvision_resnet(monkeypatch, *, probabilities):
    fake_torch = types.ModuleType("torch")
    fake_torchvision = types.ModuleType("torchvision")
    fake_models = types.SimpleNamespace()
    fake_transforms = types.SimpleNamespace()

    class FakeLinear:
        def __init__(self, in_features, out_features):
            self.in_features = in_features
            self.out_features = out_features

    class FakeFunctional:
        @staticmethod
        def softmax(_logits, dim):
            assert dim == 1
            return [FakeProbabilityTensor(probabilities)]

    class FakeNoGrad:
        def __enter__(self):
            return None

        def __exit__(self, _exc_type, _exc, _traceback):
            return False

    class FakeCuda:
        @staticmethod
        def is_available():
            return False

    class FakeTensor:
        def unsqueeze(self, dim):
            assert dim == 0
            return self

        def to(self, device):
            assert device in {"cpu", "cuda"}
            return self

    class FakeProbabilityTensor:
        def __init__(self, values):
            self.values = values

        def detach(self):
            return self

        def cpu(self):
            return self

        def tolist(self):
            return self.values

    class FakeTransform:
        def __call__(self, _image):
            return FakeTensor()

    class FakeCompose:
        def __init__(self, steps):
            self.steps = steps

        def __call__(self, image):
            assert self.steps
            return FakeTensor()

    class FakeModel:
        def __init__(self):
            self.fc = types.SimpleNamespace(in_features=512)
            self.loaded_state_dict = None

        def load_state_dict(self, state_dict):
            self.loaded_state_dict = state_dict

        def to(self, device):
            assert device == "cpu"
            return self

        def eval(self):
            return self

        def __call__(self, _tensor):
            return object()

    fake_torch.nn = types.SimpleNamespace(
        Linear=FakeLinear,
        functional=FakeFunctional,
    )
    fake_torch.cuda = FakeCuda()
    fake_torch.no_grad = FakeNoGrad
    fake_torch.load = lambda _path, map_location=None: {}
    fake_models.resnet18 = lambda weights=None: FakeModel()
    fake_transforms.Compose = FakeCompose
    fake_transforms.Resize = lambda _size: FakeTransform()
    fake_transforms.ToTensor = lambda: FakeTransform()
    fake_transforms.Normalize = lambda mean, std: FakeTransform()
    fake_torchvision.models = fake_models
    fake_torchvision.transforms = fake_transforms

    monkeypatch.setitem(sys.modules, "torch", fake_torch)
    monkeypatch.setitem(sys.modules, "torchvision", fake_torchvision)
