#!/usr/bin/env python3
"""Report how much there is to read, against a base commit.

Nothing here passes or fails. The numbers are for a reviewer accepting a change, who knows what the
change was for and so whether its growth was earned.

    scripts/growth.py              the working tree against HEAD
    scripts/growth.py <ref>        the working tree against <ref>
    scripts/growth.py --api        also the public API, and public items nothing outside src/ names

`--api` builds rustdoc JSON through `cargo public-api` (a minute or so) and reports the working tree
only: a base would need a second build of an old checkout.

Type counts come from a line scan, not a parser, so treat them as close rather than exact. A file's
test types are everything after its first `#[cfg(test)]`.
"""

import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MEMORY_INDEX = (
    Path.home() / ".claude/projects" / re.sub(r"[^A-Za-z0-9]", "-", str(ROOT)) / "memory/MEMORY.md"
)

# Where a document's entries start, keyed by path. Requirements are list items, and run until the
# next line that is neither blank nor indented.
ENTRIES = {
    "docs/decisions.md": ("D", re.compile(r"^### (D\d+)\b")),
    "docs/deferred.md": ("X", re.compile(r"^### (X\d+)\b")),
    "docs/issues.md": ("issue", re.compile(r"^### (\d+)\b")),
    "docs/steam.md": ("S", re.compile(r"^### (S\d+)\b")),
    "docs/design.md": ("TD", re.compile(r"^#{2,3} (\d+(?:\.\d+)?)\.? ")),
    "Requirements.md": ("R", re.compile(r"^- \*\*(R\d+\.\d+[a-z]?) ")),
    "bevy_remote_driver/docs/requirements.md": ("DR", re.compile(r"^- \*\*(DR\d+\.\d+[a-z]?) ")),
    "bevy_remote_driver/docs/design.md": ("DD", re.compile(r"^#{2,3} (\d+(?:\.\d+)?)\.? ")),
}
LIST_ENTRY = {"R", "DR"}

TYPE_DEF = re.compile(r"^\s*(pub(?:\([^)]*\))?\s+)?(struct|enum|trait|type|union)\s+[A-Z]\w*")


def git(*args):
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        check=True,
    ).stdout


class Tree:
    """Files as they are at a commit, or in the working tree when `ref` is None."""

    def __init__(self, ref):
        self.ref = ref

    def paths(self, *patterns):
        if self.ref:
            listed = git("ls-tree", "-r", "--name-only", self.ref).split("\n")
        else:
            listed = git("ls-files", "--cached", "--others", "--exclude-standard").split("\n")
            listed = [p for p in listed if (ROOT / p).exists()]
        return sorted(p for p in listed if p and any(re.fullmatch(pat, p) for pat in patterns))

    def read(self, path):
        if self.ref:
            try:
                return git("show", f"{self.ref}:{path}")
            except subprocess.CalledProcessError:
                return ""
        p = ROOT / path
        return p.read_text(encoding="utf-8") if p.exists() else ""


def words(text):
    return len(text.split())


def documents(tree):
    return {p: words(tree.read(p)) for p in tree.paths(r"(?!archive/)(?!target/).*\.md")}


def entries(tree):
    """Per scheme: a list of (id, words) for each entry."""
    out = {}
    for path, (scheme, start) in ENTRIES.items():
        found, current, body = [], None, []

        def close():
            if current:
                found.append((current, words("\n".join(body))))

        for line in tree.read(path).split("\n"):
            m = start.match(line)
            if m:
                close()
                current, body = m.group(1), [line]
            elif current and scheme in LIST_ENTRY and line and not line[0].isspace():
                close()
                current, body = None, []
            elif current and line.startswith("#"):
                close()
                current, body = None, []
            elif current:
                body.append(line)
        close()
        out[scheme] = found
    return out


def section(text, heading):
    """A `## ` section's text, up to the next `## ` heading."""
    lines = text.split("\n")
    try:
        i = lines.index(heading)
    except ValueError:
        return ""
    end = next((j for j in range(i + 1, len(lines)) if lines[j].startswith("## ")), len(lines))
    return "\n".join(lines[i:end])


def bootstrap(tree):
    """What a session reads before it starts: CLAUDE.md, the memory index, and the orientation
    sequence CLAUDE.md gives. The memory index is not versioned, so it is today's on both sides."""
    parts = {
        "CLAUDE.md": words(tree.read("CLAUDE.md")),
        "Roadmap.md, Where this stands": words(
            section(tree.read("Roadmap.md"), "## Where this stands")
        ),
        "docs/issues.md": words(tree.read("docs/issues.md")),
    }
    if MEMORY_INDEX.exists():
        parts["memory index (unversioned)"] = words(MEMORY_INDEX.read_text(encoding="utf-8"))
    return parts


def code(tree):
    counts = defaultdict(int)
    for path in tree.paths(r"src/.*\.rs"):
        test = False
        for line in tree.read(path).split("\n"):
            if line.startswith("#[cfg(test)]"):
                test = True
            counts["test lines" if test else "library lines"] += 1
            m = TYPE_DEF.match(line)
            if not m:
                continue
            if test:
                counts["test types"] += 1
                continue
            counts["library types"] += 1
            if (m.group(1) or "").strip() == "pub":
                counts["  pub"] += 1
            else:
                counts["  private"] += 1
            if m.group(2) == "type":
                counts["  of which aliases"] += 1
    return counts


API_ITEM = re.compile(
    r"^pub (?:const |unsafe |async )*(struct|enum|trait|fn|const|static|type|macro|union) "
    r"bevy_action_map::(.*)"
)
API_IMPL = re.compile(r"^impl(?:<.*?>)? (.*?)(?: for (.*?))?(?: where .*)?$")


def item_path(rest):
    """The item's path from the rest of a listing line, generic arguments dropped."""
    out, depth = [], 0
    for c in rest:
        if c == "<":
            depth += 1
        elif c == ">":
            depth -= 1
        elif depth == 0:
            if c.isspace() or c in "(=":
                break
            out.append(c)
    return "".join(out).rstrip(":")


def public_api():
    """Public items the crate defines, and methods from inherent impls. Methods a trait impl brings
    in, derived `Reflect` above all, are the trait's surface rather than the crate's."""
    listing = subprocess.run(
        ["cargo", "public-api", "--all-features", "-sss", "--color=never"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    if listing.returncode != 0:
        sys.exit("`cargo public-api` failed; is it installed, with a nightly toolchain?")
    items, methods, ours = {}, set(), True
    for line in listing.stdout.split("\n"):
        m = API_IMPL.match(line)
        if m:
            ours = m.group(2) is None
            continue
        m = API_ITEM.match(line)
        if not m:
            continue
        kind, path = m.group(1), item_path(m.group(2))
        if kind == "trait":
            ours = True
        # A capitalized parent makes it associated: a method, constant or type of a type or trait.
        if re.search(r"(^|::)[A-Z]\w*::\w+$", path):
            if kind == "fn" and ours:
                methods.add(path)
            continue
        items[path.rsplit("::", 1)[-1]] = kind
    return items, methods


def prelude():
    """Names `lib.rs`'s prelude re-exports: public on purpose, where the rest of the public API may
    be public only because its module is."""
    text = (ROOT / "src/lib.rs").read_text(encoding="utf-8")
    start = text.index("pub mod prelude {")
    end = text.index("\n}", start)
    return set(re.findall(r"\b\w+\b", text[start:end]))


def unreached(names):
    """Public names that no example, integration test or Steam example mentions."""
    text = "\n".join(
        p.read_text(encoding="utf-8")
        for d in ("examples", "tests", "steam_examples/disasteroids")
        for p in (ROOT / d).rglob("*.rs")
    )
    return sorted(n for n in names if not re.search(rf"\b{re.escape(n)}\b", text))


def row(label, before, after, width):
    delta = after - before
    sign = f"{delta:+d}" if delta else "."
    print(f"  {label:<{width}} {before:>8} {after:>8} {sign:>8}")


def table(title, before, after):
    keys = list(dict.fromkeys([*before, *after]))
    width = max(map(len, keys), default=0)
    print(f"\n{title}\n  {'':<{width}} {'base':>8} {'now':>8} {'delta':>8}")
    for k in keys:
        row(k, before.get(k, 0), after.get(k, 0), width)
    row("total", sum(before.values()), sum(after.values()), width)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("base", nargs="?", default="HEAD")
    ap.add_argument("--api", action="store_true", help="also the public API (slow)")
    args = ap.parse_args()

    base, now = Tree(args.base), Tree(None)
    short = git("rev-parse", "--short", args.base).strip()
    print(f"Base: {args.base} ({short}). Now: the working tree.")

    table("Markdown words, by document", documents(base), documents(now))

    print("\nEntries: count, mean words, largest (base -> now)")
    b, n = entries(base), entries(now)
    for scheme in ENTRIES.values():
        s = scheme[0]

        def summary(found):
            if not found:
                return "-"
            big = max(found, key=lambda e: e[1])
            return f"{len(found)}, {sum(w for _, w in found) // len(found)}, {big[0]} {big[1]}"

        print(f"  {s:<6} {summary(b[s]):>22} -> {summary(n[s])}")

    table("Bootstrap reading, in words", bootstrap(base), bootstrap(now))

    before, after = code(base), code(now)
    order = ["library lines", "test lines", "library types", "  pub", "  private",
             "  of which aliases", "test types"]
    width = max(map(len, order))
    print(f"\nsrc/\n  {'':<{width}} {'base':>8} {'now':>8} {'delta':>8}")
    for k in order:
        row(k, before.get(k, 0), after.get(k, 0), width)

    if args.api:
        items, methods = public_api()
        kinds = defaultdict(int)
        for kind in items.values():
            kinds[kind] += 1
        print("\nPublic API, now: " + ", ".join(f"{v} {k}" for k, v in sorted(kinds.items()))
              + f", {len(methods)} inherent methods")
        missing, exported = unreached(items), prelude()
        print(
            f"\nPublic items no example or integration test names ({len(missing)}; "
            f"{sum(n not in exported for n in missing)} outside the prelude, marked *):"
        )
        for name in sorted(missing, key=lambda n: (n in exported, n)):
            print(f"  {' ' if name in exported else '*'} {items[name]:<6} {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
