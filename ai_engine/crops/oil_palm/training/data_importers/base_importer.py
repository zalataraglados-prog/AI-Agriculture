"""Base class and interface for oil palm data importers.

Every importer must subclass BaseImporter and implement the convert() method.
This ensures all data sources — regardless of their original format — produce
a unified output that the training pipeline can consume directly.
"""

from __future__ import annotations

import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


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

    The importer does NOT modify raw data. It reads from raw_dir and writes
    to output_dir, applying label mapping and format conversion as needed.
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
            output_dir: Path to write converted output (processed/ or yolo/ or classification/).
            label_map: Optional mapping from source labels to project internal labels.
                       If None, assumes source labels already match internal labels.
        """
        self.raw_dir = Path(raw_dir)
        self.output_dir = Path(output_dir)
        self.label_map = label_map or {}

    def map_label(self, source_label: str) -> str | None:
        """Map a source dataset label to the internal training label.

        Args:
            source_label: Label from the source dataset.

        Returns:
            Internal label string, or None if the source label should be skipped.
        """
        if not self.label_map:
            return source_label
        return self.label_map.get(source_label)

    @abstractmethod
    def convert(self) -> ImportResult:
        """Convert raw data into unified processed format.

        Returns:
            ImportResult summarizing what was converted.
        """

    def validate_raw_dir(self) -> bool:
        """Check that the raw directory exists and is not empty.

        Returns:
            True if valid.

        Raises:
            FileNotFoundError: If raw_dir does not exist.
            ValueError: If raw_dir is empty.
        """
        if not self.raw_dir.exists():
            raise FileNotFoundError(f"Raw directory not found: {self.raw_dir}")
        if not any(self.raw_dir.iterdir()):
            raise ValueError(f"Raw directory is empty: {self.raw_dir}")
        return True
