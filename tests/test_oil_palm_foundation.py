"""Tests for oil palm model data foundation.

Validates:
- Manifest files are valid JSON with required fields
- Labels.json files are valid non-empty lists
- Metrics example files have required structure
- Data utils functions work correctly
- Pipeline mode switching stays safe in the foundation branch
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
TEST_OUTPUT_DIR = PROJECT_ROOT / "target" / "test_oil_palm_foundation"

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

    def test_ganoderma_v1_manifest_records_handoff_counts(self) -> None:
        manifest_path = MANIFEST_DIR / "ganoderma_risk.json"
        assert manifest_path.exists(), "Ganoderma v1 manifest should be recorded"
        with open(manifest_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        assert data["task"] == "ganoderma_risk"
        assert data["version"] == "v1"
        assert data["class_order"] == {"0": "healthy", "1": "suspected_risk"}
        assert data["active_training_labels"] == ["healthy", "suspected_risk"]
        assert data["reserved_labels"] == ["other_stress_unknown"]

        counts = data["counts"]
        assert counts["train"] == {"healthy": 414, "suspected_risk": 337, "total": 751}
        assert counts["val"] == {"healthy": 88, "suspected_risk": 70, "total": 158}
        assert counts["test"] == {"healthy": 92, "suspected_risk": 75, "total": 167}
        assert counts["total"] == {"healthy": 594, "suspected_risk": 482, "all": 1076}
        assert counts["train"]["total"] + counts["val"]["total"] + counts["test"]["total"] == 1076


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

    def test_a0_yolo_labels_do_not_include_unknown(self) -> None:
        labels_path = MODELS_OIL_PALM / "a0_image_routing" / "labels.json"
        manifest_path = MANIFEST_DIR / "a0_image_routing.example.json"

        with open(labels_path, "r", encoding="utf-8") as f:
            labels = json.load(f)
        with open(manifest_path, "r", encoding="utf-8") as f:
            manifest = json.load(f)

        assert labels == ["fruit_bunch", "trunk_base", "crown_region"]
        assert "unknown" not in labels
        assert manifest["internal_training_standard"]["format"] == "yolo_detection"
        assert manifest["internal_training_standard"]["annotation_type"] == "bbox"


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

    @pytest.mark.parametrize("task", TASKS)
    def test_trained_metrics_json_has_required_fields_when_present(self, task: str) -> None:
        metrics_path = MODELS_OIL_PALM / task / "metrics.json"
        if not metrics_path.exists():
            pytest.skip(f"No trained metrics recorded for {task}")

        with open(metrics_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        for field in [
            "model_version",
            "framework",
            "task",
            "dataset_version",
            "dataset_manifest",
            "metrics",
            "training",
            "inference",
            "notes",
        ]:
            assert field in data, f"Trained metrics {task} missing required field: {field}"
        assert data["task"] == task
        assert data["dataset_manifest"].endswith(".json")
        manifest_path = PROJECT_ROOT / data["dataset_manifest"]
        assert manifest_path.exists(), f"Metrics {task} references missing manifest"
        assert isinstance(data["metrics"], dict)
        assert isinstance(data["training"], dict)
        assert isinstance(data["inference"], dict)

    def test_ganoderma_trained_metrics_record_v1_contract(self) -> None:
        metrics_path = MODELS_OIL_PALM / "ganoderma_risk" / "metrics.json"
        with open(metrics_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        assert data["dataset_version"] == "v1"
        assert data["dataset_manifest"] == "datasets/oil_palm/manifests/ganoderma_risk.json"
        assert data["class_order"] == {"0": "healthy", "1": "suspected_risk"}
        assert data["active_training_labels"] == ["healthy", "suspected_risk"]
        assert "other_stress_unknown" not in data["active_training_labels"]
        assert data["runtime_interpretation"]["diagnosis_policy"].endswith(
            "not agronomic diagnosis."
        )

        training = data["training"]
        assert training["dataset_sizes"] == {
            "train": 751,
            "val": 158,
            "test": 167,
            "total": 1076,
        }
        assert training["split_class_counts"]["train"] == {
            "healthy": 414,
            "suspected_risk": 337,
        }
        assert training["split_class_counts"]["val"] == {
            "healthy": 88,
            "suspected_risk": 70,
        }
        assert training["split_class_counts"]["test"] == {
            "healthy": 92,
            "suspected_risk": 75,
        }
        assert training["class_weights"] == {
            "healthy": 0.9263,
            "suspected_risk": 1.0864,
        }

        assert data["metrics"]["source"] == "test_set"
        assert data["metrics"]["best_val_accuracy"] == 0.9684
        assert data["metrics"]["per_class"]["suspected_risk"]["false_negative_count"] == 6

    def test_a0_trained_metrics_are_recorded(self) -> None:
        metrics_path = MODELS_OIL_PALM / "a0_image_routing" / "metrics.json"
        assert metrics_path.exists(), "Trained A0 metrics.json should be recorded"
        with open(metrics_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        assert data["model_version"] == "oil_palm_a0_yolo_structure_detector_v1"
        assert data["task"] == "a0_image_routing"
        assert data["dataset_version"] == "roboflow_a0_2026_05_17"
        assert data["labels"] == ["fruit_bunch", "trunk_base", "crown_region"]
        assert data["metrics"]["best_epoch"] == 76
        assert data["metrics"]["mAP50"] > 0
        assert "unknown" not in data["labels"]

    def test_a0_inference_config_points_to_ignored_run_artifact(self) -> None:
        import yaml

        config_path = MODELS_OIL_PALM / "a0_image_routing" / "inference_config.yaml"
        assert config_path.exists(), "A0 inference_config.yaml should exist"
        with open(config_path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)

        assert data["model_version"] == "oil_palm_a0_yolo_structure_detector_v1"
        assert data["task"] == "a0_image_routing"
        assert data["labels_file"].endswith("labels.json")
        assert data["weights"].endswith("runs/a0_yolo_structure_detector_v1/weights/best.pt")
        assert data["input_size"] == 960


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
# Importer tests
# ---------------------------------------------------------------------------


class TestBaseImporter:
    """Test shared importer safeguards."""

    def _build_importer(self, raw_dir: Path):
        from ai_engine.crops.oil_palm.training.data_importers.base_importer import (
            BaseImporter,
            ImportResult,
        )

        class DummyImporter(BaseImporter):
            def convert(self) -> ImportResult:
                return ImportResult(source_name="dummy", task="ffb_maturity")

        return DummyImporter(raw_dir=raw_dir, output_dir=TEST_OUTPUT_DIR / "dummy_out")

    def test_validate_raw_dir_ignores_gitkeep_only_directory(self) -> None:
        raw_dir = TEST_OUTPUT_DIR / "raw_gitkeep_only"
        raw_dir.mkdir(parents=True, exist_ok=True)
        (raw_dir / ".gitkeep").write_text("", encoding="utf-8")

        importer = self._build_importer(raw_dir)
        with pytest.raises(ValueError, match="Raw directory is empty"):
            importer.validate_raw_dir()

    def test_validate_raw_dir_accepts_real_data_with_sentinel(self) -> None:
        raw_dir = TEST_OUTPUT_DIR / "raw_with_data"
        raw_dir.mkdir(parents=True, exist_ok=True)
        (raw_dir / ".gitkeep").write_text("", encoding="utf-8")
        (raw_dir / "sample.jpg").write_bytes(b"fake image bytes")

        importer = self._build_importer(raw_dir)
        assert importer.validate_raw_dir() is True


class TestGanodermaRoboflowImporters:
    """Test Ganoderma Roboflow importers and active-label safeguards."""

    def _write_roboflow_csv(
        self,
        raw_dir: Path,
        header: str,
        rows: list[str],
        image_names: list[str],
    ) -> None:
        train_dir = raw_dir / "train"
        train_dir.mkdir(parents=True)
        (train_dir / "_classes.csv").write_text(
            "\n".join([header, *rows]) + "\n",
            encoding="utf-8",
        )
        for image_name in image_names:
            (train_dir / image_name).write_bytes(b"fake image bytes")

    def test_importers_append_active_classes_without_reserved_directory(
        self, tmp_path: Path
    ) -> None:
        from ai_engine.crops.oil_palm.training.data_importers.import_ganoderma_healthy_roboflow import (
            GanodermaHealthyImporter,
        )
        from ai_engine.crops.oil_palm.training.data_importers.import_ganoderma_infected_roboflow import (
            GanodermaInfectedImporter,
        )

        infected_raw = tmp_path / "ganoderma_infected"
        healthy_raw = tmp_path / "ganoderma_healthy"
        output_dir = tmp_path / "classification"

        self._write_roboflow_csv(
            infected_raw,
            "filename,Ganoderma,Ganoderma Fungus",
            ["infected.jpg,1,0", "control.jpg,0,0"],
            ["infected.jpg", "control.jpg"],
        )
        self._write_roboflow_csv(
            healthy_raw,
            "filename,healthy,unhealthy",
            ["healthy.jpg,1,0", "risk.jpg,0,1"],
            ["healthy.jpg", "risk.jpg"],
        )

        infected_summary = GanodermaInfectedImporter(
            raw_dir=str(infected_raw),
            output_dir=str(output_dir),
            summary_file=str(tmp_path / "infected_summary.json"),
            seed=42,
            overwrite=True,
        ).convert()
        healthy_summary = GanodermaHealthyImporter(
            raw_dir=str(healthy_raw),
            output_dir=str(output_dir),
            summary_file=str(tmp_path / "healthy_summary.json"),
            seed=42,
        ).convert()

        assert infected_summary["active_labels"] == ["healthy", "suspected_risk"]
        assert healthy_summary["active_labels"] == ["healthy", "suspected_risk"]
        assert not any(output_dir.glob("*/other_stress_unknown"))
        copied = list(output_dir.glob("*/*/*.jpg"))
        assert len(copied) == 4

    def test_ganoderma_training_active_labels_are_project_label_subset(self) -> None:
        from ai_engine.crops.oil_palm.training.ganoderma_risk.train import (
            validate_active_labels,
        )

        project_labels = ["healthy", "suspected_risk", "other_stress_unknown"]
        validate_active_labels(project_labels, ["healthy", "suspected_risk"])
        with pytest.raises(ValueError, match="not in project labels"):
            validate_active_labels(project_labels, ["confirmed_ganoderma"])


# ---------------------------------------------------------------------------
# Metrics utils tests
# ---------------------------------------------------------------------------


class TestMetricsUtils:
    """Test training common metrics utilities."""

    def test_save_and_load_metrics(self) -> None:
        from ai_engine.crops.oil_palm.training.common.metrics_utils import (
            save_metrics, load_metrics,
        )

        out_dir = TEST_OUTPUT_DIR / "save_and_load_metrics"
        out_dir.mkdir(parents=True, exist_ok=True)
        metrics = {
            "model_version": "test_v1",
            "task": "ffb_maturity",
            "metrics": {"mAP50": 0.75, "precision": 0.8},
        }
        out = save_metrics(metrics, out_dir / "metrics.json")
        loaded = load_metrics(out)
        assert loaded["model_version"] == "test_v1"
        assert loaded["metrics"]["mAP50"] == 0.75

    def test_compare_metrics(self) -> None:
        from ai_engine.crops.oil_palm.training.common.metrics_utils import (
            save_metrics, compare_metrics,
        )

        out_dir = TEST_OUTPUT_DIR / "compare_metrics"
        out_dir.mkdir(parents=True, exist_ok=True)
        old = {"model_version": "v1", "metrics": {"mAP50": 0.70, "precision": 0.75}}
        new = {"model_version": "v2", "metrics": {"mAP50": 0.80, "precision": 0.72}}

        save_metrics(old, out_dir / "old.json")
        save_metrics(new, out_dir / "new.json")

        result = compare_metrics(out_dir / "old.json", out_dir / "new.json")
        assert result["old_version"] == "v1"
        assert result["new_version"] == "v2"
        assert result["changes"]["mAP50"]["improved"] is True
        assert result["changes"]["precision"]["improved"] is False


# ---------------------------------------------------------------------------
# Pipeline mode tests
# ---------------------------------------------------------------------------


class TestPipelineMode:
    """Test foundation-safe oil palm model modes."""

    def _reload_pipeline(self, monkeypatch: pytest.MonkeyPatch, mode: str | None = None):
        import importlib
        import ai_engine.crops.oil_palm.pipeline as pipeline_mod

        if mode is None:
            monkeypatch.delenv("OIL_PALM_MODEL_MODE", raising=False)
        else:
            monkeypatch.setenv("OIL_PALM_MODEL_MODE", mode)

        return importlib.reload(pipeline_mod)

    def test_pipeline_mock_mode_is_default(self, monkeypatch: pytest.MonkeyPatch) -> None:
        """Default build (no env override) should produce mock mode."""
        pipeline_mod = self._reload_pipeline(monkeypatch)
        p = pipeline_mod.build_default_oil_palm_pipeline()
        assert p.model_mode == "mock"

    def test_pipeline_metadata_includes_model_mode(self, monkeypatch: pytest.MonkeyPatch) -> None:
        """analyze() output metadata should contain model_mode field."""
        pipeline_mod = self._reload_pipeline(monkeypatch)
        pipeline = pipeline_mod.build_default_oil_palm_pipeline()
        result = pipeline.analyze(
            image_bytes=b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR",
            image_role="fruit",
        )
        assert "model_mode" in result["metadata"]
        assert result["metadata"]["model_mode"] == "mock"

    def test_invalid_confidence_threshold_does_not_break_import(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Future threshold parsing must not happen at module import time."""
        monkeypatch.setenv("OIL_PALM_CONFIDENCE_THRESHOLD", "")
        pipeline_mod = self._reload_pipeline(monkeypatch)

        pipeline = pipeline_mod.build_default_oil_palm_pipeline()
        assert pipeline.model_mode == "mock"
        assert pipeline_mod.get_oil_palm_confidence_threshold() == 0.5

    def test_pipeline_hybrid_mode_safely_uses_mock_predictors(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Hybrid mode falls back safely when real assets are unavailable."""
        monkeypatch.setenv(
            "OIL_PALM_FFB_MODEL_PATH",
            str(MODELS_OIL_PALM / "ffb_maturity" / "labels.json"),
        )
        monkeypatch.setenv(
            "OIL_PALM_A0_MODEL_PATH",
            str(MODELS_OIL_PALM / "a0_image_routing" / "missing_best.pt"),
        )
        monkeypatch.setenv(
            "OIL_PALM_GANODERMA_MODEL_PATH",
            str(MODELS_OIL_PALM / "ganoderma_risk" / "missing_best.pth"),
        )
        pipeline_mod = self._reload_pipeline(monkeypatch, "hybrid")

        pipeline = pipeline_mod.build_default_oil_palm_pipeline()
        assert pipeline.model_mode == "hybrid"
        capabilities = pipeline.registry.capabilities("oil_palm")
        assert {item["mode"] for item in capabilities} == {"mock"}
        assert {item["task"] for item in capabilities} == {
            "a0_structure_detection",
            "ffb_maturity",
            "ganoderma_risk",
            "growth_vigor",
            "uav_tree_crown",
        }

        result = pipeline.analyze(
            image_bytes=b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR",
            image_role="fruit",
        )
        assert result["metadata"]["model_mode"] == "hybrid"
        assert "safe mock fallback" in result["metadata"]["model_mode_note"]
        assert result["model_version"].endswith("_mock_v1")

    def test_pipeline_real_mode_fails_fast(self, monkeypatch: pytest.MonkeyPatch) -> None:
        """Real mode requires configured A0 runtime assets."""
        monkeypatch.setenv(
            "OIL_PALM_A0_MODEL_PATH",
            str(MODELS_OIL_PALM / "a0_image_routing" / "missing_best.pt"),
        )
        pipeline_mod = self._reload_pipeline(monkeypatch, "real")

        with pytest.raises(RuntimeError, match="A0 YOLO predictor"):
            pipeline_mod.build_default_oil_palm_pipeline()

    def test_a0_structure_detector_is_registered_as_mock(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """A0 is registered as a mock structure detector, not a real model."""
        pipeline_mod = self._reload_pipeline(monkeypatch)
        pipeline = pipeline_mod.build_default_oil_palm_pipeline()

        capabilities = pipeline.registry.capabilities("oil_palm")
        assert any(
            item["task"] == "a0_structure_detection" and item["mode"] == "mock"
            for item in capabilities
        )
