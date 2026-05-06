"""Tests for oil palm model data foundation.

Validates:
- Manifest files are valid JSON with required fields
- Labels.json files are valid non-empty lists
- Metrics example files have required structure
- Real predictor skeletons raise NotImplementedError
- Data utils functions work correctly
- Pipeline defaults to mock mode with model_mode in metadata
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

# ---------------------------------------------------------------------------
# Path helpers
# ---------------------------------------------------------------------------

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DATASETS_OIL_PALM = PROJECT_ROOT / "datasets" / "oil_palm"
MODELS_OIL_PALM = PROJECT_ROOT / "models" / "oil_palm"
MANIFEST_DIR = DATASETS_OIL_PALM / "manifests"

TASKS = ["ffb_maturity", "uav_tree_crown", "ganoderma_risk", "a0_image_routing"]


# ---------------------------------------------------------------------------
# Manifest tests
# ---------------------------------------------------------------------------


class TestManifests:
    """Validate dataset manifest example files."""

    @pytest.mark.parametrize("task", TASKS)
    def test_manifest_is_valid_json(self, task: str) -> None:
        manifest_path = MANIFEST_DIR / f"{task}.example.json"
        assert manifest_path.exists(), f"Manifest not found: {manifest_path}"
        with open(manifest_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        assert isinstance(data, dict)

    @pytest.mark.parametrize("task", TASKS)
    def test_manifest_has_required_fields(self, task: str) -> None:
        manifest_path = MANIFEST_DIR / f"{task}.example.json"
        with open(manifest_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        required = ["task", "version", "label_map", "split_strategy"]
        for field in required:
            assert field in data, f"Manifest {task} missing required field: {field}"

    @pytest.mark.parametrize("task", TASKS)
    def test_manifest_has_importer_directory(self, task: str) -> None:
        manifest_path = MANIFEST_DIR / f"{task}.example.json"
        with open(manifest_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        assert "importer_directory" in data, \
            f"Manifest {task} should reference importer_directory"

    @pytest.mark.parametrize("task", TASKS)
    def test_manifest_task_field_matches_filename(self, task: str) -> None:
        manifest_path = MANIFEST_DIR / f"{task}.example.json"
        with open(manifest_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        assert data["task"] == task


# ---------------------------------------------------------------------------
# Labels tests
# ---------------------------------------------------------------------------


class TestLabels:
    """Validate model labels.json files."""

    @pytest.mark.parametrize("task", TASKS)
    def test_labels_json_is_valid(self, task: str) -> None:
        labels_path = MODELS_OIL_PALM / task / "labels.json"
        assert labels_path.exists(), f"Labels not found: {labels_path}"
        with open(labels_path, "r", encoding="utf-8") as f:
            labels = json.load(f)
        assert isinstance(labels, list)
        assert len(labels) > 0, f"Labels for {task} must not be empty"
        for label in labels:
            assert isinstance(label, str), f"Each label must be a string, got {type(label)}"

    @pytest.mark.parametrize("task", TASKS)
    def test_labels_match_manifest(self, task: str) -> None:
        """Labels.json entries must match manifest label_map values."""
        labels_path = MODELS_OIL_PALM / task / "labels.json"
        manifest_path = MANIFEST_DIR / f"{task}.example.json"

        with open(labels_path, "r", encoding="utf-8") as f:
            labels = json.load(f)
        with open(manifest_path, "r", encoding="utf-8") as f:
            manifest = json.load(f)

        manifest_labels = sorted(manifest["label_map"].values())
        file_labels = sorted(labels)
        assert manifest_labels == file_labels, (
            f"Mismatch for {task}:\n"
            f"  manifest: {manifest_labels}\n"
            f"  labels.json: {file_labels}"
        )


# ---------------------------------------------------------------------------
# Metrics tests
# ---------------------------------------------------------------------------


class TestMetrics:
    """Validate metrics example files."""

    @pytest.mark.parametrize("task", TASKS)
    def test_metrics_example_is_valid_json(self, task: str) -> None:
        metrics_path = MODELS_OIL_PALM / task / "metrics.example.json"
        assert metrics_path.exists(), f"Metrics not found: {metrics_path}"
        with open(metrics_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        assert isinstance(data, dict)

    @pytest.mark.parametrize("task", TASKS)
    def test_metrics_has_required_fields(self, task: str) -> None:
        metrics_path = MODELS_OIL_PALM / task / "metrics.example.json"
        with open(metrics_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        for field in ["model_version", "task", "metrics"]:
            assert field in data, f"Metrics {task} missing required field: {field}"

    @pytest.mark.parametrize("task", TASKS)
    def test_metrics_task_field_matches_directory(self, task: str) -> None:
        metrics_path = MODELS_OIL_PALM / task / "metrics.example.json"
        with open(metrics_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        assert data["task"] == task


# ---------------------------------------------------------------------------
# Real predictor tests
# ---------------------------------------------------------------------------


class TestRealPredictors:
    """Verify real predictor skeletons raise NotImplementedError."""

    def test_ffb_real_predictor_not_implemented(self) -> None:
        from ai_engine.crops.oil_palm.inference.real_predictors import FFBRealPredictor
        from ai_engine.common.predictors.base import PredictorContext

        p = FFBRealPredictor(model_path="dummy.pt")
        ctx = PredictorContext(crop="oil_palm", task="ffb_maturity")
        with pytest.raises(NotImplementedError):
            p.predict(b"fake_image", ctx)

    def test_uav_real_predictor_not_implemented(self) -> None:
        from ai_engine.crops.oil_palm.inference.real_predictors import UAVTileRealPredictor
        from ai_engine.common.predictors.base import PredictorContext

        p = UAVTileRealPredictor(model_path="dummy.pt")
        ctx = PredictorContext(crop="oil_palm", task="uav_tree_crown")
        with pytest.raises(NotImplementedError):
            p.predict(b"fake_image", ctx)

    def test_ganoderma_real_predictor_not_implemented(self) -> None:
        from ai_engine.crops.oil_palm.inference.real_predictors import GanodermaRealPredictor
        from ai_engine.common.predictors.base import PredictorContext

        p = GanodermaRealPredictor(model_path="dummy.pt")
        ctx = PredictorContext(crop="oil_palm", task="ganoderma_risk")
        with pytest.raises(NotImplementedError):
            p.predict(b"fake_image", ctx)

    def test_growth_real_predictor_not_implemented(self) -> None:
        from ai_engine.crops.oil_palm.inference.real_predictors import GrowthRealPredictor
        from ai_engine.common.predictors.base import PredictorContext

        p = GrowthRealPredictor(model_path="dummy.pt")
        ctx = PredictorContext(crop="oil_palm", task="growth_vigor")
        with pytest.raises(NotImplementedError):
            p.predict(b"fake_image", ctx)

    def test_a0_real_predictor_not_implemented(self) -> None:
        from ai_engine.crops.oil_palm.inference.real_predictors import A0RoutingRealPredictor
        from ai_engine.common.predictors.base import PredictorContext

        p = A0RoutingRealPredictor(model_path="dummy.pt")
        ctx = PredictorContext(crop="oil_palm", task="a0_image_routing")
        with pytest.raises(NotImplementedError):
            p.predict(b"fake_image", ctx)

    def test_real_predictors_have_mode_real(self) -> None:
        from ai_engine.crops.oil_palm.inference.real_predictors import (
            FFBRealPredictor,
            UAVTileRealPredictor,
            GanodermaRealPredictor,
            GrowthRealPredictor,
            A0RoutingRealPredictor,
        )
        for cls in [FFBRealPredictor, UAVTileRealPredictor, GanodermaRealPredictor,
                    GrowthRealPredictor, A0RoutingRealPredictor]:
            assert cls.mode == "real"


# ---------------------------------------------------------------------------
# Data utils tests
# ---------------------------------------------------------------------------


class TestDataUtils:
    """Test training common data utilities."""

    def test_load_manifest(self) -> None:
        from ai_engine.crops.oil_palm.training.common.data_utils import load_manifest

        manifest = load_manifest(MANIFEST_DIR / "ffb_maturity.example.json")
        assert manifest["task"] == "ffb_maturity"
        assert "label_map" in manifest

    def test_load_manifest_missing_file(self) -> None:
        from ai_engine.crops.oil_palm.training.common.data_utils import load_manifest

        with pytest.raises(FileNotFoundError):
            load_manifest("/nonexistent/manifest.json")

    def test_load_labels(self) -> None:
        from ai_engine.crops.oil_palm.training.common.data_utils import load_labels

        labels = load_labels(MODELS_OIL_PALM / "ffb_maturity" / "labels.json")
        assert len(labels) == 6
        assert "ripe" in labels

    def test_validate_label_map_consistent(self) -> None:
        from ai_engine.crops.oil_palm.training.common.data_utils import (
            load_manifest, validate_label_map,
        )

        manifest = load_manifest(MANIFEST_DIR / "ffb_maturity.example.json")
        assert validate_label_map(
            manifest,
            MODELS_OIL_PALM / "ffb_maturity" / "labels.json",
        )


# ---------------------------------------------------------------------------
# Metrics utils tests
# ---------------------------------------------------------------------------


class TestMetricsUtils:
    """Test training common metrics utilities."""

    def test_save_and_load_metrics(self, tmp_path: Path) -> None:
        from ai_engine.crops.oil_palm.training.common.metrics_utils import (
            save_metrics, load_metrics,
        )

        metrics = {
            "model_version": "test_v1",
            "task": "ffb_maturity",
            "metrics": {"mAP50": 0.75, "precision": 0.8},
        }
        out = save_metrics(metrics, tmp_path / "metrics.json")
        loaded = load_metrics(out)
        assert loaded["model_version"] == "test_v1"
        assert loaded["metrics"]["mAP50"] == 0.75

    def test_compare_metrics(self, tmp_path: Path) -> None:
        from ai_engine.crops.oil_palm.training.common.metrics_utils import (
            save_metrics, compare_metrics,
        )

        old = {"model_version": "v1", "metrics": {"mAP50": 0.70, "precision": 0.75}}
        new = {"model_version": "v2", "metrics": {"mAP50": 0.80, "precision": 0.72}}

        save_metrics(old, tmp_path / "old.json")
        save_metrics(new, tmp_path / "new.json")

        result = compare_metrics(tmp_path / "old.json", tmp_path / "new.json")
        assert result["old_version"] == "v1"
        assert result["new_version"] == "v2"
        assert result["changes"]["mAP50"]["improved"] is True
        assert result["changes"]["precision"]["improved"] is False


# ---------------------------------------------------------------------------
# Pipeline mode tests
# ---------------------------------------------------------------------------


class TestPipelineMode:
    """Test that pipeline correctly reflects model mode in metadata."""

    def test_pipeline_mock_mode_is_default(self) -> None:
        """Default build (no env override) should produce mock mode."""
        import os
        # Ensure env var is not set or is 'mock'
        old = os.environ.pop("OIL_PALM_MODEL_MODE", None)
        try:
            # Re-import to pick up fresh module-level env read
            import importlib
            import ai_engine.crops.oil_palm.pipeline as pipeline_mod
            importlib.reload(pipeline_mod)
            p = pipeline_mod.build_default_oil_palm_pipeline()
            assert p.model_mode == "mock"
        finally:
            if old is not None:
                os.environ["OIL_PALM_MODEL_MODE"] = old

    def test_pipeline_metadata_includes_model_mode(self) -> None:
        """analyze() output metadata should contain model_mode field."""
        from ai_engine.crops.oil_palm.pipeline import build_default_oil_palm_pipeline

        pipeline = build_default_oil_palm_pipeline()
        result = pipeline.analyze(
            image_bytes=b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR",
            image_role="fruit",
        )
        assert "model_mode" in result["metadata"]
        assert result["metadata"]["model_mode"] == "mock"
