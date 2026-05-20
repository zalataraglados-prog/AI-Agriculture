from __future__ import annotations

from io import BytesIO
from pathlib import Path

from PIL import Image, UnidentifiedImageError


class ImageLoadError(ValueError):
    """Raised when image bytes cannot be decoded."""


def load_image_from_path(image_path: str | Path) -> Image.Image:
    path = Path(image_path)
    if not path.exists():
        raise ImageLoadError(f"image file not found: {path}")
    try:
        with path.open("rb") as f:
            return load_image_from_bytes(f.read())
    except ImageLoadError:
        raise
    except OSError as exc:
        raise ImageLoadError(f"cannot read image file: {path}") from exc


def load_image_from_bytes(image_bytes: bytes) -> Image.Image:
    if not image_bytes:
        raise ImageLoadError("empty image bytes")
    try:
        with Image.open(BytesIO(image_bytes)) as img:
            return img.convert("RGB")
    except (UnidentifiedImageError, OSError) as exc:
        raise ImageLoadError("cannot decode image bytes") from exc


def validate_image_bytes(image_bytes: bytes) -> str:
    if not image_bytes:
        raise ImageLoadError("empty image bytes")
    kind = detect_image_kind(image_bytes)
    if kind is None:
        raise ImageLoadError("unsupported or invalid image bytes")
    return kind


def detect_image_kind(image_bytes: bytes) -> str | None:
    if image_bytes.startswith(b"\xff\xd8\xff"):
        return "jpeg"
    if image_bytes.startswith(b"\x89PNG\r\n\x1a\n"):
        return "png"
    if image_bytes.startswith((b"GIF87a", b"GIF89a")):
        return "gif"
    if image_bytes.startswith(b"BM"):
        return "bmp"
    return None
