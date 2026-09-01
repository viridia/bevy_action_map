# Working on `bevy_action_map`

A Bevy input-mapping crate. Bevy 0.20-dev pinned by commit, `no_std` + alloc, `forbid(unsafe_code)`.

## Starting a session

One chunk per session, ending at the commit. To orient, in this order — it is cheap and it is the
whole bootstrap:

1. `git log --oneline -5` — what landed, and therefore where in the sequence we are.
2. `Roadmap.md`'s "Where this stands" — what is wrong and what was never built.
3. The current chunk's `###` section in `Roadmap.md`.

Everything below is on demand.

## The documents, and when to open one

Every section is numbered, so a known target can be reached with `grep -n` for the anchor and
`sed -n` for the span. Use that for lookups; see "Context" below for when not to.

| File | Holds | Reach for it when |
| --- | --- | --- |
| `docs/design.md` | how the crate works, `§1`–`§11` | you need the shape of a thing before changing it |
| `docs/decisions.md` | why it is that way, `D1`–`D57` | a decision looks arbitrary, or you are about to reverse one |
| `Roadmap.md` | what is left, what is broken, the chunk sequence | **start here for any chunk** |
| `Requirements.md` | ~220 numbered requirements, `R<section>.<n>`, open questions `OQ-n` | you need to know what must be true, or a chunk cites an R-number |
| `docs/comparison.md` | how this crate differs from BEI and LWIM | someone asks why this exists |
| `docs/one-way-doors.md` | what stops being revisable if an input crate goes upstream | upstreaming comes up |
| `docs/plan.md` | the document refactor, phase by phase | you are working on the documents themselves |

`archive/` holds the retired `Design.md`, `Log.md` and `Log-archive.md`. **Nothing in flight reasons
from them** — they describe the crate as it was, two of them are longer than anything in `docs/`,
and their content has been extracted into `docs/design.md` and `docs/decisions.md`. Do not orient
from them.

**`Requirements.md`'s `D1`–`D9` are its own numbering**, defined in its "Resolved decisions" table,
and are *not* the same identities as `docs/decisions.md`'s. Reconciling the two is phase 5's job;
see `docs/plan.md`. Until then, read a `D`-number against the document it appears in.

## Workflow

Work proceeds in numbered **chunks** defined in `Roadmap.md`. Chunk numbers are stable identities,
not positions.

- **The author commits.** Never run `git commit` or `git push`. Produce the commit message as text
  in your reply and stop there.
- **A landed chunk** gets a row in `Roadmap.md`'s "What has landed" index, and its `###` section
  deleted from the sequence. What it taught, if durable, becomes an entry in `docs/decisions.md`.
- **Ground rule 5 is the one that bites:** nothing outstanding may be left without a destination. An
  item with no chunk number is an item that will be dropped. "Later" and "its own decision" are not
  destinations — the deferred table with a stated gate is.

## Ground rules

1. **One chunk, one reviewable change.** Each chunk is a single branch: code, its tests, and any doc
   changes it forces. If a chunk turns out to be more than roughly a day's reading, it gets split
   before it gets written.
2. **Every chunk is verifiable on its own.** Pure-data chunks get unit tests; chunks that touch ECS
   get either a headless `App` test or a runnable example. No chunk lands whose only justification
   is "the next one needs it".
3. **The examples are the acceptance test.** There is always something to run. When a chunk is an
   internal change, the criterion is that *the examples do not change* — a diff in `examples/`
   during a refactor chunk is a signal the abstraction leaked.
4. **Deliberate omissions are stated.** Each chunk lists what it does *not* do, so review can tell
   "not yet" from "overlooked".
5. **Nothing outstanding is left without a destination.** A chunk that lands short of its own
   description says so, and the obligation is written onto the chunk that finishes the job.

## Where a piece of prose belongs

Each document admits one kind of thing, and the test is what distinguishes them.

| Document | Admits | The test |
| --- | --- | --- |
| `docs/design.md` | mechanism, present tense | Would this still be true after the reason for it is forgotten? |
| `docs/decisions.md` | decisions expensive to reverse | Name what breaks if it is reversed. If you cannot, it is a code comment. |
| `Requirements.md` | normative statements | Can it be violated? If nothing could violate it, it is design. |
| `Roadmap.md` | work not done, and gaps | Does it name something that will change? If it describes the present, it is design. |
| `CLAUDE.md` | process | Is it about the work rather than about the crate? |

**What has been built is described in exactly one place, `docs/design.md`.** Four documents used to
describe it; that duplication is what the refactor removed, and re-introducing a second description
anywhere is the regression to watch for.

## House style

This crate is a candidate for upstream inclusion, and game developers as a class are sensitive to
text that reads as machine-authored. The standard is: **the code should read as though a maintainer
of the surrounding codebase wrote it.**

**Internal comments are terse.** Don't explain what a maintainer already knows — ECS semantics,
borrow rules, standard Bevy behaviour. Comment the non-obvious decision: the thing that would break
if someone changed it. They may go into detail on an algorithm or a theory of operation where that
is genuinely needed, and they keep their `R`-number and `docs/` references.

**Doc comments are the exception, and address a different reader.** They are a library's public
documentation, written for a game developer who wants to use the crate, so **pedagogy is the ruling
principle**: explain the concept, show the usage, say why it matters. Length is whatever teaching
that costs and not a word more — being public licenses clarity, not loquacity. Don't explain how it
is implemented, and never cite a requirement number, a `§`, an `OQ`, a decision or a chunk: users do
not care, and on docs.rs the documents being cited do not exist. The reasoning usually survives the
edit — drop the parenthesis, keep the sentence.

**Analysis belongs in the review conversation.** The reasoning that produced a design — why an
alternative was rejected, what the trade-off was — goes in the chunk's discussion, and where it
needs to persist it goes in `docs/decisions.md`. What it must not do is accumulate as prose in the
code, or in the requirements.

**`Requirements.md` is the constitution, not the Federalist Papers.** A requirement states what must
be true and stops. It may carry whatever *structure* it takes to say that precisely — enumerated
states, a table of cases, a worked example — because that structure is the requirement. What it may
not carry is the argument. One clause of reason where a requirement needs one to be intelligible;
not a paragraph defending the choice. A **withdrawn** requirement is the exception: it keeps enough
to stop the idea being re-proposed, which is the only argument in the document with a job to do.

**Avoid the tells:** restating what the code says, hedging, enumerating the obvious, unusual
punctuation or phrasing.

**Prefer sketches over applied refactors.** Small, staged, individually reviewable edits — ground
rule 1 applied within a chunk.

**User-facing prose tone.** Doc comments and the README are the user manual, and the internal
documents' voice does not transfer. Watch for retired metaphors ("load-bearing", "seam", "fold",
"reach for" instead of "use") that ask a reader to learn what a word means here before the sentence
parses; for defending anything; for manufactured significance ("Crucially,", "It's worth noting
that"); for project history; for hedges that gate no real exception; and for a constant diet of
em-dash asides, which reads as a fingerprint once noticed. Concise is not thin — cut the padding
around every parameter, panic and edge case, not the content.

Prose in the markdown documents wraps at 100 columns; tables are exempt.

## Commit messages

No `Co-authored-by` for the LLM — Bevy's AI policy wants a disclosure section instead. Effectively
all the code here is LLM-authored to a human's direction, so the disclosure is a standing fact
rather than a per-commit judgement, and one line does it. Name the model that actually did the
session's work, not whichever one wrote the last commit — check rather than copy forward:

```
LLM Usage Disclosure: implementation, tests and documentation written by
{model}; design decisions, review and acceptance by the author.
```

Say more only where a commit departs from that — where the model chose something the author would
otherwise have decided, or where the author wrote the code and the model reviewed it.

## Verification

```sh
cargo fmt --check
cargo test --all-features
cargo clippy --all-features --all-targets
cargo clippy --no-default-features --features libm     # the no-devices build
```

Whenever a `cfg` group changes, build all eight device-feature combinations — a configuration nobody
has ever built is where the breakage hides:

```sh
for f in "" keyboard mouse gamepad keyboard,mouse keyboard,gamepad mouse,gamepad keyboard,mouse,gamepad; do
  cargo check --no-default-features --features "std,bevy_reflect,$f" || echo "FAILED: [$f]"
done
```

**Known, not regressions:** doctests compile but fail to *run* (`dynamic_linking` on the `bevy`
dev-dependency — chunk 28 owns the fix). Everything else is warning-free in every configuration
above, so a warning is a regression — treat one as such rather than assuming it was already there.

## Context, and what not to economize on

Sessions run out before the work does. But the value of working this way is noticing that two widely
separated things disagree, and that is not free — so cut waste, not reading. Distinguish:

- **A lookup**: you know what you need and where it lives. Go straight there. Reading the whole file
  is waste.
- **A sweep**: you are looking for something you do not yet know to look for — a decision that moved
  and left a stale copy behind, a requirement the change quietly contradicts, a `cfg` group that now
  spans a configuration nobody has built. This costs context and is worth it, and it has repeatedly
  found things no diff would have shown.

What to scope down is **tool output**: `| head`, `| wc -l`, `git diff` rather than a sweep over four
files dumped into the transcript to find five long lines. The Verification commands are the biggest
offender — `cargo test --all-features` alone prints one line per test — so filter rather than let it
scroll: `| grep -E "FAILED|error|test result"` for a run you expect to pass, and only drop the
filter when something actually fails and you need to see which test. This costs nothing in accuracy;
it is pure waste to keep paying for it.

**Say when the problem has outgrown the window.** A large change held half in view produces
confident work with holes in it, which is worse than the same change split in two. If something
needs more of the tree in mind at once than is left, say so and propose the split rather than
starting it and hoping.
