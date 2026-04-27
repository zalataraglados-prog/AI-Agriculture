"""Abstract base class for all crop/disease prediction models.

To add a new crop (e.g. corn, oil-palm), create a subclass of
``BasePredictor`` and implement :meth:`predict` and
:meth:`get_model_info`.  No changes to ``main.py`` or routing
logic should be necessary 鈥?new models are loaded via
configuration, not hard-coded branches.

Architecture constraint
-----------------------
This module belongs to **L3 (Core Engine Layer)**.  It MUST NOT
import ``fastapi``, ``starlette``, or any web-framework symbol.
"""

import logging
from abc import ABC, abstractmethod
from typing import Any, Dict

from PIL import Image

logger = logging.getLogger(__name__)


class BasePredictor(ABC):
    """Contract that every inference model must fulfil.

    Subclasses are free to choose their own DL framework (PyTorch,
    ONNX-Runtime, TensorRT, etc.) as long as they honour the
    ``predict`` / ``get_model_info`` interface.
    """

    # ------------------------------------------------------------------
    # Abstract interface
    # ------------------------------------------------------------------

    @abstractmethod
    def predict(self, image: Image.Image, top_k: int = 3) -> Dict[str, Any]:
        """Run inference on a single image.

        Parameters
        ----------
        image : PIL.Image.Image
            An RGB image 鈥?the adapter layer is responsible for
            decoding and colour-space conversion *before* this
            method is called.
        top_k : int, optional
            Number of top predictions to return (default 3).

        Returns
        -------
        dict
            A dictionary whose keys match the fields defined in
            ``service.schemas.prediction.PredictionResult``:

            - ``predicted_class`` (str)
            - ``confidence`` (float)
            - ``topk`` (list[dict])
            - ``model_version`` (str)
            - ``metadata`` (dict)   鈥?reserved for future LLM advice
            - ``geometry`` (dict | None) 鈥?reserved for BBox / Mask
        """
        ...

    @abstractmethod
    def get_model_info(self) -> Dict[str, str]:
        """Return human-readable model metadata.

        Expected keys (at minimum):
        ``model_name``, ``model_version``, ``architecture``,
        ``num_classes``.
        """
        ...
