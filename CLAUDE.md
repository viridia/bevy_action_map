# Working on `bevy_action_map`

A Bevy input-mapping crate. Bevy 0.20.0-rc.1 from crates.io, `no_std` + alloc,
`forbid(unsafe_code)`.

## Starting a session

One chunk per session, ending at the commit. To orient, in this order — it is cheap and it is the
whole bootstrap:

1. `git log --oneline -5` — what landed, and therefore where in the sequence we are.
2. `docs/issues.md` for what is known to be wrong, and `Roadmap.md`'s "Where this stands" for what
   was never built.
3. The current chunk's `###` section in `Roadmap.md`.

Everything below is on demand.

## The documents, and when to open one

Every section is numbered, so a known target can be reached with `grep -n` for the anchor and
`sed -n` for the span. Use that for lookups; see "Context" below for when not to.

| File | Holds | Reach for it when |
| --- | --- | --- |
| `docs/design.md` | how the crate works, in sections `TD<n>` | you need the shape of a thing before changing it |
| `docs/decisions.md` | why it is that way, in entries `D<n>` | a decision looks arbitrary, or you are about to reverse one |
| `Roadmap.md` | what is left, what is broken, the chunk sequence | **start here for any chunk** |
| `Requirements.md` | numbered requirements, `R<section>.<n>` | you need to know what must be true, or a chunk cites an R-number |
| `docs/comparison.md` | how this crate differs from BEI and LWIM | someone asks why this exists |
| `docs/one-way-doors.md` | what stops being revisable if an input crate goes upstream | upstreaming comes up |
| `docs/issues.md` | findings awaiting routing, in five tiers by severity | a finding needs routing, or you are about to re-find one |
| `docs/guidelines.md` | how this project judges scope and shapes code, in entries `G<n>` | you are about to scope a chunk or write code |
| `docs/deferred.md` | work decided against for now, in entries `X<n>`, each with its gate | a gate may have fired (a Bevy bump is the first group), or you are about to defer something |
| `docs/steam.md` | what a running Steam client actually does, in entries `S<n>` | a decision rests on how an external backend behaves |
| `bevy_remote_driver/docs/requirements.md` | numbered requirements for the remote test driver, `DR<section>.<n>` | a chunk touches the driver, or cites a DR-number |
| `bevy_remote_driver/docs/design.md` | how the driver works, in sections `DD<n>` | you are writing or running an end-to-end test against a live example — see "Verification" for the command |

`archive/` holds the retired `Design.md`, `Log.md` and `Log-archive.md`. **Nothing in flight reasons
from them** — they describe the crate as it was, two of them are longer than anything in `docs/`,
and their content has been extracted into `docs/design.md` and `docs/decisions.md`. Do not orient
from them.

There is **one `D`-numbering** in the project, defined in `docs/decisions.md`. `Requirements.md`
tags requirements with it; it used to carry a rival `D1`–`D9` of its own, and does not any more.

Two documents have numbered sections, and each has its own prefix so a reference never needs to know
which document it is standing in. `Requirements.md`'s sections are `R<n>`, told from the
requirements inside them by the dot: `R19` is the section, `R19.14` a requirement in it.
`docs/design.md`'s are `TD<n>`, with subsections such as `TD5.3`. The section sign these replaced is
retired, and `scripts/xref.py` fails on one: any that survives is a reference nothing migrated.

A requirement is defined once, as a list item reading `- **R<section>.<n> (MUST)**`, and is cited
bare everywhere else. So to reach a definition, search for `**` followed by the number: the `**`
prefix appears nowhere but the definition. Read it to the next `- **R` line: indented sub-bullets
are part of the requirement, and a fixed `grep -A` cuts them off.

## Workflow

Work proceeds in numbered **chunks** defined in `Roadmap.md`. Chunk numbers are stable identities,
not positions.

- **The author commits.** Never run `git commit` or `git push`. Produce the commit message as text
  in your reply and stop there.
- **A landed chunk** gets a row in `Roadmap.md`'s "What has landed" index, and its `###` section
  deleted from the sequence. What it taught, if durable, becomes an entry in `docs/decisions.md`.
- **Ground rule 5 is the one that bites:** nothing outstanding may be left without a destination. An
  item with no chunk number is an item that will be dropped. "Later" and "its own decision" are not
  destinations — an entry in `docs/deferred.md` with a stated gate is.
- **A new chunk gets a `###` section** and nothing else. The "Next" list is the author's shortlist.
- **A structural idiom is proposed before it is built.** A new kind of function, module dependency
  or wrapper arrives as options with a recommendation, not in a diff.
- **A plan opens with what changes**, in sentences that assume nothing, and answers a question about
  scope or complexity with counts rather than assurance.
- **Never post publicly.** No issue, PR or comment on a public forum, through `gh` or anything else,
  even when asked to "file an issue": write a short brief for the author to edit and post. Bevy's AI
  policy is why.

## Ground rules

1. **One chunk, one reviewable change.** Each chunk is a single branch: code, its tests, and any doc
   changes it forces. If a chunk turns out to be more than roughly a day's reading, it gets split
   before it gets written. Two items that could share a chunk default to two, with an ordering note.
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
| `docs/issues.md` | findings not yet routed | Is something wrong, and has no chunk taken it? A finding with a chunk belongs to the chunk. |
| `docs/guidelines.md` | judgement a maintainer would otherwise learn by correction | Would someone new choose differently with it? If it cannot be departed from with a reason, it is a requirement. |
| `docs/deferred.md` | work decided against for now | Has the decision been made, and is it "not yet"? Name the event that reopens it; with no event, it is dropped. |
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
is implemented, and never cite a requirement number, a section, an `OQ`, a decision or a chunk:
users do not care, and on docs.rs the documents being cited do not exist. The reasoning usually
survives the edit — drop the parenthesis, keep the sentence.

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
punctuation or phrasing. "Nothing" as an agent ("nothing reports it") is the commonest; prefer a
first-class antonym ("unranked", "dead") or name what is absent.

**More room is not more words.** Turning prose into bullets, a table or a new section keeps one line
per item; a longer explanation points at the document that owns it.

**Every term is introduced for this document's reader.** A word familiar where it was written is
used only once this reader has met it: a crates.io reader gets "the `disasteroids` example", and a
sentence beside two lists says which one it means.

**Name a rejected alternative only where a reader would reach for it**, in one clause. Once the code
exists the argument is won, and a paragraph against a ruled-out option is the dead horse.

**An introduction defines a category by kind**, not by listing or counting its members, which go
stale every chunk. Keep a number only where its change is itself a signal.

**Prefer sketches over applied refactors.** Small, staged, individually reviewable edits — ground
rule 1 applied within a chunk.

**User-facing prose tone.** Doc comments and the README are the user manual, and the internal
documents' voice does not transfer. Watch for retired metaphors ("load-bearing", "seam", "fold",
"reach for" instead of "use") that ask a reader to learn what a word means here before the sentence
parses; for defending anything; for manufactured significance ("Crucially,", "It's worth noting
that"); for project history; for hedges that gate no real exception; and for a constant diet of
em-dash asides, which reads as a fingerprint once noticed. Concise is not thin — cut the padding
around every parameter, panic and edge case, not the content.

The crate docs in `src/lib.rs` are an overview, with a ceiling of about 250 `//!` lines. An addition
there is a paragraph linking the item; a worked example goes on the item's own doc comment. Raising
the ceiling is the author's call, asked before writing.

Prose in the markdown documents wraps at 100 columns; tables are exempt. The same limit applies to
Rust comments, `///`/`//!`/`//` alike.

After editing a comment or a doc paragraph — a rename, added prose, anything that could have pushed
a line over the limit — fix the overflow with `tools/devfmt` rather than by hand:

```sh
cargo run --manifest-path tools/devfmt/Cargo.toml --quiet -- --diff
```

`--diff` (default `HEAD`) only touches paragraphs overlapping a line the working tree actually
changed, so pre-existing debt elsewhere in the file is left alone — add `--preview` first to read
exactly what it would rewrite, or `--check` for just the list of files. A paragraph it reaches is
refilled whether or not its lines already fit, since an edit shortens a line as often as it
lengthens one; a scope flag is therefore required, and `--sweep <path>` is the deliberate one.

Do the style pass on edited comments before `cargo fmt`, so formatting runs once. devfmt is the only
authority on width: `awk` and `wc` count bytes, and an em-dash is three of them.

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

`scripts/verify.sh` runs the whole recipe below in one call and is the routine way to do this —
prefer it to running each command by hand.

```sh
python3 scripts/xref.py                                 # the documents' cross-references
cargo fmt --check
cargo test --all-features --lib --tests
cargo clippy -p bevy_remote_driver --all-targets        # a workspace member the above skip
cargo test -p bevy_remote_driver
cargo clippy --all-features --all-targets
cargo clippy --no-default-features --features libm      # the no-devices build
cargo test --no-default-features --features libm --test no_devices
cargo test --no-default-features --features std,mouse,gamepad --test focus_loss_without_keyboard
```

Whenever a `cfg` group changes, build all eight device-feature combinations — a configuration nobody
has ever built is where the breakage hides. `scripts/verify.sh --full` covers this too:

```sh
for f in "" keyboard mouse gamepad keyboard,mouse keyboard,gamepad mouse,gamepad keyboard,mouse,gamepad; do
  cargo check --no-default-features --features "std,bevy_reflect,$f" || echo "FAILED: [$f]"
done
```

`--full` also runs clippy on `steam_examples/`, which is outside the workspace and links the base
Disasteroids' modules by path, so a change under `examples/disasteroids` can break it without
touching a file in it. Run `--full` when that directory changes.

Doctests are out of the default run, because the doc examples are stable and the step pays for a
separate compile of the merged doctest binary. `scripts/verify.sh --doc` adds it; run that when a
`///` example changes. By hand it needs the dyld workaround, since `dynamic_linking` on the `bevy`
dev-dependency leaves that binary without an rpath to the toolchain's own libstd:

```sh
DYLD_FALLBACK_LIBRARY_PATH="$(rustc --print target-libdir)" cargo test --workspace --all-features --doc
```

**An end-to-end run** is out of the default recipe as well, and out of the per-chunk habit: it
builds a real example, launches it, drives it over BRP and reads the scene back, which costs a
windowed app and a minute. Run one when a change could only fail in a running game, or when a
chunk's acceptance test is something a player does.

```sh
python3 bevy_remote_driver/client/run.py bevy_remote_driver/plans/disasteroids/rebind.py
```

The plans live under `bevy_remote_driver/plans/<example>/`: `disasteroids/rebind.py` covers the
controls screen and the override path, `disasteroids/pad.py` the virtual gamepad, and `testbed/` the
steps themselves. One command per run, from anywhere in the workspace — `DD9` is the instructions
for writing a plan, and says why a run cannot be split across two commands. A plan that rebinds
writes the developer's real settings file: a clean pass puts it back, a failure halfway leaves it
dirty.

**Known, not regressions:** 42 of the 51 doctests are `ignore` fences — fragments written to be read
mid-prose rather than to stand alone — so they are neither compiled nor run, and chunk 28 owns
making them execute. That workaround is macOS-only; elsewhere the doctest step skips rather than
pretending to have run. The unit tests under `src/` assume a keyboard is available and do not build
in the no-devices configuration, so `tests/no_devices.rs` is run on its own rather than as part of
the full suite. Everything else is warning-free in every configuration above, so a warning is a
regression — treat one as such rather than assuming it was already there.

**Launch every example a chunk touched** before calling it done: `cargo run --example <x>` in the
background for a minute, with the log grepped for `panicked|ERROR` (Disasteroids needs
`--features serialize`). It catches what the recipe cannot, such as a system running before the
entities it expects exist.

## Context, and what not to economize on

Sessions run out before the work does. But the value of working this way is noticing that two widely
separated things disagree, and that is not free — so cut waste, not reading. Distinguish:

- **A lookup**: you know what you need and where it lives. Go straight there. Reading the whole file
  is waste.
- **A sweep**: you are looking for something you do not yet know to look for — a decision that moved
  and left a stale copy behind, a requirement the change quietly contradicts, a `cfg` group that now
  spans a configuration nobody has built. This costs context and is worth it, and it has repeatedly
  found things no diff would have shown.
- **A scan**: a script finds a signature, and only its hits are read. Validate it against a known
  positive first; a clean result from an unvalidated scan is worth nothing. Prefer a scan to a sweep
  wherever the thing sought has a signature.

Before a tree-wide mechanical change, run it on one file of each kind and read the output. Tests
prove it correct, not legible.

What to scope down is **tool output**: `| head`, `| wc -l`, `git diff` rather than a sweep over four
files dumped into the transcript to find five long lines. `scripts/verify.sh` already does this for
the Verification commands, which used to be the biggest offender — `cargo test --all-features` alone
prints one line per test. Apply the same filtering by hand only when running one of those commands
directly: `| grep -E "FAILED|error|test result"` for a run you expect to pass, and only drop the
filter when something actually fails and you need to see which test. This costs nothing in accuracy;
it is pure waste to keep paying for it.

**Say when the problem has outgrown the window.** A large change held half in view produces
confident work with holes in it, which is worse than the same change split in two. If something
needs more of the tree in mind at once than is left, say so and propose the split rather than
starting it and hoping.
