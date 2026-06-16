"""Real Python implementation of the polyrocket JSON-RPC sidecar.

See module-level docstring in __main__.py for the wire protocol and entry
point. This package is kept small on purpose — it's a black box to the
Tauri app, and the value of v0.7b is to prove the protocol is implementable
on both sides, not to ship a strong model.
"""

__version__ = "0.1.0"

from .predict import predict_logic, predict_from_markets
from .protocol import (
    SidecarRequest,
    SidecarResponse,
    SidecarError,
    parse_line,
    serialize_response,
    PROTOCOL_VERSION,
)

__all__ = [
    "SidecarRequest",
    "SidecarResponse",
    "SidecarError",
    "parse_line",
    "serialize_response",
    "PROTOCOL_VERSION",
    "predict_logic",
    "predict_from_markets",
    "__version__",
]
