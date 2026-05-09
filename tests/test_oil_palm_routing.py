from __future__ import annotations

from fastapi import FastAPI
from fastapi.testclient import TestClient

from ai_engine.common.schemas.prediction import PredictionEnvelope
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
