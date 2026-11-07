#!/usr/bin/env python3
"""Apply the Vello CPU flush workaround to the resolved epaint 0.37 source.

The workspace enables Vello CPU multithreading through feature unification. epaint
0.37's outline-glyph paths must flush queued Vello work before rendering; without
that, the multithreaded dispatcher panics on first text layout.
"""
from __future__ import annotations

import os
import re
from pathlib import Path

cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
candidates = sorted(
    (cargo_home / "registry" / "src").glob(
        "*/epaint-0.37.*/src/text/font_face.rs"
    )
)

if not candidates:
    raise SystemExit(
        "Could not find fetched epaint 0.37 sources. Run cargo fetch first."
    )

patched = 0
for source in candidates:
    text = source.read_text(encoding="utf-8")
    pattern = re.compile(r"(?m)^([ \t]*)ctx\.render\(")

    def add_flush(match: re.Match[str]) -> str:
        indent = match.group(1)
        before = text[: match.start()]
        if re.search(
            r"(?m)^" + re.escape(indent) + r"ctx\.flush\(\);\s*$", before
        ):
            return match.group(0)
        return f"{indent}ctx.flush();\n{match.group(0)}"

    updated, count = pattern.subn(add_flush, text)
    if count == 0:
        raise SystemExit(f"No Vello render calls found in {source}")
    if updated != text:
        source.write_text(updated, encoding="utf-8")
        patched += count
        print(f"Patched {count} Vello render call(s): {source}")
    else:
        print(f"Already patched: {source}")

if patched == 0:
    print("Vello flush calls were already present.")
