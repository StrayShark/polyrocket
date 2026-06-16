"""JSON-RPC protocol DTOs — mirrors `domain::lab::sidecar` in Rust.

We use stdlib only (dataclasses + json) to keep the install footprint zero.
If `pydantic` is ever added for validation, both this module and the
matching Rust types would need to grow their validation rules together.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field, asdict
from typing import Any, Optional

PROTOCOL_VERSION = "0.1.0"


# --- Request ---------------------------------------------------------------

@dataclass
class SidecarRequest:
    id: Any  # accept int OR str to match Rust (Rust uses strings like "rt-1")
    method: str
    params: dict[str, Any] = field(default_factory=dict)

    def to_json(self) -> str:
        return json.dumps(
            {"id": self.id, "method": self.method, "params": self.params},
            ensure_ascii=False,
        )


# --- Response --------------------------------------------------------------

@dataclass
class SidecarError:
    code: int
    message: str

    def to_dict(self) -> dict[str, Any]:
        return {"code": self.code, "message": self.message}

    def to_string(self) -> str:
        """Match the Rust wire format: `error` field is a plain string.

        The Rust `SidecarResponse.error` is `Option<String>`, not a structured
        object. We embed the code in the message text so callers can still
        parse it out if they care.
        """
        return f"[{self.code}] {self.message}"


@dataclass
class SidecarResponse:
    id: Any  # accept int OR str
    result: Optional[Any] = None
    error: Optional[SidecarError] = None

    @property
    def is_error(self) -> bool:
        return self.error is not None

    @property
    def ok(self) -> bool:
        """Mirror the Rust `SidecarResponse::ok` field. Must appear in the
        serialized JSON so the Rust `parse_line` can distinguish requests
        (have `method`) from responses (have `ok`).
        """
        return self.error is None


# --- Parse / serialize -----------------------------------------------------

def parse_line(line: str) -> SidecarRequest:
    """Parse one stdin line into a SidecarRequest.

    Accepts `id` as int OR str (Rust side uses string ids like "rt-1").
    Raises ValueError on malformed input (caller catches and replies -32700).
    """
    obj = json.loads(line)
    if not isinstance(obj, dict):
        raise ValueError("request must be a JSON object")
    if "id" not in obj or "method" not in obj:
        raise ValueError("request missing 'id' or 'method'")
    rid = obj["id"]
    if not isinstance(rid, (int, str)):
        raise ValueError(f"id must be int or string, got {rid!r}")
    method = str(obj["method"])
    params = obj.get("params", {})
    if not isinstance(params, dict):
        raise ValueError("params must be an object")
    return SidecarRequest(id=rid, method=method, params=params)


def serialize_response(resp: SidecarResponse) -> str:
    """Serialize to a single stdout line (no trailing newline).

    Wire format (must match Rust `SidecarResponse`):
        {"id": ..., "ok": true,  "result": ...}   on success
        {"id": ..., "ok": false, "error": "msg"}  on failure (string!)
    """
    out: dict[str, Any] = {"id": resp.id, "ok": resp.ok}
    if resp.error is not None:
        out["error"] = resp.error.to_string()
        out["result"] = None
    else:
        out["result"] = resp.result
        out["error"] = None
    return json.dumps(out, ensure_ascii=False)
