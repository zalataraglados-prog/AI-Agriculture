"""Prepare the UAV tree crown YOLO dataset from a Roboflow export."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from ai_engine.crops.oil_palm.training.data_importers.import_uav_roboflow_yolo import (
    UAVRoboflowYoloImporter,
)

PROJECT_ROOT = Path(__file__).resolve().parents[5]
DEFAULT_OUTPUT_ROOT = PROJECT_ROOT / "datasets" / "oil_palm" / "uav_tree_crown"
DEFAULT_SOURCE_ROOT = DEFAULT_OUTPUT_ROOT / "raw"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Convert UAV Roboflow YOLO exports into project YOLO format.",
    )
    parser.add_argument(
        "--source-root",
        default=os.environ.get("OIL_PALM_UAV_SOURCE_ROOT", str(DEFAULT_SOURCE_ROOT)),
        help="Directory containing the Roboflow YOLO train export.",
    )
    parser.add_argument(
        "--output-root",
        default=str(DEFAULT_OUTPUT_ROOT),
        help="Project UAV dataset root.",
    )
    parser.add_argument(
        "--dataset-version",
        default="roboflow_uav_tree_crown_2026_05_26",
        help="Version string written to manifest and metadata.",
    )
    parser.add_argument("--seed", type=int, default=42, help="Deterministic split seed.")
    parser.add_argument(
        "--split-grouping",
        choices=["filename", "mission_hint"],
        default="filename",
        help="Best-effort split grouping strategy when mission IDs are unavailable.",
    )
    parser.add_argument(
        "--copy-raw",
        action="store_true",
        help="Copy the source export into datasets/oil_palm/uav_tree_crown/raw/.",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Regenerate existing YOLO/raw outputs inside the UAV dataset root.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Parse and summarize without writing dataset files.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    importer = UAVRoboflowYoloImporter(
        raw_dir=args.source_root,
        output_dir=args.output_root,
        dataset_version=args.dataset_version,
        seed=args.seed,
        split_grouping=args.split_grouping,
        copy_raw=args.copy_raw,
        overwrite=args.overwrite,
        dry_run=args.dry_run,
    )
    result = importer.convert()
    summary = importer.last_summary.as_dict() if importer.last_summary else {}

    print(json.dumps({
        "status": "ok",
        "dry_run": args.dry_run,
        "result": {
            "source_name": result.source_name,
            "task": result.task,
            "images_processed": result.images_processed,
            "images_skipped": result.images_skipped,
            "labels_mapped": result.labels_mapped,
            "output_dir": result.output_dir,
        },
        "summary": summary,
    }, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
