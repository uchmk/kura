#!/usr/bin/env python3
"""Move finished top-level `- [x]` items from TODO.md to TODO-DONE.md.

Why: sessions re-read TODO.md every time, so finished items cost tokens.
Run this before committing a change that ticks an item. Stdlib only, idempotent.

Block rule: a top-level line starting with `- [x]` plus the lines after it that
are indented (blank lines count only when an indented line follows them). The
block ends at the next non-indented, non-blank line (`- `, `#`, or prose).
`- [ ]` blocks stay in TODO.md together with their children, even if a child is `[x]`.

TODO-DONE.md gets one `## 項目ごとの移し替え（…）` heading (made once), and under it
a `### <section heading text>` per source section with the moved blocks in order.

Usage: python scripts/archive-done.py [--dry-run]
"""
import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TODO = ROOT / "TODO.md"
DONE = ROOT / "TODO-DONE.md"
MARKER = "## 項目ごとの移し替え（"
HEADING_RE = re.compile(r"^(#{1,6})\s+(.*\S)\s*$")


def read(path):
    with open(path, encoding="utf-8", newline="") as f:
        return f.read()


def write(path, text):
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)


def split_todo(lines):
    """Return (kept_lines, moved) where moved is a list of (section, [lines])."""
    kept, moved = [], []
    heading = "(見出しなし)"
    fence = False
    i, n = 0, len(lines)
    while i < n:
        line = lines[i]
        if line.startswith("```"):
            fence = not fence
        m = None if fence else HEADING_RE.match(line)
        if m:
            heading = m.group(2)
        if not fence and line.startswith("- [x]"):
            end = i + 1
            j = i + 1
            while j < n:
                if lines[j].strip() == "":
                    j += 1
                elif lines[j][0] in " \t":
                    j += 1
                    end = j
                else:
                    break
            moved.append((heading, lines[i:end]))
            i = end
            continue
        kept.append(line)
        i += 1
    return kept, moved


def collapse_blanks(lines):
    out = []
    for line in lines:
        if line.strip() == "" and out and out[-1].strip() == "":
            continue
        out.append(line)
    return out


def merge_into_done(done_text, moved):
    by_section = {}
    for heading, block in moved:
        by_section.setdefault(heading, []).extend(block)
    lines = done_text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()  # drop the final newline's empty piece; re-added on join
    if not any(l.startswith(MARKER) for l in lines):
        today = datetime.date.today().isoformat()
        lines += ["", f"## 項目ごとの移し替え（{today} 以降は、済んだ項目をすぐここへ移す）"]
    start = next(k for k, l in enumerate(lines) if l.startswith(MARKER))
    for heading, block in by_section.items():
        want = f"### {heading}"
        pos = next((k for k in range(start + 1, len(lines)) if lines[k] == want), None)
        if pos is None:
            lines += ["", want] + block
            continue
        end = pos + 1
        while end < len(lines) and not lines[end].startswith(("## ", "### ")):
            end += 1
        while end > pos + 1 and lines[end - 1].strip() == "":
            end -= 1
        lines[end:end] = block
    return "\n".join(lines) + "\n"


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    dry = "--dry-run" in sys.argv[1:]
    todo_text = read(TODO)
    kept, moved = split_todo(todo_text.split("\n"))
    counts = {}
    for heading, block in moved:
        counts[heading] = counts.get(heading, 0) + 1
    print(f"to move: {len(moved)} item(s) from {len(counts)} section(s)")
    for heading, c in counts.items():
        print(f"  {c:4d}  {heading}")
    if dry or not moved:
        return 0
    write(TODO, "\n".join(collapse_blanks(kept)))
    write(DONE, merge_into_done(read(DONE), moved))
    print("done: TODO.md -> TODO-DONE.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
