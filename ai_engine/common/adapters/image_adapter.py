from __future__ import annotations


class ImageLoadError(ValueError):
    """Raised when image bytes cannot be decoded."""


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
