"""Prepare the A0 image routing YOLO dataset from Roboflow COCO exports."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from ai_engine.crops.oil_palm.training.data_importers.import_a0_roboflow_coco import (
    A0RoboflowCocoImporter,
)

PROJECT_ROOT = Path(__file__).resolve().parents[5]
DEFAULT_OUTPUT_ROOT = PROJECT_ROOT / "datasets" / "oil_palm" / "a0_image_routing"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Convert A0 Roboflow COCO exports into project YOLO format.",
    )
    parser.add_argument(
        "--source-root",
        default=r"E:\a0",
        help="Directory containing crown_region, fruit_bunch, and trunk_base COCO exports.",
    )
    parser.add_argument(
        "--output-root",
        default=str(DEFAULT_OUTPUT_ROOT),
        help="Project A0 dataset root.",
    )
    parser.add_argument(
        "--dataset-version",
        default="roboflow_a0_2026_05_17",
        help="Version string written to manifest and metadata.",
    )
    parser.add_argument("--seed", type=int, default=42, help="Deterministic split seed.")
    parser.add_argument(
        "--copy-raw",
        action="store_true",
        help="Copy the source exports into datasets/oil_palm/a0_image_routing/raw/.",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Regenerate existing YOLO/raw outputs inside the A0 dataset root.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Parse and summarize without writing dataset files.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    importer = A0RoboflowCocoImporter(
        raw_dir=args.source_root,
        output_dir=args.output_root,
        dataset_version=args.dataset_version,
        seed=args.seed,
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
