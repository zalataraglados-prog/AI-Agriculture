import logging
from typing import Any, Dict
from PIL import Image
from ai_engine.common.base_predictor import BasePredictor

logger = logging.getLogger(__name__)

class OilPalmPredictor(BasePredictor):
    """
    Mock predictor for Oil Palm fruit bunch detection.
    Will be replaced by YOLOv8 implementation in Phase 2.
    """
    def __init__(self, model_path: str = None):
        self._model_version = "oil_palm_mock_v0.1.0"
        logger.info("Initializing OilPalmPredictor (Mock)")

    def predict(self, image: Image.Image, **kwargs) -> Dict[str, Any]:
        logger.info("Running mock prediction for Oil Palm")
        return {
            "predicted_class": "RipeBunch",
            "confidence": 0.98,
            "model_version": self._model_version,
            "geometry": {
                "type": "box",
                "coords": [100, 100, 250, 250]
            },
            "metadata": {
                "advice": "Ready for harvest."
            }
        }

    def get_model_info(self) -> Dict[str, str]:
        return {
            "model_name": "OilPalmPredictor",
            "model_version": self._model_version,
            "architecture": "Mock/YOLOv8-Placeholder"
        }
