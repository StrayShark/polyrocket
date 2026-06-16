"""Entrypoint — `python3 -m polyrocket_sidecar`.

Reads JSON-RPC lines from stdin, dispatches to the right method, writes
JSON-RPC lines to stdout. All errors are caught and turned into a
SidecarError response so the Tauri app never sees a truncated line.

Usage:
    python3 -m polyrocket_sidecar          # interactive
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

# Error codes (mirroring JSON-RPC 2.0 loosely)
ERR_PARSE = -32700
ERR_INVALID_REQUEST = -32600
ERR_METHOD_NOT_FOUND = -32601
ERR_INVALID_PARAMS = -32602
ERR_INTERNAL = -32603


def _handle(line: str) -> str:
    try:
        req = parse_line(line)
    except (ValueError, json.JSONDecodeError) as e:
        # No id available — reply with id=-1 so the caller can still log it
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
    except Exception as e:  # noqa: BLE001 — we want to catch *all* sidecar errors
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
