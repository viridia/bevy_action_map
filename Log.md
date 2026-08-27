# Work log: `bevy_action_map`

> What has been built, in the order it was built, and what building it taught us.
>
> This document exists so the other two do not have to carry it. [Requirements.md](./Requirements.md)
> says what must be true and why; [Roadmap.md](./Roadmap.md) says what is left to do. Neither needs
> to explain how we arrived at them, and a reviewer coming to this project cold should be able to
> read those two and skip this one entirely.
>
> Read this when you want to know **why a requirement says what it says** beyond the rationale given
> in place, or when a decision looks arbitrary and you suspect there was a reason.
>
> **[Log-archive.md](./Log-archive.md) holds the closed entries** — everything whose obligations are
> stated elsewhere and which nothing left in the sequence reasons from. What stays here is what the
> work still in flight is built on, so this file is short by construction rather than by age.

---

## The shape of the record

Two kinds of entry appear below, and the distinction is the useful one:

- **A chunk landed and taught us something.** The lesson usually became an amendment to the
  requirements or a new item on the roadmap, and the entry says which.
- **A chunk landed and claimed more than it delivered.** These are recorded at least as carefully,
  because the pattern in them is the most useful thing this document contains.

---

## Phase VII — the player-facing model

### The persistence design, before any of it was built

Chunk 23 was a paragraph and a vehicle. Talking it through produced enough to design it (Design
§10.1) and four requirements that were missing, which is a better return than discovering them
while writing a loader.

**The starting proposal was one row per action path, holding an encoded binding.** Three things
break that, and each pushes the row in the same direction:

- an action has several bindings, so `Jump` is Space *and* South;
- the unit of rebinding is the mapping rather than the action, which D7 settled and which a
  composite makes unavoidable — the player rebinds "move forward", never `Move`;
- only the *source* belongs to the player at all. Modifiers, conditions and chord structure are
  developer data, and the knobs a player does get are tunables.

So a row is keyed by mapping and holds a control, not a binding. Everything stays a scalar, which is
what keeps the file legible in TOML or anything else.

**The finding that would have cost the most later: absent and cleared are different.** Overrides are
a diff against defaults, so a missing row already means "use the default" — which leaves a player who
deliberately unbinds something with nothing to say. It needs its own value. And an action an external
backend owns is a third state again, neither defaulted nor cleared, or a Steam-bound action reads as
one the player wiped. That is now R17.7, and it is a table with three rows because a format with two
loses one of them silently.

**Two players with identical controllers is a persistence question in disguise.** If a stored
binding could name a device instance, the file would say "player 1 uses the pad with GUID abc123"
and break when a controller is replaced. It cannot, so bindings name a device *class*, and pairing
and calibration are separate stores keyed by persistent identity — R17.8. Two players with identical
pads and identical mappings then share one table and differ only in pairing, which is chunk 26's
business and correctly invisible here.

**Steam's IGA file is not a binding file**, which is the answer to whether it should inspire the
format. It declares action sets, layers, and actions with their types; the bindings live per-user in
Steam's own storage. So it is the counterpart of our action *declarations*, and we already match it
— their action types are our `Intent`, existing for the same reason, and their localization block is
R19.14. Two things did come out of reading it that way: an authority backend rewriting bindings
mid-session is normal rather than exotic, which is why applying to a live context is the only path
in rather than a reload feature bolted onto a startup path; and a Steam action belongs to exactly
one action set while ours may be bound in many, which is R19.15's colliding mapping keys arriving
from another direction.

The IGA details here are from the documented format rather than from a file in hand. If the backend
work leans on them, they want the treatment §14's gamepad claims got.

### The Steam grooming, before chunks 40 and 38

No code. D3 was the one structural commitment in §0 that had never been checked against the thing it
exists for, and the reason to check it now rather than when a backend gets written is that **chunks
40 and 38 build the two surfaces it needs**. R18.8 says reverse lookup is a trait with our binding
table as one implementation; R19.8 says rebinding must be able to answer "not ours, delegate". Both
are decisions those chunks make by default if nobody has looked at the second implementer, and both
are cheap while the type is being written and awkward afterwards. Doing this after them would have
meant discovering it as a refactor, which is the shape §12 already warns about for R8.6.

**Four requirements survived unchanged**, which is the more useful half of the result. R0.4's
per-action authority, R14.10's "the backend's deadzone is the backend's", R18.8's trait, and R19.8's
delegation all describe Steam correctly, and they were written before anyone here had read the SDK.
The layering held: an API that returns action values and owns its own binding UI is exactly the
authority case §0 predicted, and nothing about the four-layer split had to move.

**What did not survive is the assumption that L2 is where a backend takes over.** Steam presents a
pad it is driving as an emulated gamepad, so the platform enumerates it and `sample_input` records it
while the backend is also reporting it — every input twice. R0.4 stops us *computing* the action; it
never said anything about sampling the hardware underneath. That is R0.6, and it is not a Steam
workaround: a replay backend needs the same verb for the same reason, and the demo is unusable
without it. Worth noting how it was found — by asking what the first five minutes of running the
thing would look like, not by reading the requirements again.

**Three decisions, each written to be falsifiable by chunk 42** (Design §10.5):

- **A backend writes a value, not a state.** Steam returns a level with no edge and no timestamp, so
  §9's timing cannot be reconstructed from it — but `fired()` has to keep working or R0.5 is false.
  Both hold if the write substitutes for the fold's output *inside* the evaluator, letting our
  existing state machine synthesize the edges. The alternative, a second write path into
  `InputContextState::actions`, would need its own copy of R6.1, and two implementations of the
  button state machine is precisely the drift R0.5 exists to prevent. Falsified if a backend-owned
  action can be given a `.hold()` without a plan-build diagnostic.
- **A context is a layer.** Steam runs one action set per pad plus a stack of layers, we run many
  contexts at once. The mismatch §10.2 flagged turned out narrower than it read, because a context
  that is always active is not a mode — Disasteroids' `Shell` holds `Pause` unconditionally and
  belongs in the base set, leaving only the state-gated `Flying` as a layer. What genuinely does not
  translate is consumption: a layer shadows or it does not, and there is no equivalent of one
  context claiming a control for a frame. Recorded rather than solved. §10.2 asked for this to be
  settled once rather than twice and it now is, in the same place as R19.15.
- **Suppression is L0.** Above.

**The dependency question answered itself.** `steamworks` is `std`-only, `unsafe` FFI beneath, and
wants the redistributable at link time; this crate is `no_std`, `forbid(unsafe_code)`, and minimal
by manifest comment. So the real backend is someone else's crate and nothing under `src/` may name
Steam — which is a constraint on the seam, not a packaging note, and the thing that tests it is a
mock living entirely in `examples/`. That is chunk 42, and its acceptance criterion is a non-diff:
Disasteroids' pad becomes backend-owned and every file except `actions.rs` is untouched.
`actions.rs:96` already says "the pad is left alone: console and Steam remapping already own that",
which was written as policy and becomes true.

**One correction.** R18.9 said backend glyphs arrive as opaque handles or raw image bytes. Steam
returns a filesystem path, which is neither and which the app has to load. The requirement's
substance was right and only its enumeration of shapes was short — but the same gap one level up is
load-bearing: a backend's *origins* are its own enumeration of physical controls, covering device
families we have no `Control` for, so a reverse lookup returning `Vec<Control>` quietly makes the
trait ours-only. That is now chunk 40's second review surface.

### Housekeeping, before chunk 38: the archive gets a rule

This file had reached 941 lines and 70KB, larger than the archive it feeds, and every session was
paying for it during the bootstrap. Eleven entries moved to
[Log-archive.md](./Log-archive.md) — chunks 18, 19, 37, 20, 39, 41, 43, 21, 40, 29/30, and the
housekeeping entry between 47 and 44 — leaving 324 lines.

**The admission rule was the problem, not the length.** The archive said it held "phases I–VI,
closed", which is a rule about *age*, and an age rule cannot shrink this file while a phase is open:
Phase VII still has five chunks in it, so a phase boundary would have archived nothing at all. The
rule it was actually applying — its own header says so one line further down — is **closed and no
longer consulted**, which is about what is still load-bearing. Stated properly: an entry moves when
every obligation it created is written somewhere else *and* nothing left in the sequence reasons
from the entry itself. That archives a chunk the week it finishes and keeps a grooming note for a
year, which is the right shape.

What stayed is what chunks 38, 54, 31, 23 and 42 are built on: the persistence design, the Steam
grooming, chunk 47, chunk 44, chunk 50 and chunk 53.

**The bootstrap described both files by phase**, so `CLAUDE.md`'s document table went stale the
moment the rule was applied honestly. It now describes them by what they carry. Worth noting because
that table is the only part of these documents a session is guaranteed to read, and a wrong
description there is a wrong description everywhere.

Nothing was deleted. The archive is one `grep` away, which is the entire reason the split exists.

### Housekeeping: the rule applied a second time

Ten entries moved — chunks 47, 44, 50, 53, 38, 54, 25, 56, 57 and 58 — checked one at a time against
every `###` section still open in [Roadmap.md](./Roadmap.md) rather than against age or phase.
Each of the ten turned out to be cited only in passing by what is still pending (a bare "chunk N did
X", already true from the landed-chunks table alone) or not cited at all; nowhere did a pending
chunk's own reasoning depend on going and reading the entry. What stayed did so for the reason the
rule names: **the persistence design** and **the Steam grooming** are pre-work for chunks 23 and 42,
both still unbuilt, and a chunk not yet written is exactly the case "nothing left in the sequence
reasons from it" is not yet true for.

**Chunk 31 stays for now, deliberately not swept in with the ten.** It landed this session, chunk 45
(Presets) is next in the sequence and touches the same screen, and there has not yet been a session
that needed to consult it and found it already gone. Revisit next time this rule runs.

### Chunk 31: The settings screen, rebinding

Pressing a boxed cell now captures a control into it. R19.3 leaves the conflict policy to the app;
Disasteroids steals — the pressed control moves to the row just captured, and whatever else held it
loses it, with no extra prompt. Confirm calls `apply_overrides` on the working copy and leaves;
Cancel drops it and leaves; B cancels a capture in progress before it falls through to Cancel's
meaning, which is the two-stage rule R19.2's exclusion list was already half of — `Back`'s own two
controls (`Escape`, `GamepadButton::East`) are excluded from every capture, so pressing either
reaches the action instead of being bound.

**The first design landed on despawn-and-rebuild the whole table on every capture, and it was wrong
twice before it was right.** The first pass rebuilt the entire settings root each time, tracked by a
marker component; reviewed before being written, and rejected for churn no capture actually needs.
The second pass narrowed that to a `bevy_reactor`-style trick — a `bundle_template` that despawns an
entity's children in the same frame new ones are spawned, reapplied to a container holding just the
two tables — also reviewed before being written, and rejected once it was clear a capture never
changes a row's *shape*: column count, which cells are boxed, and the follower lines under them are
all declaration-level facts capacity and rebindability, never touched by an override. What actually
changes is what a handful of cells *say*. The landed version tags every boxed cell with
`RebindCell(Scheme, MappingKey, usize)` and every follower's cell with `FollowerCell(Scheme,
MappingKey, usize, ConditionDescriptor)` at spawn time, and a capture patches `Text` on exactly the
cells named by the rows it touched — the row captured into, and every row a steal emptied — found by
a plain query, no entity despawned or respawned anywhere. Two full-table scans per capture rather
than tracking which columns actually moved: a steal's `retain` can shift every later slot of the row
it took from, so "repaint the whole row" is the right answer as often as the precise one would be,
for far less code.

**`MappingKey` alone does not name a row, and the first version of the two tags above forgot it —
found by playing it, not by review.** `Hyperspace`'s keyboard cell refused every capture; logging
`start_capture`'s lookup showed it resolving to `Hyperspace`'s *gamepad* mapping instead —
`Rebinding::Fixed`, wrong capacity, refused on the first check. `MappingKey` is derived from the
action's path and part alone (§19.R19.9's own doc says so), so the same key names two different rows
whenever an action is bound in both schemes, which every action in Disasteroids is. `mappings(world)`
returns both, in whatever order the context happened to declare them, and `.find(|row| row.key ==
key)` took whichever came first — the keyboard row for some actions, the gamepad row for others,
depending on which scheme's binding was written first in `actions.rs`. Every lookup this chunk added
now carries `Scheme` alongside the key and matches both. `conflicts_pending` itself needed no such
fix: the `control` a capture is choosing among is itself scheme-specific — a key can never sit in a
gamepad row's `slots` — so its scheme-blind key comparison was never actually ambiguous, only this
file's own row lookups were.

**A click did not survive itself: `bevy_input_focus`'s pointer-click handler cleared focus instead of
granting it — also found by playing it.** This screen drives its selection through
`AutoDirectionalNavigation`, not `bevy_input_focus`'s tab-index scheme, and the crate's own click
handler resolves a click through whatever carries `TabIndex` before falling back to "clicked outside
everything, clear focus" — with no `TabIndex` anywhere on this screen, every click read as outside.
The visible effect was exactly what it sounds like: click a cell, watch the focus ring vanish, and
directional navigation has nowhere left to resume from. Considered and declined: adding `TabIndex` to
`focusable()` to ride the existing bridge — it works, but it borrows tab-navigation's own vocabulary
for a screen that does not use tab navigation and never will. Landed instead as a fourth thing every
`focusable()` widget carries: `claim_focus`, an observer on the same `Activate` a click, `Enter` and
pad-A already trigger, setting `InputFocus` to the entity itself. Idempotent for the keyboard and pad
cases (they already hold focus by the time `Activate` fires) and the fix for the mouse case, which
never did.

**`bsn!`'s tuple-constructor form needs `FromTemplate`, which needs `Default` — plain data tags
generally don't have one.** `RebindCell(key, slot)` written directly in a `bsn!` block failed to
compile: bsn!'s `Type(args)` syntax builds through a `Template`, and `FromTemplate` is only blanket-
implemented for `Clone + Default + Unpin` types, which a `MappingKey`-keyed tag has no honest
default for. `template_value(RebindCell(key, slot))` is the documented way around it — `Template`
itself is blanket-implemented for `Clone + Unpin` alone, which every `Copy` component gets for free,
so handing over an already-built value sidesteps the missing `Default` entirely. The same fix
applied to `PromptScheme(Scheme::Gamepad)`, used here for the first time outside `examples/common/`.

**Two more findings from playing it, both small.** A capture in progress looked identical to one
that had not started — nothing on screen said "listening" — so `LISTENING`, an amber
`BackgroundColor`, is set on the cell in `start_capture` and cleared in both ways a capture ends
(`captured`, and `back`'s mid-capture branch). And the help text never said how to *start* a
capture at all; it now reads "press one, then press what you want bound there" instead of stopping
at "the ones this game offers for rebinding". Raised and declined: a gamepad row reading "West
Button" rather than a controller's own printed label (X, on the pad tested against) — this is
`fallback_label`'s own generic-by-design behavior (R19.13's ships-with-no-catalogue fallback), which
a real game replaces with glyphs or a per-platform localization pass; Disasteroids has neither, and
inventing one for this screen alone would be answering a question the crate already has a chunk for
(glyphs, deferred table).

**Not done: telling the player why a capture refused something.** `Refused` (wrong shape, wrong
scheme, reserved) has no observer on this screen; the session simply keeps listening, silently,
which is `CaptureSession`'s own documented behavior for anything not a deliberate press but is a
real gap for the three reasons that are. No mechanism in this file renders a transient message of
any kind, so this is left as a found-not-fixed rather than invented on the spot.