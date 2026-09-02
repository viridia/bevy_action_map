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
| 5 | `Requirements.md` | itself, section by section; the `D`-number reconciliation below | **done**, in 1 |
| 6 | Implementation scan — what is overbuilt, underbuilt or wrongly built | the non-archived documents, and the code | 2 of 6 done |
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

### Phase 5 — what it turned out to be

Estimated at 3+ sessions and it took one, because phases 1–2 had drained more than the estimate
assumed. What was actually left:

- **86 lines of pure duplication** — the `D1`–`D9` table and the resolved-`OQ` list, both fully
  carried by `decisions.md`. Deleted, replaced by a pointer.
- **53 `D`-references remapped** through unique placeholders, so the two collisions could not
  double-map. Verified by checking every remapped tag lands on a decision whose title matches the
  requirement's subject.
- **Only ~14 of the 24 italic asides were argument**; the rest were `_(D8)_`-style tags. Eight were
  trimmed or deleted, six kept — the enumerated-state explanation under R3.1 because structure *is*
  the requirement, the withdrawn half of R3.6 because a withdrawn requirement's argument is what
  stops re-proposal, and four one-clause reasons that make their requirement intelligible.
- **The wrap**, reflowed to 100 columns and verified by asserting the word sequence byte-identical
  before and after.

Two staleness finds the pass turned up rather than the diff: `OQ-5` was still listed **open** when
`D19` had resolved it, and `OQ-10` was referenced at R4.7 and never defined anywhere.

**One regression caught in verification, worth recording.** The first reflow wrapped the 39 link
definitions as if they were prose, merging them into one paragraph and breaking every reference link
in the document. The word-sequence check passed — the words were all still there, in order — so
*content* verification alone did not catch it. What caught it was counting structural line types
before and after. Any future reflow needs both checks.

### Phase 5 — why it had ranked last

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

**Phase 6 — the implementation.** Four categories, each with its own method, because they are not
found the same way:

| | What it is | How it is found |
| --- | --- | --- |
| **Overbuilt** | machinery out of proportion to the problem — a map where four items ever exist, a type parameter nothing reads, one fact stored three ways | read each type against the sizes and the cases the documents say it actually meets |
| **Unasked-for** | public surface no document asks for | enumerate every `pub` item, check each against `design.md` and `Requirements.md` |
| **Underbuilt** | a normative statement with no implementation behind it | walk `Requirements.md` section by section against the code |
| **Wrongly built** | code that contradicts a document | read `design.md`'s claims against what the code does |

**Session 1 ran the second of these under the first's name**, so `device` and `frame` have had the
public-surface sweep and not the proportion one. Whichever chunk takes their findings picks it up.

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

**Both need scoping; neither fits one window.** `src/` is 20,000 lines, of which 11,500 are code,
and the surviving documents are 3,000. The split was estimated at three sessions each by layer;
sessions 1 and 2 measured the unit at about 2,000 lines of code plus the documents they touch, which
makes it six:

| | Modules | Code |
| --- | --- | --- |
| 1 | `device`, `frame` | 707 |
| 2 | `action`, `condition`, `event`, `plan` | 2,112 |
| 3 | `binding` | 2,360 |
| 4 | `context`, `eval` | 2,498 |
| 5 | `overrides`, `preset`, `mapping` | 1,642 |
| 6 | `capture`, `present`, `inspect`, `player`, `join`, `lib` | 2,140 |

Session 1 read 707 and ran short; session 2 read 2,112 and was full, so 3 and 4 are at the ceiling
rather than under it. Groups 5 and 6 are the persistence and presentation halves, which is the same
seam `docs/design.md` §9 and §10 already use.

**If phase 5 is skipped**, phase 6 runs against `Requirements.md` as it stands, which is already
normative. The scans depend on phase 4, not on phase 5.

### Phase 6, session 1 — `device` and `frame`

Nine findings, **unrouted**. Ground rule 5 wants each a destination, and that is the author's call;
nothing in the code or in `Roadmap.md` was edited. The proposal is at the end.

**Wrongly built.**

1. **`ActionMapPlugin` panics on its first update in the no-devices build.** `lib.rs:255` gates
   `InputFramePlugin` on `any(keyboard, mouse, gamepad)`, and that plugin is the only caller of
   `init_resource::<InputFrame>()`; `capture.rs:607`'s `run_captures` takes `Res<InputFrame>` and is
   registered unconditionally, as does `evaluate_context`. Verified by running an `App` under
   `--no-default-features --features libm`: `Parameter … failed validation: Resource does not
   exist`. All three single-feature builds pass. **`cargo check` and clippy cannot see this**, which
   is all the Verification section runs for that configuration.
2. **The `touch` feature advertises a source that does not exist.** `lib.rs:174` calls it "Touch
   input as a binding source"; there is no `cfg(feature = "touch")` anywhere in `src/`. design §11
   says *reserved*, so the contradiction is between the two, and the user-facing half is the wrong
   one.
3. **`device.rs`'s module doc claims a persistent identity and capability data**, neither of which
   the module has (R11.5, R11.3). The `DeviceHandle` doc eight lines below says the first is
   not-yet-built, so the module page contradicts the type page.
4. **R14.9's warning reads four of `GamepadSettings`' six fields.** `device.rs:296` misses
   `default_button_settings` and `default_button_axis_settings`, so a game setting a button
   threshold gets the silence R14.9 exists to prevent. Its comment's reason is wrong at the pinned
   commit besides: `ButtonSettings` derives `PartialEq` (`gamepad.rs:821`) and only
   `ButtonAxisSettings` does not (`gamepad.rs:1413`).
5. **R16.1 is unimplemented in any build without `keyboard`.** `KeyboardFocusLost` is behind
   `bevy_input`'s own `keyboard` feature, so a `mouse`-only build samples no focus loss and
   `eval.rs:430`'s `held_mouse_buttons.clear()` compiles out with it: a held mouse button survives
   alt-tab. Not fixable at our layer, so it wants a stated price rather than silence.

**Underbuilt, and unrouted anywhere.**

6. **§11's openness** — R11.1, R11.2, R11.3, R11.8, R11.9, three of them MUST. R11.5, R11.6 and
   R11.7 all have destinations, so what is missing is the *model's openness*, not its identity half.
   `DeviceHandle` is closed, with no `Custom` and no `#[non_exhaustive]`, and its doc argues for
   exhaustive matchability — the opposite of D19's choice for modifiers and conditions. That is a
   decision by the admission test's own standard and `decisions.md` does not carry it.
7. **§13's pointer half** — R13.1 wants position distinguished from motion and the frame carries
   no absolute position at all; R13.4 and R13.6 likewise. R13.3 and R13.5 are routed; these are not.
8. **R9.9's pumped sampling mode** — `sample_input`, `begin_sample` and `record` are all public,
   so the pieces exist. What is missing is a way to stop `InputFramePlugin` scheduling sampling.
9. **R14.10's only claimed satisfaction is a mis-citation.** `frame.rs:338` credits calibration's
   placement with meeting it; R14.10 governs an authority backend, which per D51 enters at the
   button state machine and never touches the frame. The placement is right and the `R`-number is
   wrong — phase 7's lane, recorded here because it is the crate's only claim on R14.10.

**Unasked-for surface came to four items**, which is the honest result for these two modules:
`GamepadCalibration::clear_device` and `is_empty` have no caller and no document behind them, and
`warn_on_unread_gamepad_settings` and `retire_read_events` are `pub` while only `InputFramePlugin`
ever names them. The proportion category did not exist yet and these two modules have not had it.

**Checked and correct**, so a later session need not re-derive it: R9.1–R9.5 and R9.7, including
the two worth doubting — deltas are summed rather than replaced (`eval.rs:347`, asserted at
`eval.rs:1131`), and events are replayed singly rather than folded once, which is what keeps a
press-and-release inside one window from cancelling. R11.4's hot-plug policy holds at `eval.rs:422`;
the queue's append-monotonic invariant survives `clear()`, since `frame` is never reset; D20's
stage-1 no-rescale rule holds, clamp included.

**The proposed routing**, for whenever it is decided: 1, 4 and 5 to the defect register; 2 and 3
fixed in place by whichever chunk takes 1, being three lines and no design question; 6 and 7 two
deferred rows with gates, plus a `decisions.md` entry for the closed `DeviceHandle`; 8 a deferred
row; 9 to chunk 28.

**What the session says about the method.** The estimate holds — one module pair, one session, and
it ran short rather than long. Two of the three categories paid: "wrongly built" yielded five
despite phase 0's reconciliation, because every one of them is in a *configuration* rather than in
the default build, which is what phase 0 read. The underbuilt walk is the one that wants care: most
`NONE` results are requirements that are simply built, so the cheap grep for an unrouted `R`-number
is a starting list and not a finding list.

### Phase 6, session 2 — `action`, `condition`, `event` and `plan`

Eleven findings, on session 1's terms: **unrouted**, nothing in the code or in `Roadmap.md` edited,
proposal at the end.

**Wrongly built.**

1. **A binding whose only conditions are blocking fires at rest.** `combine` (`condition.rs:361`)
   tests actuation in the no-conditions case only; once a binding has any condition the "control is
   off rest" test is gone, and nothing replaces it for a set with no explicit condition that reads
   the value. One blocking condition that is not vetoing leaves `explicit == 0` and `implicit_all`
   vacuously true, so the binding fires every tick with the control at rest. Verified by driving
   `combine` at `ActionValue::Bool(false)`: `Fired`. Unreachable through the built-ins, none of
   which returns `ConditionKind::Blocking` — but the kind is public API through `Condition`, and
   chunk 33's `BlockedBy` is the first built-in that would land on it.
2. **design §4 says a variant rebuilds only the scratch.** `Plan::compile` rebuilds
   `indexed_controls` and `has_chords` as well, and `plan.rs:716`'s comment says the first is
   required rather than incidental: an override rewrites which controls a binding reads, so one has
   to move between indexed and not along with everything else. The code is right and the
   sentence is a clause short.
3. **design §3's trait sketch names a constant that does not exist** — `// plus CATEGORY and
   CONSUME, with defaults`, where the constant is `CONSUMES`. Copying the sketch into a hand-written
   impl does not compile.
4. **The advice for writing an `InputContext` by hand leaves out the half that matters.**
   `action.rs:497` says to implement the trait yourself "if you need to configure the component
   differently; it is three associated constants" — but the trait is not what makes the type a
   component. The derive emits `Component`, `Default`, `Clone` and `Copy` alongside it and a
   hand-written impl gets none of them. `macros/src/lib.rs:131` says "four associated consts" for
   the same trait, so the two disagree about the count as well; four exist and three are required.

**Unasked-for surface.**

5. **`Plan` is `pub` with nothing public on it** — no field, no method, no constructor, and it
   appears in no public signature, every wrapper holding one being `pub(crate)`. It is on docs.rs as
   a struct a reader can name and do nothing with. design §4 names `Plan<C>` in prose, which is
   architecture rather than a request for it to be public.
6. **Four public names for two conversions.** `ActionValue::into_output` and `from_output` are
   one-line forwards to `ActionOutput::from_action_value` and `into_action_value`. `from_output` has
   no caller in `src/`, `examples/` or the tests and duplicates the four `From` impls twenty lines
   above it; `into_output` is called only by its own test.
7. **`Intent::supports_output`** is a public wrapper over `is_one_of`, which is the one the derive
   calls. Nothing else calls either.
8. **`ActionState::new`** is a `const fn` constructor for a two-field struct with both fields public
   and a `Default` impl. No caller.

**Overbuilt**, which is the category session 1 did not have.

9. **The action registry holds one fact three ways.** `next_id` is always `entries.len()`; each
   entry's stored `ActionId` is always its own index; and `ActionId::info` linear-scans the vector
   that index would subscript. The table is written once per process and holds tens of rows, so none
   of it costs anything at run time. What it costs is three invariants a reader has to confirm are
   still true.
10. **`Plan<C>`'s type parameter is phantom.** No field and no method reads `C`. What it buys is
    that handing context A's plan to context B's state does not compile; what it costs is `compile`,
    130 lines of it, monomorphized once per context type — and the three wrappers that hold a `Plan`
    carry `C` themselves already. Whether the guard is worth the copies is a judgement, which is why
    it is here rather than fixed.

**Underbuilt, and unrouted anywhere.**

11. **R3.4 and R3.5 have no implementation and no destination.** R3.4 is a MUST — elapsed time in
    the current state, in the same simulated seconds the action's own conditions count with — and
    R3.5 is its SHOULD, progress toward firing for a hold-to-confirm meter. `ActionState` is
    `{ value, phase }` and nothing public exposes either number. Both exist: `Scratch::time` is the
    elapsed time and `BindingCondition::Hold`'s `duration` is R3.5's denominator, but the scratch is
    `pub(crate)` inside `InputContextState` with no read path out. Neither appears in the defect
    register, the deferred table, or any `Still open` remainder.

**Checked and correct**, so a later session need not re-derive it: R2.2's conversion table matches
`ActionValue::to_bool`/`to_axis1`/`to_axis2`/`to_axis3` cell for cell, including the two rows the
requirement expects to be argued about — narrowing to 1D measures the whole value and therefore
loses the sign, and the bool conversion tests against rest rather than against a press threshold.
R2.10's two hardware cases hold in `Intent::accepts`: a trigger on a `Button` channel serves
`Analog1`, and `Directional2` accepts nothing but `Axis2`, which is what forces a D-pad through the
same composite as four keys. R1.1's declared path is required by the derive with no default. R6.7
holds by construction — a condition's only clock is the `delta` it is handed. R24.4's
app-build/runtime split is honoured at both panics, `plan.rs:681` and `action.rs:673`. design §4's
fourteen `DiagnosticKind` variants and both `Severity` variants match the code exactly.

**One thing the Verification section does not run.** `cargo doc --no-deps --all-features` warns —
`device.rs:8`, a redundant explicit link target. Doc comments are the crate's public documentation
and this is the only command that reads them, so it belongs in `CLAUDE.md`'s list.

**The proposed routing.** 1 to the defect register, owned by chunk 33, which is where a built-in
blocking condition first arrives. 2, 3 and 4 fixed in place, being three document lines and one doc
comment; 4 is chunk 28's lane as well. 5 to 10 want a chunk of their own that phase 6 accumulates
into, since sessions 3 onward will add to it and one pass over the whole public surface is cheaper
than four. 11 wants a chunk: a MUST with no destination is chunk 35's situation exactly, and the
mechanism is as nearly built as 35's is.

**What the session says about the method.** The estimate does not hold and the split above is the
correction. On the categories: nine of the eleven came from the three session 1 already had, and the
two the addendum added are a judgement call and a tidy-up rather than defects — worth keeping, but
not where the yield is. The underbuilt walk paid this time where it barely did in session 1, and for
a reason worth carrying forward: `Requirements.md` §3 and §6 are about *state over time*, and the
requirements a code reading silently satisfies are the ones about shape.

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
