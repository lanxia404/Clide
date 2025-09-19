"""Minimal synchronous agent stub for Clide.

The executable reads newline-delimited JSON encoded :class:`AgentRequest`
objects from ``stdin`` and replies with a single :class:`AgentResponse`
object per line.  It is intentionally simple so developers can adapt the
structure to their own toolchains or language runtimes.
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass, asdict
from typing import Any, Dict


@dataclass
class AgentResponsePayload:
    title: str
    detail: str
    file: str
    line: int
    patch: str
    raw: Dict[str, Any]


def build_response(request: Dict[str, Any]) -> AgentResponsePayload:
    content = request.get("content", "")
    line_count = len(content.splitlines())
    cursor_line = int(request.get("cursor_line", 0))
    cursor_col = int(request.get("cursor_col", 0))
    file_path = request.get("file_path") or "untitled"

    detail = (
        f"檔案共有 {line_count} 行，游標位於第 {cursor_line + 1} 行 "
        f"第 {cursor_col + 1} 欄。"
    )
    patch = (
        f"- old line at {cursor_line + 1}\n"
        f"+ new line at {cursor_line + 1} (stub output)"
    )

    return AgentResponsePayload(
        title="分析完成",
        detail=detail,
        file=file_path,
        line=cursor_line,
        patch=patch,
        raw={
            "source": "agent_stub",
            "metadata": request.get("metadata"),
        },
    )


def main() -> None:
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            continue

        response = build_response(request)
        sys.stdout.write(json.dumps(asdict(response), ensure_ascii=False) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
