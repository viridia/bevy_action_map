# Plan: refactoring the documents

The project's documents have accumulated four separate descriptions of what has been built, and the
reasoning behind the work is spread across all of them. This plan replaces them with a set where
each document answers one question, and each carries the rule that keeps it from drifting back.

`docs/design.md` is written and is phase 0. Everything below follows from it existing.

**Every phase is a rewrite-up from named sources, never an edit-down.** A pass that opens the old
document and deletes from it reproduces its structure and keeps its slop; a pass that opens the old
document, reads it, and writes a new one from the sources named here does not.

---

## 1. The target set

| Question a session asks | Document | Status |
| --- | --- | --- |
| How does it work now? | `docs/design.md` | written |
| Why is it this way — can I change it? | `docs/decisions.md` | written |
| What must stay true? | `Requirements.md` | phase 5, optional |
| What is left, and what is broken? | `Roadmap.md` | phase 3 |
| How do I work here? | `CLAUDE.md` | phase 4 |
| How does it compare? Should it go upstream? | `docs/comparison.md`, `docs/one-way-doors.md` | unchanged |
| What is this crate? | `README.md` | unchanged |

Moved to `archive/` at the end: `Design.md`, `Log.md`, `Log-archive.md`.

### What each document admits

Stated in the document itself, because the rule is what stops the drift. Each is an **exclusion**
rule first: it says what to leave out.

| Document | Admits | The test |
| --- | --- | --- |
| `docs/design.md` | mechanism, present tense | Would this sentence still be true after the reason for it is forgotten? |
| `docs/decisions.md` | decisions expensive to reverse | Name what breaks if it is reversed. If you cannot, it is a code comment. |
| `Requirements.md` | normative statements | Can it be violated? If nothing could violate it, it is design. |
| `Roadmap.md` | work not done, and gaps | Does it name something that will change? If it describes the present, it is design. |
| `CLAUDE.md` | process | Is it about the work rather than about the crate? |

The consequence worth stating on its own: **"what has been built" is described in exactly one place,
`docs/design.md`.** Roadmap's "Works today" prose is deleted rather than moved. Its "What has
landed" table survives as a bare index of chunk numbers — an index names the work, it does not
describe the crate — and the Log's per-chunk delivery record is git's job.

---

## 2. The section map

This table does two jobs. It is the source list for phases 1–2 — the right-hand column says where
each old section's *argument* goes, once its mechanism has already been extracted. And it is the
remap table for phase 4, where 23 live citations of `Design §n` have to be repointed.

| `Design.md` § | Subject | Mechanism now in | Argument goes to |
| --- | --- | --- | --- |
| 1 | Architecture | design §1 | D: layer seam, L2 reads only L1 |
| 2 | Data flow through one frame | design §2 | D: event queue vs. level sampling; the timestamp shim |
| 3 | Core object model | design §3 | D: action as a type |
| 3.1 | Naming actions and contexts | design §3.3 | D: declared path, not type path |
| 4 | The compiled plan | design §4 | D: compile once, share by `Arc` |
| 4.1 | Where class bindings do not fit | **design §5.4** | D: two structures, not an expanded class |
| 5 | Evaluation pipeline | design §5 | — |
| 5.1 | Folding several bindings | **design §5.5** | D: intent decides the fold |
| 5.2 | Consumption across tick domains | design §5.2 | D: consumption flows forward in schedule order |
| 5.3 | Exclusive contexts | design §5.3 | D: a ceiling, not a third context state |
| 6 | State and storage | design §6 | D: two tables, uniform scratch (closes OQ-3 / D9) |
| 7 | Tick domains | design §1, §3 | D: one domain per context (closes OQ-6) |
| 8 | Extensibility | design §8.2 | D: enum plus `Custom` (closes OQ-5) |
| 8.1 | The deadzone stages | **design §8.4** | D: three stages, one rescaling (D6) |
| 9.1–9.2 | Worked examples | README, rustdoc | — |
| 9.3 | What the derives generate | design §3 | D: registry keyed by path, not the reflect registry |
| 9.4 | Binding combinators | design §8.2 | — |
| 9.5 | Diagnostics — three tiers | design §4 | D: the three tiers |
| 9.6 / 9.6.1 | Observers, BSN | **design §5.6** | D: generic `EntityEvent` on the context entity |
| 9.7 | The presentation surface | **design §9.1** | D: D7 split; four listing states; slots and capacity; `follow` |
| 9.8 | Navigating a screen | design §8.3 | D: two combinators rather than a navigation path |
| 10.1 | The overrides structure | design §10 | D: diff against defaults; variant plan; three row states |
| 10.2 | What Steam's IGA file is | — | D: what it is a model for, and what it is not |
| 10.3 | Naming a control | **design §9.2** | D: one name as identity and key; our table, not Bevy's |
| 10.4 | Capture | **design §9.3** | D: reads L1 directly; the refusal order; conflicts not resolved |
| 10.5 | The backend seam | — (nothing built) | D: authority writes a value, not a state; nothing names Steam |
| 10.6 | The reverse lookup | **design §9.2** | D: a trait, `ControlOrigin`, and the ranking refused |
| 10.7 | Prompts on screen | **design §9.2** | D: the crate keeps the lookup, not the drawing |
| 10.8 | Brand resolution | — (open) | D: the generic tier, recorded as unresolved |
| 10.9 | Device pairing, join gesture | **design §7.4** | D: runtime handle, not persistent identity |
| 11 | Crate structure | design §11 | D: one crate; what must not move upstream |
| 12 | Consequences, tensions, risks | — | D, and the Roadmap defect register |

Bold entries are the ones whose number **moved**. The rest kept theirs.

### The 23 citations to repoint (phase 4)

| Where | Cites |
| --- | --- |
| `src/lib.rs:275`, `src/lib.rs:277` | §5.2, §5.3 — unchanged |
| `src/eval.rs:223` | §5.3 — unchanged |
| `src/context.rs:160` | §6 — unchanged |
| `src/plan.rs:559`, `src/eval.rs:359` | §4.1 → **§5.4** |
| `examples/capture.rs:196` | §9.7 → **§9.1** |
| `Requirements.md` × 8, `docs/comparison.md` × 6, `docs/one-way-doors.md` × 2 | mixed — read each |

Citations inside `Roadmap.md` (14) and `CLAUDE.md` (2) need no separate pass; those documents are
rewritten in phases 3 and 4 anyway. Citations inside `Log.md` and `Log-archive.md` (13) go to
`archive/` with them and are not repointed.

**This is a reading, not a find-and-replace.** Some of the 23 point at a section whose argument has
moved to `decisions.md` rather than at one whose number changed, and the correct repoint is to the
decision, not to a design section.

---

## 3. `docs/decisions.md`

**One file, numbered `D1..Dn`, with a one-line index at the top** so `grep -n` reaches a known
target. One file rather than several because the admission rule already does the sorting a subject
split would ask for per entry — and because `D`-numbers have **zero citations in code**, so the
numbering is free to be assigned afresh. (`R`-numbers have 52 and are not.)

`D1`–`D9` in `Requirements.md` are absorbed rather than preserved; nothing outside the documents
depends on those identities.

Per entry:

> **What was decided.** One or two sentences.
> **What it rules out.** The alternatives it forecloses, named.
> **What reversal would cost.** Concretely — an API break, a save-format break, a reshaped pipeline.

The load-bearing few get the fuller treatment `docs/one-way-doors.md` already uses on someone else's
crate; that document is the house format for exactly this, and turning it on ourselves is the point.

Estimated at 25–40 entries and 500–800 lines. **It came out at 53 and 1083.** The overshoot is
concentrated in overrides and persistence, where the admission rule explicitly names the save format
and there are seven distinct rows that breaking would corrupt a player's file. Every entry passes
the test; nothing was padded. If it wants shortening later, the honest lever is merging D45 with D46
and D49 with D50, not dropping an entry.

The estimate mattering here is phase 5's: `Requirements.md` is larger than `Design.md` was, and this
one ran 35% long.

Two things land here that have no other home:

- **Deliberate non-fixes.** Roadmap's "Known wrong today" mixes defects with accepted limitations.
  The per-device gamepad-state entry is not a defect — it is a limitation with a stated price ("a
  per-device map in every context instance") and a decision not to pay it. Those become decisions.
  Actual defects stay in `Roadmap.md`.
- **Design commitments inside the upstreaming stance.** Nothing under `src/` names Steam; no
  `bevy_ui` dependency. The stance *itself* gates chunks and stays in `Roadmap.md`; the commitments
  it implies are decisions.

---

## 4. Phases

| Phase | Work | Sources | Sessions |
| --- | --- | --- | --- |
| 0 | `docs/design.md` | old `Design.md`, the code | **done** |
| 1 | `docs/decisions.md` D1–D26 — pipeline and model | `Design.md` §1–§8, §9.3, §9.5, §9.6, §11, §12; `Requirements.md` D1–D9 and the OQ closures | **done** |
| 2 | `docs/decisions.md` D27–D53 — presentation and persistence | `Design.md` §9.7, §9.8, §10; the OQ-9 closure | **done** |
| 3 | `Roadmap.md`, plus the archive sweep below | its 18 remaining chunk sections, the deferred table, the landed index, "Known wrong today" | **done** |
| 4 | `CLAUDE.md`; move three documents to `archive/`; repoint 22 citations | Roadmap's ground rules, house style, commit format | **done** |
| 5 | `Requirements.md` — **optional**, and it now has an obligation | itself, section by section; the `D`-number reconciliation below | 3+ |
| 6 | Implementation scan — what is overbuilt, underbuilt or wrongly built | the non-archived documents, and the code | 3 |
| 7 | Comment scan — the prose in the code, against the same documents | the same | 3 |

Phase 1's sources ran wider than this table originally said: §9.3, §9.5, §9.6 and §11 are model and
pipeline rather than presentation, so they were taken there rather than left for phase 2.

### What phase 2 handed to phase 3

**An archive sweep that phase 2 did not do.** The plan named "the durable half of `Log.md` and
`Log-archive.md`" as a source. `Log.md`'s recent entries were read and one archive entry was
spot-checked; the other 28 were not, on the reasoning that `Design.md` §9.7 and §10 are the
consolidated form of the chunks that produced them. Chunk 54 is a counterexample to exactly that
reasoning — it records that a crate-side `ConflictPolicy` enum and a resolving `Overrides::rebind`
were *built and rejected on review*, which `Design.md` only gestures at, and which became D43 and
D53. So the reasoning is weaker than it looked, and the entries whose titles name a rejected
approach or a withdrawn requirement are worth reading before the archive moves in phase 4:

`The review, and the requirements amendments it produced` · `The second grooming, after §§9–16` ·
`Reading bevy_enhanced_input's state integration` · `Chunk 32: activation by run condition` ·
`Chunk 53: a context the player never sees` · `The persistence design, before any of it was built` ·
`The Steam grooming, before chunks 40 and 38` · `Focus-driven dispatch, and R22.7 withdrawn`

About 400 lines. Anything durable found there is a late entry in `decisions.md`, appended rather
than renumbered.

**Four things phase 2 declined to admit, which now need a destination.**

| From | Why it is not a decision | Goes to |
| --- | --- | --- |
| `Design.md` §10.8, brand resolution and the generic tier | explicitly unresolved — `fallback_label` is brand-agnostic "by omission, not by decision" | deferred table, with a gate |
| `Design.md` §10.2, what Steam's IGA file is | comparative analysis of prior art, not a choice this crate made | `comparison.md`, or dropped |
| `Design.md` §12, "R23.2 is unenforced" | a risk with no tooling behind it | defect register |
| `Design.md` §12, "the derive carrying too much" | relieved when player-facing names became keys | dropped |

**Five decisions carry an unresolved remainder**, each marked `Still open` in `decisions.md`, and
ground rule 5 wants every one of them a row in the deferred table with a stated gate:

| | Remainder |
| --- | --- |
| D4 | gamepad timing stays frame-quantized until gilrs polling is rewritten upstream |
| D9 | schedule enforcement is not airtight — Bevy gives a `SystemParam` no way to know its schedule |
| D13 | the exclusion ceiling is global, so one player's modal shadows every player |
| D22 | whether an authority backend's actions can participate in rollback at all |
| D52 | owner-scoping consumption and the exclusion ceiling themselves |

### Phase 3 — what `Roadmap.md` keeps

Keeps: the remaining chunk sections; the deferred table with its gates; the ground rules *pointer*
(the statement itself moves to `CLAUDE.md` in phase 4); the defect register, which is "Known wrong
today" minus the accepted limitations; the upstreaming stance, because it gates chunks.

Keeps, trimmed: **the landed index.** Chunk numbers are stable identities, cited in commit messages
and in eleven code comments, so the sequence has to stay recoverable. It becomes two columns —

| # | Chunk |
| --- | --- |
| 8 | Gamepad and the design-stage deadzone |

— dropping the `State` column. That column's forward-references ("stages 1 and 3 → 22") were
tracking obligations, and ground rule 5 already guarantees every open one a row in the deferred
table; a second copy in a history index is one of the four descriptions this refactor is removing.

Deletes: **"Works today."** `docs/design.md` is that list, and a second copy is the exact failure
this refactor exists to end.

### Phase 4 — the archive

`Design.md`, `Log.md` and `Log-archive.md` move to `archive/` — `git mv`, not deletion, so the
history stays browsable in the working tree until it is clearly not wanted. `CLAUDE.md`'s document
table loses their rows and gains one line saying `archive/` exists and that nothing in flight
reasons from it.

`Log.md`'s stated job is "reach for it when a decision looks arbitrary" — which is `decisions.md`'s
job. Its other half, what each chunk delivered, is `docs/design.md` plus git. It moves **last**,
after `decisions.md` exists, so the extraction can be checked against it first.

`CLAUDE.md` absorbs the full statement of the ground rules, house style and commit format, which
`Roadmap.md` currently holds and `CLAUDE.md` currently summarizes. Removing the hop is most of the
point: the always-loaded document should carry the rules that apply to every session, not a pointer
to them.

### What phase 4 handed to phase 5

**Two `D`-numbering schemes now coexist, and phase 4 deliberately did not merge them.**
`Requirements.md` carries 48 references to its own `D1`–`D9`, defined in its "Resolved decisions"
table. `docs/decisions.md` assigned its numbers afresh, on the stated grounds that nothing outside
the documents cited a `D`-number. That was true of the *code* and false of `Requirements.md`, which
phase 1's count did not separate.

Nothing dangles — every `D`-reference in `Requirements.md` resolves against that document's own
table — so this is duplication, not breakage, and it is phase 5's to resolve. It was not done in
phase 4 because a mechanical remap is exactly the kind that fails silently:

| `Requirements.md` | `docs/decisions.md` | |
| --- | --- | --- |
| D1 actions are types | **D5** | |
| D3 external backends | **D22**, extended by **D51** | |
| D4 focus by activation | **D23** | D4 and D5 both land here — |
| D5 interception is static | **D23** | the merge loses which half was cited |
| D6 dead-zone chain | **D20**, with **D21** | |
| D7 presentation separate | **D27** | |
| D8 declared path | **D6** | collides with the old D6 |
| D9 state layout | **D8** | collides with the old D8 |

Two collisions and one merge. A sequential find-and-replace maps old D8 to new D6 and then that new
D6 onward again; the D4/D5 merge needs a reading of each site to know which half it meant. Do this
by reading, or leave `Requirements.md`'s numbering alone and say in its own preamble that it is
local — which is the cheaper answer and may be the right one.

Until then, `CLAUDE.md` tells a reader to interpret a `D`-number against the document it appears in.

### Phase 5 — why `Requirements.md` ranks last

It is the largest document, the **least** duplicative, and the only one with live code citations —
52 of them. Its slop is real, but it has the worst ratio of risk to relief in the set, and phases
1–2 will drain some of it into `decisions.md` on their own. Decide whether to do it after seeing how
much is left.

Its numbering is load-bearing either way: an `R`-number is an identity, and a rewrite that renumbers
breaks 52 comments in the code.

### Phases 6 and 7 — the scans

The reason the refactor is worth doing, and the test of whether it worked. The documents are the
**only** source of truth: `README.md`, `docs/`, `Requirements.md`, `Roadmap.md`, `CLAUDE.md`.
Nothing in `archive/` is consulted, and neither is git history — if a fact is not in the surviving
set, it does not exist for the purpose of the scan.

Two passes, deliberately separate, because judging prose and judging behaviour want undivided
attention and running them together produces a worse job of both.

**Phase 6 — the implementation.** Three categories, each with its own method, because they are not
found the same way:

| | What it is | How it is found |
| --- | --- | --- |
| **Overbuilt** | public surface no document asks for | enumerate every `pub` item, check each against `design.md` and `Requirements.md` |
| **Underbuilt** | a normative statement with no implementation behind it | walk `Requirements.md` section by section against the code |
| **Wrongly built** | code that contradicts a document | read `design.md`'s claims against what the code does |

**Excluded, and this is the load-bearing part.** Anything already recorded in `Roadmap.md`'s defect
register or deferred table, or marked `Still open` in `decisions.md`, is not a finding. Phase 3
therefore decides what phase 6 is able to report: a defect missing from the register arrives as
news, and a stale entry silently suppresses a real finding.

**Temper the expectation on "wrongly built".** Phase 0 reconciled every disagreement it found
between `Design.md` and the code *in favour of the code* — `ActionState`'s fields, the module tree,
the persistence table names. So that category starts from documents already trued against the
implementation, and the yield will be in what phase 0 did not look at: the code against
`Requirements.md`, which no phase has checked in either direction.

**Phase 7 — the comments.** Doc comments are user-facing and render on docs.rs where our documents
do not exist, so the rule they are checked against is that none may cite an `R`-number, a `§`, an
`OQ` or a chunk. Internal comments keep their references and are checked for being *true* — which is
why this runs after phase 4, when the 23 `Design §` citations have been repointed.

**Both need scoping; neither fits one window.** `src/` is 20,000 lines and the surviving documents
are 3,000. The split that falls out of the layering is three sessions each: `device` and `frame`;
then `action`, `binding`, `condition`, `context`, `plan`, `eval` and `event`; then `mapping`,
`overrides`, `preset`, `capture`, `present`, `inspect`, `player` and `join`.

**If phase 5 is skipped**, phase 6 runs against `Requirements.md` as it stands, which is already
normative. The scans depend on phase 4, not on phase 5.

---

## 5. What could go wrong

- ~~**`decisions.md` becomes a second Log.**~~ _It did not. All 53 entries answer the admission
  test, and four candidates were turned away by it — chunks 74 and 75 as internal refactors, and the
  two `Design.md` §12 items in the table above. What the rule does **not** bound is volume: it
  governs what gets in, not how much, which is how the document ran 35% past its estimate while
  every entry earned its place. Phase 5 should size from that._
- ~~**A decision is lost between phases 1 and 2.**~~ _Partly. The section map worked as a checklist
  and caught §9.5's diagnostic tiers missing after the first twenty-five were numbered, which
  became D26 and is why the numbering has one deliberate jump. It could not catch the archive gap,
  because the map covers `Design.md` and nothing else — which is the hole the sweep above closes._
- **Phase 4 repoints a citation to the wrong place.** Some of the 23 point at an argument rather
  than at a mechanism. Read each one; a find-and-replace over `§4.1` would silently send a reader to
  a section about class-binding *mechanism* when the comment cited the reason it works that way.
- **`archive/` is read as current.** It is three documents that describe the crate as it was, two of
  them longer than anything in `docs/`. The one line in `CLAUDE.md` has to say plainly that nothing
  in flight reasons from them, or a future session will orient off a stale `Design.md`.
