#!/usr/bin/env python3
"""Print sections of the documents, found by their anchors.

    scripts/show.py <n>       a Roadmap chunk, by its number
    scripts/show.py TD8.4     a section of docs/design.md
    scripts/show.py R19.14    a requirement, with its sub-bullets
    scripts/show.py R19       a section of Requirements.md
    scripts/show.py 1046      an entry of docs/issues.md: four digits is an issue, fewer a chunk
    scripts/show.py R2.2 D6   several at once
    scripts/show.py --outline TD8
                              only the headings and requirements inside, with line numbers
    scripts/show.py --toc TD  every heading of one document: R, TD, D, X, G, S, DR, DD, chunks or
                              issues

Decisions (`D12`), deferred entries (`X3`), guidelines (`G3`), Steam findings (`S7`) and the remote
driver's `DR` and `DD` anchors work the same way. A section runs to the next heading of the same or
a higher level; a requirement runs to the next requirement or heading. A landed chunk has no
section, and says so; a lettered part without a section of its own prints its parent's; a retired
issue is reported as absent. Any unknown anchor exits 1, after the others are printed.
"""

import re
import sys

from xref import (
    D_DEF,
    DR_DEF,
    G_DEF,
    H2_NUM,
    H3_NUM,
    R_DEF,
    ROOT,
    X_DEF,
    prose,
)

CHUNK_DEF = re.compile(r"^### (\d+[a-z]?)\.")
S_DEF = re.compile(r"^### (S\d+)\b")
ISSUE_DEF = re.compile(r"^### (\d{4}) ")
ISSUE = re.compile(r"\d{4}")
LETTERED = re.compile(r"(\d{1,3})[a-z]")
LANDED_ROW = re.compile(r"^\| (\d+[a-z]?) +\|")
HEADING = re.compile(r"^(#+) ")
OUTLINE_WIDTH = 100

REQ = ROOT / "Requirements.md"
DRIVER = ROOT / "crates/bevy_remote_driver/docs"

# (anchor form, file, heading patterns, prefix the anchor carries that the heading does not)
KINDS = [
    (re.compile(r"R\d+\.\d+[a-z]?"), REQ, [R_DEF], ""),
    (re.compile(r"DR\d+\.\d+[a-z]?"), DRIVER / "requirements.md", [DR_DEF], ""),
    (re.compile(r"R\d+"), REQ, [H2_NUM], "R"),
    (re.compile(r"DR\d+"), DRIVER / "requirements.md", [H2_NUM], "DR"),
    (re.compile(r"TD\d+(?:\.\d+)?"), ROOT / "docs/design.md", [H2_NUM, H3_NUM], "TD"),
    (re.compile(r"DD\d+(?:\.\d+)?"), DRIVER / "design.md", [H2_NUM, H3_NUM], "DD"),
    (re.compile(r"D\d+"), ROOT / "docs/decisions.md", [D_DEF], ""),
    (re.compile(r"X\d+"), ROOT / "docs/deferred.md", [X_DEF], ""),
    (re.compile(r"G\d+"), ROOT / "docs/guidelines.md", [G_DEF], ""),
    (re.compile(r"S\d+"), ROOT / "docs/steam.md", [S_DEF], ""),
    # Issues are numbered from 1000, so four digits is never a chunk (X60).
    (ISSUE, ROOT / "docs/issues.md", [ISSUE_DEF], ""),
    (re.compile(r"\d+[a-z]?"), ROOT / "Roadmap.md", [CHUNK_DEF], ""),
]

TOC = {
    "R": REQ,
    "DR": DRIVER / "requirements.md",
    "TD": ROOT / "docs/design.md",
    "DD": DRIVER / "design.md",
    "D": ROOT / "docs/decisions.md",
    "X": ROOT / "docs/deferred.md",
    "G": ROOT / "docs/guidelines.md",
    "S": ROOT / "docs/steam.md",
    "chunks": ROOT / "Roadmap.md",
    "issues": ROOT / "docs/issues.md",
}


def level(line):
    m = HEADING.match(line)
    return len(m.group(1)) if m else None


def find(anchor):
    """Return (path, first line, last line), 1-based and inclusive, or None."""
    for form, path, patterns, prefix in KINDS:
        if not form.fullmatch(anchor):
            continue
        key = anchor[len(prefix) :]
        lines = dict(prose(path))  # fenced lines are absent, so a `#` in a code block is not a heading
        start = next(
            (
                n
                for n, line in lines.items()
                for p in patterns
                for m in [p.match(line)]
                if m and m.group(1) == key
            ),
            None,
        )
        if start is None:
            return None
        text = path.read_text(encoding="utf-8").split("\n")
        own = level(lines[start])
        end = len(text)
        for n in range(start + 1, len(text) + 1):
            line = lines.get(n)
            if line is None:
                continue
            if own is None:
                # A requirement: its indented sub-bullets belong to it.
                if level(line) or patterns[0].match(line):
                    end = n - 1
                    break
            elif (l := level(line)) and l <= own:
                end = n - 1
                break
        while end > start and text[end - 1].strip() in ("", "---"):
            end -= 1
        return path, start, end
    return None


def landed(anchor):
    return any(
        m and m.group(1) == anchor for _, l in prose(ROOT / "Roadmap.md") for m in [LANDED_ROW.match(l)]
    )


def outline(path, start, end):
    lines = dict(prose(path))
    for n in range(start, end + 1):
        line = lines.get(n)
        if line is not None and (level(line) or R_DEF.match(line) or DR_DEF.match(line)):
            print(f"{n:>5}  {line.strip()[:OUTLINE_WIDTH]}")


def toc(path):
    print(path.relative_to(ROOT))
    for n, line in prose(path):
        if (level(line) or 0) >= 2:
            print(f"{n:>5}  {line[:OUTLINE_WIDTH]}")


def main():
    args = sys.argv[1:]
    brief = "--outline" in args
    contents = "--toc" in args
    anchors = [a for a in args if a not in ("--outline", "--toc")]
    if not anchors:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    status = 0
    for i, anchor in enumerate(anchors):
        if i:
            print()
        if contents:
            if anchor in TOC:
                toc(TOC[anchor])
            else:
                print(f"{anchor}: not a document; one of {', '.join(TOC)}", file=sys.stderr)
                status = 1
            continue
        found = find(anchor)
        if found is None and landed(anchor):
            print(f"{anchor}: landed; see git log --grep 'chunk {anchor})'", file=sys.stderr)
            status = 1
            continue
        # A sweep unit is lettered as it lands, under its sweep's section.
        part = LETTERED.fullmatch(anchor)
        if found is None and part and (found := find(part.group(1))):
            print(f"{anchor}: part of {part.group(1)}", file=sys.stderr)
        if found is None and ISSUE.fullmatch(anchor):
            print(f"{anchor}: no such issue; a gap in the numbering is a retired one", file=sys.stderr)
            status = 1
            continue
        if found is None:
            print(f"{anchor}: no such section", file=sys.stderr)
            status = 1
            continue
        path, start, end = found
        print(f"{path.relative_to(ROOT)}:{start}-{end}")
        if brief:
            outline(path, start, end)
            continue
        text = path.read_text(encoding="utf-8").split("\n")
        print("\n".join(text[start - 1 : end]))
    return status


if __name__ == "__main__":
    sys.exit(main())
