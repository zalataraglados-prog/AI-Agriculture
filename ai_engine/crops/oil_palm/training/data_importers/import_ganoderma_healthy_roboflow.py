"""Importer for the Roboflow healthy oil palm dataset."""

from __future__ import annotations

import json

try:
    from .import_ganoderma_roboflow_common import (
        GanodermaRoboflowImporter,
        build_arg_parser,
        csv_int,
    )
except ImportError:  # pragma: no cover - supports direct script execution
    from import_ganoderma_roboflow_common import (  # type: ignore
        GanodermaRoboflowImporter,
        build_arg_parser,
        csv_int,
    )


DEFAULT_RAW_DIR = "datasets/oil_palm/ganoderma_risk/raw/ganoderma_healthy"
DEFAULT_SUMMARY_FILE = "datasets/oil_palm/ganoderma_risk/splits/ganoderma_healthy_summary.json"


def map_healthy_label(row: dict[str, str]) -> str:
    if csv_int(row, "unhealthy") == 1:
        return "suspected_risk"
    if csv_int(row, "healthy") == 1:
        return "healthy"
    return "healthy"


class GanodermaHealthyImporter(GanodermaRoboflowImporter):
    def __init__(
        self,
        raw_dir: str = DEFAULT_RAW_DIR,
        output_dir: str = "datasets/oil_palm/ganoderma_risk/classification",
        summary_file: str | None = DEFAULT_SUMMARY_FILE,
        seed: int = 42,
        overwrite: bool = False,
    ) -> None:
        super().__init__(
            source_name="ganoderma_healthy_roboflow",
            raw_dir=raw_dir,
            output_dir=output_dir,
            map_label=map_healthy_label,
            seed=seed,
            summary_file=summary_file,
            overwrite=overwrite,
        )


def main() -> None:
    parser = build_arg_parser(DEFAULT_RAW_DIR, DEFAULT_SUMMARY_FILE)
    args = parser.parse_args()
    importer = GanodermaHealthyImporter(
        raw_dir=args.raw_dir,
        output_dir=args.output_dir,
        summary_file=args.summary_file,
        seed=args.seed,
        overwrite=args.overwrite,
    )
    print(json.dumps(importer.convert(), indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
