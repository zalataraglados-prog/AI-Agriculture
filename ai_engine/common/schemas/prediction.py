"""Pydantic models that define the AI inference contract.

These schemas are the **single source of truth** for the JSON
structure exchanged with the Rust cloud backend.

Design decisions
----------------
* ``PredictionResponse.results`` is always a ``List`` even when
  there is only one image —this avoids a breaking schema change
  when batch inference is introduced later.
* Every ``PredictionResult`` carries a ``metadata`` dict and an
  optional ``geometry`` dict, reserved for Phase 2/3 features
  (LLM advice, bounding-box / mask overlays).
"""

from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field


# ------------------------------------------------------------------
# Building blocks
# ------------------------------------------------------------------

class PredictionItem(BaseModel):
    """A single class label and its predicted probability."""

    label: str
    score: float


# ------------------------------------------------------------------
# Per-image result
# ------------------------------------------------------------------

class PredictionResult(BaseModel):
    """Complete inference result for one image.

    Attributes
    ----------
    predicted_class : str
        The top-1 predicted class name.
    confidence : float
        Probability of the top-1 class, in [0, 1].
    topk : list[PredictionItem]
        Top-k predictions sorted by descending score.
    model_version : str
        Semantic version tag of the model that produced the result.
    metadata : dict
        Reserved —will carry LLM-generated advice in Phase 3.
    geometry : dict or None
        Reserved —will carry bounding-box / mask data in Phase 2.
    """

    predicted_class: str
    confidence: float
    topk: List[PredictionItem]
    model_version: str
    metadata: Dict[str, Any] = Field(default_factory=dict)
    geometry: Optional[Dict[str, Any]] = None

    model_config = {
        "protected_namespaces": ()
    }


# ------------------------------------------------------------------
# Top-level response envelope
# ------------------------------------------------------------------

class PredictionResponse(BaseModel):
    """Envelope returned by ``POST /api/v1/predict``.

    Wraps one or more ``PredictionResult`` objects.
    """

    status: str = "success"
    results: List[PredictionResult]
    metadata: Dict[str, Any] = Field(default_factory=dict)


class ErrorResponse(BaseModel):
    """Returned when inference fails gracefully.

    The API layer MUST catch exceptions raised by the core engine
    and convert them into this format —never expose a raw 500.
    """

    status: str = "error"
    message: str
    error_code: Optional[str] = None
