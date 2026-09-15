---
name: comment-grooming
description: Sweep comments and doc comments for house-style violations and duplicated explanations, using read-only recon subagents to find candidates before editing anything by hand. Use when asked to groom, sweep, or clean up comments, doc comments, or prose in the source; or when house-style violations (doc-comment citations, chunk references, the "nothing" tic, narrated history) are suspected to have leaked in.
---

# Comment grooming

A sweep over the crate's comments. Recon is delegated and parallel; **every edit is made by hand,
serially, in one voice**. The whole value of the sweep is that the crate reads as though one
maintainer wrote it, and parallel editing agents destroy exactly that.

## What this is not

Not a bug hunt — use `/code-review`. Not a refactor. No line of code changes; if a comment is wrong
about what the code does, that is a finding for `docs/issues.md`, not a licence to fix the code in
the same pass.

## The dial

**Keep every conclusion. Cut history, restatement, scaffolding and tells.**

The worked example, from the sweep that produced this skill. A 36-line theory-of-operation block in
`src/frame.rs` became 27 lines.

Out: a parenthetical saying what the code "did originally"; a rationale already stated verbatim on
the function it described; the word "load-bearing"; "take nothing" → "take none"; and a "Theory of
operation. … Three rules keep those from interfering." preamble.

In: every "this would fail silently" warning, every constraint on where code may move, a paragraph
defending a non-obvious design against a plausible simplification, and a link to an upstream issue.

A block that is already conclusion-first shrinks by nothing, and that is a correct outcome. Most of
this crate's prose is in that state; the real yield is category 1 below, not length.

## Categories, in value order

1. **Restatement across locations.** A comment saying what is already said by its own adjacent doc
   comment, by another comment elsewhere, or by `docs/design.md`. This is the highest-value
   category and the one that justifies the recon cost, because it cannot be found by reading one
   file. It is also the project's named regression: what has been built is described in exactly one
   place, and a second description anywhere is the thing to watch for. The fix is to pick the
   canonical home and let the others cross-reference it — `Plan::is_indexed` in `src/plan.rs` is the
   model ("A class binding yields to this unconditionally; see the note on `indexed_controls`").
2. **Story rather than conclusion.** Prose narrating how the design was reached — what it used to
   do, what bug was hit, what was tried. A bug story that stops a maintainer re-introducing the bug
   earns its place **compressed to the warning**; cut the narrative tense, not the trap.
3. **Doc-comment citations.** An `R`-number, `§`, `D`-number or `OQ` inside a `///` or `//!`. On
   docs.rs the cited documents do not exist. The reasoning usually survives the edit: drop the
   parenthesis, keep the sentence. Internal `//` comments keep their citations.
4. **"chunk" anywhere in a comment.** Development-methodology vocabulary with no referent in the
   crate's design. Worse in a `///`, but wrong in a `//` too.
5. **House-style tells.** Judge these by *reader*, not by comment marker — see "Which reader" below.
   - *Retired metaphors*: "load-bearing", "seam", "fold", "reach for" used figuratively, which ask
     the reader to learn what a word means here before the sentence parses. Domain vocabulary is not
     a metaphor: `Fold` is a real type in `eval.rs`, and `split_friction`'s graphics seams are
     literal ones between tiles.
   - *Manufactured significance*: "Crucially,", "Importantly,", "It's worth noting that", "Remember
     that". If the sentence matters it matters without the announcement.
   - *Defending anything*: a doc comment arguing for the design rather than describing it. The
     argument belongs in `docs/decisions.md`, or in the chunk's review conversation.
   - *Hedges that gate no real exception*: "simply", "just".
   - *A constant diet of em-dash asides*, which reads as a fingerprint once noticed.
   - *Unusual punctuation or phrasing.*
   - *"nothing" where a positive term exists*: "later ticks take nothing" → "take none". Most uses
     are ordinary English ("nothing fires it") and stay.
6. **Comments explaining the obvious** — restating the adjacent line, enumerating what a reader can
   see, or explaining ECS semantics, borrow rules or standard Bevy behaviour to a maintainer who
   knows them.

## Which reader

The rules divide by who reads a comment, which is not the same as which marker it carries.

A `///` on a `pub` item is public documentation: pedagogy rules, no citations, and every tell in
category 5 applies. It also must not explain *how the thing is implemented* — a public doc describes
what the item does and why it matters to a caller, and an implementation detail there both misleads
and freezes an internal choice into the API's documentation. A `///` on a private or `pub(crate)`
item has a maintainer as its reader, so it keeps its citations, may use the crate's own vocabulary
freely, and may describe the implementation at whatever length the algorithm needs. All nine "fold"
doc comments in the first sweep were on non-public items and were correctly left alone. **Check
visibility before reporting a doc-comment violation.**

And the counterweight, which matters as much as the dial: **concise is not thin.** Cut the padding
around a parameter, a panic or an edge case — not the parameter, panic or edge case itself. A doc
comment that loses a documented panic to brevity has been made worse.

## Workflow

### 1. Survey

Measure before planning, so the split is sized from the real numbers:

```sh
echo -n "doc-comment citations: "; grep -rnE '^\s*(///|//!)' --include='*.rs' src/ tests/ examples/ \
  | grep -cE 'R[0-9]+\.[0-9]+|§|\bD[0-9]+\b|[Cc]hunk [0-9]+|\bOQ[0-9]*\b'
echo -n "chunk refs:            "; grep -rncE '\bchunks?\b' --include='*.rs' src/ tests/ examples/ \
  | awk -F: '{s+=$2} END {print s}'
echo -n "metaphors:             "; grep -rniE '(load-bearing|\bseams?\b|reach for)' --include='*.rs' \
  src/ tests/ examples/ | grep -cE ':[0-9]+:\s*(///|//!|//)'
echo -n "hedges:                "; grep -rncE '^\s*(///|//!|//).*\b(simply|just)\b' --include='*.rs' \
  src/ tests/ examples/ | awk -F: '{s+=$2} END {print s}'
echo -n "manufactured signif.:  "; grep -rncE \
  '^\s*(///|//!|//).*(Crucially|Importantly|Notably|worth noting|Remember that|Keep in mind)' \
  --include='*.rs' src/ tests/ examples/ | awk -F: '{s+=$2} END {print s}'
echo -n "'nothing' in comments: "; grep -rncE '^\s*(///|//!|//).*\bnothing\b' --include='*.rs' \
  src/ tests/ examples/ | awk -F: '{s+=$2} END {print s}'
echo "internal comment lines, per file, heaviest first:"
grep -rcE '^\s*//[^/!]' --include='*.rs' src/ | sort -t: -k2 -rn | head -15
```

Note `grep -c` counts *lines*, not occurrences, which is the right unit here.

### 2. Group the files

Group by area, not by rule. Reading a file once per rule is the waste `CLAUDE.md` warns about;
every rule gets applied in one read. Aim for roughly 250 internal comment lines per group, and keep
files that talk about each other together — `eval.rs` with `plan.rs`, the `context/` files as one.

### 3. Recon, all groups, before any editing

One read-only `Explore` agent per group, in parallel. **Run every group's recon before editing
anything.** Duplication clusters span groups: the first sweep found one fact written out four times
across `eval.rs`, `plan.rs` and `design.md`. Merging a cluster before you can see all of it means
picking a canonical home that the next group then touches again.

Use `Explore`, never `general` — the agents must not be able to edit. Write each list to the
scratchpad.

The prompt template is in [recon-prompt.md](recon-prompt.md). It is long on purpose: the agent
starts cold, with no access to the conversation, `CLAUDE.md`, or the dial.

### 4. Spot-check before trusting a list

On receiving each list, verify three or four of its highest-ranked claims directly. This is a
go/no-go on the list as a whole, not per-entry validation — that happens naturally at edit time,
since the file is open anyway.

A list that survives the spot-check is still only a candidate list. An agent that read the file
once can be confidently wrong about what a comment is for, and the reported line numbers drift if
anything above them was edited first.

### 5. Edit by hand, one group per commit

Serially. One voice. Never delegate the editing, and never run editing agents in parallel — it
makes the diff pane unreviewable and produces as many styles as there are agents.

After each group, before moving on:

```sh
cargo run --manifest-path tools/devfmt/Cargo.toml --quiet -- --diff
```

`--diff` reflows only paragraphs overlapping a changed line, so pre-existing debt elsewhere is left
alone. Run the style pass *before* devfmt, never after, or the same paragraph gets reformatted
twice.

Two traps that cost time in the first sweep.

`awk 'length>100'` counts **bytes**, so an em-dash reads as three columns and reports violations
that are not there. Shell `${#var}` does the same under this shell. Measure characters, or just
trust devfmt.

BSD `sed` has no `\b`, so a comment-wide rename needs `perl`, not `sed`. `sed` fails silently here.

### 6. Finish

`scripts/verify.sh` at the end of the sweep. A comment-only sweep cannot break a test, which is
exactly why an unexpected failure is worth reading rather than assuming.

## Where findings go

Candidate lists live in the scratchpad and are consumed by the edit passes.

Anything still unfixed when the sweep ends **graduates to a `docs/issues.md` entry**, tier 4
(*Prose — a comment or document contradicts the code. No behaviour at stake*), numbered from that
document's counter. Nothing outstanding is left without a destination; a finding with no entry is a
finding that will be dropped.

Two things always graduate rather than being fixed in the sweep: a comment that is *wrong* about the
code rather than merely verbose, and a duplication cluster whose canonical home is a genuine design
question rather than a clerical choice.

## Calibration

From `docs/issues.md`, and it applies to the recon agents exactly as it applied to the scan that
produced it:

> Ask a model to find sixty problems and it will find sixty. Some of what follows is real and some
> is a rule nobody would ever violate.

The recon prompt caps its list and asks for omissions rather than padding, which is the guard. If a
returned list is at its cap, treat that as a signal it was padded.
