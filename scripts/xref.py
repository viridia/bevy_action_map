#!/usr/bin/env python3
"""Check that the documents' cross-references resolve.

Three numbering schemes tie the documents and the Rust comments together: `R<section>.<n>` for
requirements, `D<n>` for decisions, and `§<n>` for sections. Nothing else validates them, so they
stay correct only because people are careful. This reports every reference that points nowhere.

    scripts/xref.py             every check; exit 1 if any fails
    scripts/xref.py --report    also list requirements nothing cites
    scripts/xref.py --quiet     failures only, no summary

Each numbered document owns a prefix, so a reference resolves without knowing where it is written:
`R19` is a section of `Requirements.md` and `R19.14` a requirement inside it, `TD5.3` a subsection of
`docs/design.md`, `X12` an entry in `docs/deferred.md`. The remote driver's two documents own `DR`
and `DD` the same way. The section sign these replaced is retired, and finding one is an error.
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
X_DEF = re.compile(r"^### (X\d+)\b")
X_CITE = re.compile(r"\bX\d+\b")
R_SECTION = re.compile(r"\bR(\d+)\b(?!\.\d)")
TD_SECTION = re.compile(r"\bTD(\d+(?:\.\d+)?)\b")
DR_DEF = re.compile(r"^\s*- \*\*(DR\d+\.\d+[a-z]?) \((?:MUST|SHOULD|MAY|WITHDRAWN)\)\*\*")
DR_CITE = re.compile(r"\bDR\d+\.\d+[a-z]?\b")
DR_SECTION = re.compile(r"\bDR(\d+)\b(?!\.\d)")
DD_SECTION = re.compile(r"\bDD(\d+(?:\.\d+)?)\b")
RETIRED = re.compile(r"§")
H2_NUM = re.compile(r"^## (\d+)\.")
H3_NUM = re.compile(r"^### (\d+\.\d+)\b")


def sources():
    md = (
        sorted(ROOT.glob("*.md"))
        + sorted(ROOT.glob("docs/*.md"))
        + sorted(ROOT.glob("bevy_remote_driver/docs/*.md"))
    )
    rs = sorted(
        p
        for d in ("src", "tests", "examples", "bevy_remote_driver/src")
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
    x_defined = headings(ROOT / "docs/deferred.md", X_DEF)

    req_sections = headings(req, H2_NUM)
    des_sections = headings(des, H2_NUM) | headings(des, H3_NUM)

    driver = ROOT / "bevy_remote_driver/docs"
    dreq, ddes = driver / "requirements.md", driver / "design.md"
    dr_defined = {m.group(1) for _, l in prose(dreq) for m in [DR_DEF.match(l)] if m}
    dreq_sections = headings(dreq, H2_NUM)
    ddes_sections = headings(ddes, H2_NUM) | headings(ddes, H3_NUM)

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

            for m in X_CITE.finditer(line):
                if m.group() not in x_defined:
                    fail(path, n, f"{m.group()} has no entry in docs/deferred.md")

            for m in R_STAR.finditer(line):
                if not R_DEF.match(line):
                    fail(path, n, f"{m.group()} is bold but is not a definition")

            # Markdown only: `R90` is a Rotation variant in the examples, and a bare R-number is
            # indistinguishable from any other short identifier. Requirement citations are dotted,
            # so they stay checked everywhere; only the section form is narrowed.
            if path in md:
                for m in R_SECTION.finditer(line):
                    if m.group(1) not in req_sections:
                        fail(path, n, f"R{m.group(1)} is not a section of Requirements.md")

            for m in TD_SECTION.finditer(line):
                if m.group(1) not in des_sections:
                    fail(path, n, f"TD{m.group(1)} is not a section of docs/design.md")

            for m in DR_CITE.finditer(line):
                if m.group() not in dr_defined:
                    fail(path, n, f"{m.group()} has no definition in {dreq.relative_to(ROOT)}")

            if path in md:
                for m in DR_SECTION.finditer(line):
                    if m.group(1) not in dreq_sections:
                        fail(path, n, f"DR{m.group(1)} is not a section of {dreq.relative_to(ROOT)}")

            for m in DD_SECTION.finditer(line):
                if m.group(1) not in ddes_sections:
                    fail(path, n, f"DD{m.group(1)} is not a section of {ddes.relative_to(ROOT)}")

            if RETIRED.search(line):
                fail(path, n, "a retired section sign survived the TD/R migration")

    for path, n, rid in r_dupes:
        fail(path, n, f"{rid} is defined more than once")

    for line in fails:
        print(line)

    if args.report:
        uncited = sorted(set(r_defined) - cited, key=lambda r: [int(x) for x in re.findall(r"\d+", r)])
        print(f"\nRequirements nothing cites ({len(uncited)}) -- a traceability signal, not a failure:")
        print("  " + ", ".join(uncited) if uncited else "  none")

    if not args.quiet:
        print(
            f"\n{len(md)} documents, {len(rs)} Rust files | "
            f"{len(r_defined)} requirements, {len(d_defined)} decisions, {len(x_defined)} deferred, "
            f"{len(req_sections) + len(des_sections)} sections | "
            f"{len(fails)} problem(s)"
        )
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
