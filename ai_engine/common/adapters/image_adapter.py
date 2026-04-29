from __future__ import annotations

import imghdr


class ImageLoadError(ValueError):
    """Raised when image bytes cannot be decoded."""


def validate_image_bytes(image_bytes: bytes) -> str:
    if not image_bytes:
        raise ImageLoadError("empty image bytes")
    kind = imghdr.what(None, h=image_bytes)
    if kind is None:
        raise ImageLoadError("unsupported or invalid image bytes")
    return kind
