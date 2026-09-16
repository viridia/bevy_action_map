#!/usr/bin/env python3
"""Check that the documents' cross-references resolve.

Three numbering schemes tie the documents and the Rust comments together: `R<section>.<n>` for
requirements, `D<n>` for decisions, and `§<n>` for sections. Nothing else validates them, so they
stay correct only because people are careful. This reports every reference that points nowhere.

    scripts/xref.py             every check; exit 1 if any fails
    scripts/xref.py --report    also list requirements nothing cites
    scripts/xref.py --quiet     failures only, no summary

A bare `§` is genuinely ambiguous: both `Requirements.md` and `docs/design.md` carry numbered
sections and both are cited bare, so a reference is accepted when it resolves in either. Where a
citation names its document the name is used instead, which is the stricter check.
"""

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

R_DEF = re.compile(r"^\s*- \*\*(R\d+\.\d+[a-z]?) \((?:MUST|SHOULD|MAY|WITHDRAWN)\)\*\*")
R_CITE = re.compile(r"\bR\d+\.\d+[a-z]?\b")
R_STAR = re.compile(r"\*\*R\d+\.\d+[a-z]?")
D_DEF = re.compile(r"^### (D\d+)\b")
D_CITE = re.compile(r"\bD\d+\b")
SECTION = re.compile(r"§(\d+(?:\.\d+)?)")
H2_NUM = re.compile(r"^## (\d+)\.")
H3_NUM = re.compile(r"^### (\d+\.\d+)\b")

# A citation may name its document first. The bare word "design" is the most common form by far --
# most of decisions.md's table cells read `| design §10.3 |`.
QUALIFIER = re.compile(
    r"`?Requirements\.md`?"
    r"|[\w./`\[\]()-]*design\.md[`)\]]*"
    r"|\b[Dd]esign\b"
)
# What may sit between a qualifier and a section number it still governs: the separators of a list,
# so that `design.md §7.3, §8.2 and §10` qualifies all three.
INHERITS = re.compile(r"[\s,]*(?:and[\s,]*)?(?:§[\d.]+[\s,]*(?:and[\s,]*)?)*")


def sources():
    md = sorted(ROOT.glob("*.md")) + sorted(ROOT.glob("docs/*.md"))
    rs = sorted(
        p
        for d in ("src", "tests", "examples")
        for p in (ROOT / d).rglob("*.rs")
    )
    return md, rs


def prose(path):
    """Yield (line number, text), skipping fenced code blocks."""
    fenced = False
    for n, line in enumerate(path.read_text(encoding="utf-8").split("\n"), 1):
        if line.lstrip().startswith("```"):
            fenced = not fenced
            continue
        if not fenced:
            yield n, line


def headings(path, pattern):
    return {m.group(1) for _, l in prose(path) for m in [pattern.match(l)] if m}


def qualifying_doc(line, pos):
    """The document named before the section reference at `pos`, or None if it stands bare."""
    named = None
    for m in QUALIFIER.finditer(line):
        if m.end() > pos:
            break
        if INHERITS.fullmatch(line[m.end() : pos]):
            named = m.group()
    if named is None:
        return None
    return "requirements" if "Requirements" in named else "design"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--report", action="store_true", help="list requirements nothing cites")
    ap.add_argument("--quiet", action="store_true", help="failures only")
    args = ap.parse_args()

    md, rs = sources()
    req, dec, des = ROOT / "Requirements.md", ROOT / "docs/decisions.md", ROOT / "docs/design.md"

    # Definitions, and where each one lives, so a definition is not counted as a citation of itself.
    r_defined, r_dupes = {}, []
    for n, line in prose(req):
        m = R_DEF.match(line)
        if m:
            if m.group(1) in r_defined:
                r_dupes.append((req, n, m.group(1)))
            r_defined.setdefault(m.group(1), n)
    d_defined = headings(dec, D_DEF)

    req_sections = headings(req, H2_NUM)
    des_sections = headings(des, H2_NUM) | headings(des, H3_NUM)

    fails, cited = [], set()

    def fail(path, n, msg):
        fails.append(f"{path.relative_to(ROOT)}:{n}  {msg}")

    for path in md + rs:
        is_req = path == req
        for n, line in prose(path):
            defines_here = is_req and R_DEF.match(line)

            for m in R_CITE.finditer(line):
                rid = m.group()
                if defines_here and r_defined.get(rid) == n:
                    continue  # the definition itself
                cited.add(rid)
                if rid not in r_defined:
                    fail(path, n, f"{rid} has no definition in Requirements.md")

            for m in D_CITE.finditer(line):
                if m.group() not in d_defined:
                    fail(path, n, f"{m.group()} has no entry in docs/decisions.md")

            for m in R_STAR.finditer(line):
                if not R_DEF.match(line):
                    fail(path, n, f"{m.group()} is bold but is not a definition")

            for m in SECTION.finditer(line):
                sec, doc = m.group(1), qualifying_doc(line, m.start())
                if doc == "requirements" and sec not in req_sections:
                    fail(path, n, f"§{sec} is not a section of Requirements.md")
                elif doc == "design" and sec not in des_sections:
                    fail(path, n, f"§{sec} is not a section of docs/design.md")
                elif doc is None and sec not in req_sections and sec not in des_sections:
                    fail(path, n, f"§{sec} resolves in neither Requirements.md nor docs/design.md")

    for path, n, rid in r_dupes:
        fail(path, n, f"{rid} is defined more than once")

    # CLAUDE.md states three counts in its document table. Each has drifted before.
    claude = ROOT / "CLAUDE.md"
    text = claude.read_text(encoding="utf-8")
    highest_d = max(int(d[1:]) for d in d_defined)
    for pattern, actual, what in (
        (r"`D1`–`D(\d+)`", str(highest_d), "highest decision number"),
        (r"(\d+) numbered requirements", str(len(r_defined)), "requirement count"),
        (r"in sections \d+–(\d+)", max(req_sections, key=int), "highest section number"),
    ):
        m = re.search(pattern, text)
        if not m:
            fail(claude, 0, f"the assertion of the {what} is missing")
        elif m.group(1) != actual:
            n = text[: m.start()].count("\n") + 1
            fail(claude, n, f"claims {what} {m.group(1)}, but it is {actual}")

    for line in fails:
        print(line)

    if args.report:
        uncited = sorted(set(r_defined) - cited, key=lambda r: [int(x) for x in re.findall(r"\d+", r)])
        print(f"\nRequirements nothing cites ({len(uncited)}) -- a traceability signal, not a failure:")
        print("  " + ", ".join(uncited) if uncited else "  none")

    if not args.quiet:
        print(
            f"\n{len(md)} documents, {len(rs)} Rust files | "
            f"{len(r_defined)} requirements, {len(d_defined)} decisions, "
            f"{len(req_sections) + len(des_sections)} sections | "
            f"{len(fails)} problem(s)"
        )
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
