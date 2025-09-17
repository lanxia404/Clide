"""Placeholder agent host illustrating how Python modules can collaborate with the Rust core.

This module sketches the async entry point that would expose an IPC or WebSocket
endpoint for AI assistants. The Rust application can spawn this module as a
subprocess and exchange JSON messages that describe file updates, suggestions,
and diff previews.
"""

from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import AsyncIterator, Dict, List

CONFIG_PATH = Path(__file__).resolve().parent.parent / "config" / "layout.json"


@dataclass
class AgentSuggestion:
    title: str
    detail: str
    file: str
    line: int
    patch: str


async def load_initial_config() -> Dict[str, object]:
    async def read() -> str:
        loop = asyncio.get_running_loop()
        return await loop.run_in_executor(None, CONFIG_PATH.read_text)

    raw = await read()
    return json.loads(raw)


async def stream_suggestions() -> AsyncIterator[AgentSuggestion]:
    """Yield fake suggestions so the Rust UI has something to render."""

    examples: List[AgentSuggestion] = [
        AgentSuggestion(
            title="Refactor proposal",
            detail="Extract helper function in src/editor.rs",
            file="src/editor.rs",
            line=12,
            patch="- let idx = self.char_index();\n+ let index = self.char_index();\n",
        ),
        AgentSuggestion(
            title="Formatting",
            detail="Trim trailing spaces in src/app.rs",
            file="src/app.rs",
            line=88,
            patch="(no-op demo)",
        ),
    ]

    for suggestion in examples:
        await asyncio.sleep(1.0)
        yield suggestion


async def main() -> None:
    config = await load_initial_config()
    print("agent bootstrap", config.get("theme"))

    async for suggestion in stream_suggestions():
        payload = json.dumps(asdict(suggestion))
        print("suggestion", payload)


if __name__ == "__main__":
    asyncio.run(main())
