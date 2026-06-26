#!/usr/bin/env python3
"""Generate docs/src/llms.txt (the llms.txt convention, https://llmstxt.org) from
SUMMARY.md, so LLMs/agents get a curated index of the docs. Run after editing
SUMMARY.md:  python3 docs/gen-llms.py

The file is served at the site root (/ringo/llms.txt). Links are flattened per
tool section and made absolute (the convention wants full URLs)."""
import re
import pathlib

BASE = "https://davidborzek.github.io/ringo/"
HERE = pathlib.Path(__file__).resolve().parent
SUMMARY = HERE / "src" / "SUMMARY.md"
OUT = HERE / "src" / "llms.txt"

SUMMARY_LINE = (
    "Tooling for baresip SIP softphones: **ringo-phone** (a terminal SIP softphone) "
    "and **ringo-flow** (a declarative telephony scenario test runner). "
    f"Full docs: {BASE}"
)

link_re = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")


def main() -> None:
    section = None
    out = [f"# ringo\n", f"> {SUMMARY_LINE}\n"]
    body: list[str] = []
    for raw in SUMMARY.read_text().splitlines():
        line = raw.strip()
        if line.startswith("# ") and "Summary" not in line:
            section = line[2:].strip()
            body.append(f"\n## {section}\n")
            continue
        m = link_re.search(line)
        if not m or not line.lstrip().startswith(("-", "[")):
            continue
        title, path = m.group(1), m.group(2)
        if not path or path.startswith(("http", "#")):
            continue
        if path == "index.md":  # the landing prefix chapter
            continue
        url = BASE + re.sub(r"\.md($|#)", r".html\1", path)
        body.append(f"- [{title}]({url})")

    out.extend(body)
    out.append("\n## Machine-readable\n")
    out.append(
        f"- [ringo-flow Rhai type definitions]({BASE}ringo-flow/ringo-flow.d.rhai): "
        "the full scenario API as a `.d.rhai` (signatures for the Rhai LSP and agents)"
    )
    OUT.write_text("\n".join(out) + "\n")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
