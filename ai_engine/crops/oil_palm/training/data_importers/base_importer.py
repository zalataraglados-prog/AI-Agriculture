"""Base class and interface for oil palm data importers.

Every importer must subclass BaseImporter and implement the convert() method.
This ensures all data sources, regardless of their original format, produce a
unified output that the training pipeline can consume directly.
"""

from __future__ import annotations

import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path

logger = logging.getLogger(__name__)

SENTINEL_FILENAMES = {".gitkeep", ".gitignore", ".DS_Store", "Thumbs.db"}


@dataclass
class ImportResult:
    """Result summary from a data import operation."""

    source_name: str
    task: str
    images_processed: int = 0
    images_skipped: int = 0
    labels_mapped: dict[str, int] = field(default_factory=dict)
    output_dir: str = ""
    errors: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)


class BaseImporter(ABC):
    """Base class for all oil palm data importers.

    Subclasses must implement:
        - convert(): Transform raw data into the unified processed format.

    The importer does not modify raw data. It reads from raw_dir and writes to
    output_dir, applying label mapping and format conversion as needed.
    """

    def __init__(
        self,
        raw_dir: str | Path,
        output_dir: str | Path,
        label_map: dict[str, str] | None = None,
    ) -> None:
        """Initialize the importer.

        Args:
            raw_dir: Path to the raw data directory for this source.
            output_dir: Path to write converted output.
            label_map: Optional source-label to internal-label mapping.
        """
        self.raw_dir = Path(raw_dir)
        self.output_dir = Path(output_dir)
        self.label_map = label_map or {}

    def map_label(self, source_label: str) -> str | None:
        """Map a source dataset label to the internal training label."""
        if not self.label_map:
            return source_label
        return self.label_map.get(source_label)

    @abstractmethod
    def convert(self) -> ImportResult:
        """Convert raw data into unified processed format."""

    def validate_raw_dir(self) -> bool:
        """Check that the raw directory exists and contains real data entries.

        Sentinel files such as .gitkeep keep the directory tracked in Git, but
        they do not count as dataset samples.
        """
        if not self.raw_dir.exists():
            raise FileNotFoundError(f"Raw directory not found: {self.raw_dir}")
        data_entries = [
            entry
            for entry in self.raw_dir.iterdir()
            if entry.name not in SENTINEL_FILENAMES
        ]
        if not data_entries:
            raise ValueError(f"Raw directory is empty: {self.raw_dir}")
        return True
