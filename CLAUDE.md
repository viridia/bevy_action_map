# Working on `bevy_action_map`

A Bevy input-mapping crate. Bevy 0.20-dev pinned by commit, `no_std` + alloc, `forbid(unsafe_code)`.

## Starting a session

One chunk per session, ending at the commit. To orient, in this order — it is cheap and it is the
whole bootstrap:

1. `git log --oneline -5` — what landed, and therefore where in the sequence we are.
2. The **last entry in `Log.md`** — what that chunk found, which is usually what the next inherits.
3. The current chunk's `###` section in `Roadmap.md`, plus the "Where this stands" table above the
   landed list.

Everything below is on demand.

## The documents, and when to open one

Every section is numbered, so a known target can be reached with `grep -n` for the anchor and
`sed -n` for the span. Use that for lookups; see "Context" below for when not to.

| File | Holds | Reach for it when |
| --- | --- | --- |
| `Requirements.md` | ~204 numbered requirements, `R<section>.<n>`, decisions `D1`–`D8`, open questions `OQ-n` | you need to know what must be true, or a chunk cites an R-number |
| `Design.md` | how they are satisfied; sections `§1`–`§12` | you need the shape of a thing before changing it |
| `Roadmap.md` | ground rules, house style, commit format, what has landed, what is left | **start here for any chunk** |
| `Log.md` | Phase VII onward: what each chunk delivered and what it taught | a decision looks arbitrary |
| `Log-archive.md` | phases I–VI, closed | rarely; it is history |

## Workflow

Work proceeds in numbered **chunks** defined in `Roadmap.md`. Chunk numbers are stable identities,
not positions.

- **The author commits.** Never run `git commit` or `git push`. Produce the commit message as text
  in your reply and stop there.
- **A landed chunk** gets a row in `Roadmap.md`'s "What has landed" table, an entry in `Log.md`, and
  its `###` section deleted from the sequence.
- **Ground rule 5 is the one that bites:** nothing outstanding may be left without a destination. An
  item with no chunk number is an item that will be dropped. "Later" and "its own decision" are not
  destinations — the deferred table with a stated gate is.

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
dev-dependency — chunk 28 owns the fix); the `--features libm` build emits 4 pre-existing warnings.

## House style

Full statement in `Roadmap.md` § House style. The three rules most often broken:

- **Doc comments are user-facing.** They render on docs.rs, where our documents do not exist. Never
  cite an R-number, a `§`, an `OQ`, or a chunk from a `///` comment. The reasoning usually survives
  the edit — drop the parenthesis, keep the sentence.
- **Internal comments are terse**, and `pub(crate)`/`//` comments keep their references. Comment the
  non-obvious decision — the thing that breaks if someone changes it — not the mechanism.
- **`Requirements.md` is the constitution, not the Federalist Papers.** A requirement states what
  must be true and stops. Structure it needs to say that precisely — enumerated states, a table of
  cases, a worked example — is the requirement. The argument for it is not: that belongs in
  `Design.md`, or `Log.md` where a chunk learned it. One clause of reason where a requirement needs
  one to be intelligible; not a paragraph defending the choice.

Prose in these markdown documents wraps at 100 columns; tables are exempt.

## Commit messages

No `Co-authored-by` for the LLM — Bevy's AI policy wants a disclosure section instead. Standard
footer, unless the commit genuinely departed from it. Name the model that actually did the session's
work, not whichever one wrote the last commit — check rather than copy forward:

```
LLM Usage Disclosure: implementation, tests and documentation written by
{model}; design decisions, review and acceptance by the author.
```

## Context, and what not to economize on

Sessions run out before the work does. But the value of working this way is noticing that two widely
separated things disagree, and that is not free — so cut waste, not reading. Distinguish:

- **A lookup**: you know what you need and where it lives. Go straight there. Reading the whole file
  is waste.
- **A sweep**: you are looking for something you do not yet know to look for — a decision that moved
  and left a stale copy behind, a requirement the change quietly contradicts, a `cfg` group that now
  spans a configuration nobody has built. This costs context and is worth it. Chunk 43's two rename
  regressions, and the modelling gap that became chunk 44, were all found this way and none of them
  was visible from a diff.

What to scope down is **tool output**: `| head`, `| wc -l`, `git diff` rather than a sweep over four
files dumped into the transcript to find five long lines. The Verification commands are the biggest
offender — `cargo test --all-features` alone prints one line per test — so filter rather than let it
scroll: `| grep -E "FAILED|error|test result"` for a run you expect to pass, and only drop the filter
when something actually fails and you need to see which test. This costs nothing in accuracy; it is
pure waste to keep paying for it.

**Say when the problem has outgrown the window.** A large change held half in view produces
confident work with holes in it, which is worse than the same change split in two. If something
needs more of the tree in mind at once than is left, say so and propose the split rather than
starting it and hoping.
