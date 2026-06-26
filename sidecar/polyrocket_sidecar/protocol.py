"""JSON-RPC 协议 DTO —— 镜像 Rust 中的 `domain::lab::sidecar`。

我们仅使用标准库（dataclasses + json），以保持安装占用为零。
如果未来为了校验而引入 `pydantic`，那么本模块以及对应的 Rust 类型
就需要一起扩展它们的校验规则。
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field, asdict
from typing import Any, Optional

PROTOCOL_VERSION = "0.1.0"


# --- Request ---------------------------------------------------------------

@dataclass
class SidecarRequest:
    id: Any  # 同时接受 int 或 str，以匹配 Rust（Rust 使用形如 "rt-1" 的字符串）
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
        """匹配 Rust 的 wire 格式：`error` 字段是一个普通字符串。

        Rust 的 `SidecarResponse.error` 是 `Option<String>`，不是结构化
        对象。我们把错误码嵌入到消息文本中，以便调用方在需要时仍
        可以解析出来。
        """
        return f"[{self.code}] {self.message}"


@dataclass
class SidecarResponse:
    id: Any  # 同时接受 int 或 str
    result: Optional[Any] = None
    error: Optional[SidecarError] = None

    @property
    def is_error(self) -> bool:
        return self.error is not None

    @property
    def ok(self) -> bool:
        """镜像 Rust 的 `SidecarResponse::ok` 字段。必须出现在
        序列化后的 JSON 中，以便 Rust 的 `parse_line` 能区分
        请求（具有 `method`）和响应（具有 `ok`）。
        """
        return self.error is None


# --- Parse / serialize -----------------------------------------------------

def parse_line(line: str) -> SidecarRequest:
    """将一行 stdin 解析为 SidecarRequest。

    接受 `id` 为 int 或 str（Rust 端使用形如 "rt-1" 的字符串 id）。
    若输入格式错误则抛出 ValueError（调用方捕获并回复 -32700）。
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
    """序列化为单行输出到 stdout（不含末尾换行）。

    Wire 格式（必须与 Rust 的 `SidecarResponse` 匹配）：
        {"id": ..., "ok": true,  "result": ...}   成功时
        {"id": ..., "ok": false, "error": "msg"}  失败时（字符串！）
    """
    out: dict[str, Any] = {"id": resp.id, "ok": resp.ok}
    if resp.error is not None:
        out["error"] = resp.error.to_string()
        out["result"] = None
    else:
        out["result"] = resp.result
        out["error"] = None
    return json.dumps(out, ensure_ascii=False)
