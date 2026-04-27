"""Image adapter 鈥?L2 firewall between external I/O and pure inference.

Every image that enters the core engine MUST pass through one of
the ``load_image_*`` functions below.  This guarantees:

1. A consistent ``PIL.Image.Image`` in RGB mode.
2. Validated, non-empty data 鈥?the core layer can trust its input.
3. Logging of image metadata *without* leaking sensitive file paths.

Architecture constraint
-----------------------
This module belongs to **L2 (Adapter Layer)**.  It may use ``PIL``
and standard-library I/O but MUST NOT import ``fastapi`` or any
web-framework symbol.  The FastAPI ``UploadFile`` 鉃?bytes conversion
happens in L1 (API layer), which then delegates to
:func:`load_image_from_bytes` here.
"""

import io
import logging
from pathlib import Path

from PIL import Image, UnidentifiedImageError

logger = logging.getLogger(__name__)


# ------------------------------------------------------------------
# Custom exception
# ------------------------------------------------------------------

class ImageLoadError(Exception):
    """Raised when an image cannot be loaded or decoded.

    The message is intentionally kept user-safe 鈥?it references
    only the file *name* (not the full path) to avoid leaking
    server-side directory structure.
    """


# ------------------------------------------------------------------
# Public helpers
# ------------------------------------------------------------------

def load_image_from_path(image_path: str) -> Image.Image:
    """Load an image from a local filesystem path.

    Parameters
    ----------
    image_path : str
        Absolute or relative path to a JPEG / PNG image file.

    Returns
    -------
    PIL.Image.Image
        The decoded image in RGB mode.

    Raises
    ------
    ImageLoadError
        If the file does not exist or cannot be decoded.
    """
    path = Path(image_path)

    if not path.exists():
        raise ImageLoadError(f"Image file not found: {path.name}")

    try:
        img = Image.open(path).convert("RGB")
        logger.info("Loaded image from path: %s (%d脳%d)", path.name, img.width, img.height)
        return img
    except UnidentifiedImageError:
        raise ImageLoadError(f"Cannot decode image: {path.name}")
    except Exception as exc:
        raise ImageLoadError(f"Failed to open image {path.name}: {exc}") from exc


def load_image_from_bytes(data: bytes) -> Image.Image:
    """Load an image from raw bytes (e.g. HTTP request body).

    Parameters
    ----------
    data : bytes
        The raw image payload.

    Returns
    -------
    PIL.Image.Image
        The decoded image in RGB mode.

    Raises
    ------
    ImageLoadError
        If *data* is empty or cannot be decoded.
    """
    if not data:
        raise ImageLoadError("Empty image data received")

    try:
        img = Image.open(io.BytesIO(data)).convert("RGB")
        logger.info("Loaded image from bytes (%d脳%d)", img.width, img.height)
        return img
    except UnidentifiedImageError:
        raise ImageLoadError("Cannot decode image from provided bytes")
    except Exception as exc:
        raise ImageLoadError(f"Failed to decode image bytes: {exc}") from exc
