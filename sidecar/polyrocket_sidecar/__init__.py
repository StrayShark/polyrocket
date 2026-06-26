"""polyrocket JSON-RPC 侧车（sidecar）的真实 Python 实现。

关于协议和入口点，请参见 `__main__.py` 中的模块级 docstring。
这个包有意保持小巧——对 Tauri 应用来说是一个黑盒，v0.7b 的价值在于证明
协议在两端都是可实现的，而不是为了交付一个强大的模型。
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
