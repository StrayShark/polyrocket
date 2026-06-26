"""入口点 —— `python3 -m polyrocket_sidecar`。

从 stdin 读取 JSON-RPC 行，分发到正确的方法，将 JSON-RPC 行写入 stdout。
所有错误都会被捕获并转换为 SidecarError 响应，这样 Tauri 应用就
永远不会看到截断的行。

用法：
    python3 -m polyrocket_sidecar          # 交互模式
    echo '{"id":1,"method":"Ping","params":{}}' | python3 -m polyrocket_sidecar
"""

from __future__ import annotations

import json
import sys
from typing import Any

from .dispatch import DISPATCH
from .protocol import (
    SidecarError,
    SidecarResponse,
    parse_line,
    serialize_response,
)

# 错误码（大致对应 JSON-RPC 2.0）
ERR_PARSE = -32700
ERR_INVALID_REQUEST = -32600
ERR_METHOD_NOT_FOUND = -32601
ERR_INVALID_PARAMS = -32602
ERR_INTERNAL = -32603


def _handle(line: str) -> str:
    try:
        req = parse_line(line)
    except (ValueError, json.JSONDecodeError) as e:
        # 没有可用的 id —— 用 id=-1 回复，以便调用方仍可记录
        return serialize_response(
            SidecarResponse(
                id=-1,
                error=SidecarError(code=ERR_PARSE, message=f"parse error: {e}"),
            )
        )

    handler = DISPATCH.get(req.method)
    if handler is None:
        return serialize_response(
            SidecarResponse(
                id=req.id,
                error=SidecarError(
                    code=ERR_METHOD_NOT_FOUND,
                    message=f"unknown method: {req.method}",
                ),
            )
        )

    try:
        result: Any = handler(req.params)
    except ValueError as e:
        return serialize_response(
            SidecarResponse(
                id=req.id,
                error=SidecarError(code=ERR_INVALID_PARAMS, message=str(e)),
            )
        )
    except Exception as e:  # noqa: BLE001 —— 我们希望捕获 *所有* 侧车错误
        return serialize_response(
            SidecarResponse(
                id=req.id,
                error=SidecarError(code=ERR_INTERNAL, message=f"internal: {e}"),
            )
        )

    return serialize_response(SidecarResponse(id=req.id, result=result))


def main() -> int:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        sys.stdout.write(_handle(line) + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
