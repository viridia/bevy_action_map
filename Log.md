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

## Phase VII — the presentation model

### The persistence design, before any of it was built

Chunk 23 was a paragraph and a vehicle. Talking it through produced enough to design it (Design
§10.1) and four requirements that were missing, which is a better return than discovering them
while writing a loader.

**The starting proposal was one row per action path, holding an encoded binding.** Three things
break that, and each pushes the row in the same direction:

- an action has several bindings, so `Jump` is Space _and_ South;
- the unit of rebinding is the mapping rather than the action, which D7 settled and which a
  composite makes unavoidable — the player rebinds "move forward", never `Move`;
- only the _source_ belongs to the player at all. Modifiers, conditions and chord structure are
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
and break when a controller is replaced. It cannot, so bindings name a device _class_, and pairing
and calibration are separate stores keyed by persistent identity — R17.8. Two players with identical
pads and identical mappings then share one table and differ only in pairing, which is chunk 26's
business and correctly invisible here.

**Steam's IGA file is not a binding file**, which is the answer to whether it should inspire the
format. It declares action sets, layers, and actions with their types; the bindings live per-user in
Steam's own storage. So it is the counterpart of our action _declarations_, and we already match it
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
while the backend is also reporting it — every input twice. R0.4 stops us _computing_ the action; it
never said anything about sampling the hardware underneath. That is R0.6, and it is not a Steam
workaround: a replay backend needs the same verb for the same reason, and the demo is unusable
without it. Worth noting how it was found — by asking what the first five minutes of running the
thing would look like, not by reading the requirements again.

**Three decisions, each written to be falsifiable by chunk 42** (Design §10.5):

- **A backend writes a value, not a state.** Steam returns a level with no edge and no timestamp, so
  §9's timing cannot be reconstructed from it — but `fired()` has to keep working or R0.5 is false.
  Both hold if the write substitutes for the fold's output _inside_ the evaluator, letting our
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
load-bearing: a backend's _origins_ are its own enumeration of physical controls, covering device
families we have no `Control` for, so a reverse lookup returning `Vec<Control>` quietly makes the
trait ours-only. That is now chunk 40's second review surface.

### Housekeeping, before chunk 38: the archive gets a rule

This file had reached 941 lines and 70KB, larger than the archive it feeds, and every session was
paying for it during the bootstrap. Eleven entries moved to
[Log-archive.md](./Log-archive.md) — chunks 18, 19, 37, 20, 39, 41, 43, 21, 40, 29/30, and the
housekeeping entry between 47 and 44 — leaving 324 lines.

**The admission rule was the problem, not the length.** The archive said it held "phases I–VI,
closed", which is a rule about _age_, and an age rule cannot shrink this file while a phase is open:
Phase VII still has five chunks in it, so a phase boundary would have archived nothing at all. The
rule it was actually applying — its own header says so one line further down — is **closed and no
longer consulted**, which is about what is still load-bearing. Stated properly: an entry moves when
every obligation it created is written somewhere else _and_ nothing left in the sequence reasons
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

**Chunk 31 stayed here rather than moving with the ten, and chunk 45 (Presets) is why it was kept:**
it reused `repaint_row` directly for a preset's own live redraw, so this was the session that needed
to consult chunk 31 and found it still here rather than already gone. Whether it can move now is a
question for the next housekeeping pass, which checks against every `###` section still open rather
than against one chunk's own use of it.

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
changes a row's _shape_: column count, which cells are boxed, and the follower lines under them are
all declaration-level facts capacity and rebindability, never touched by an override. What actually
changes is what a handful of cells _say_. The landed version tags every boxed cell with
`RebindCell(Scheme, MappingKey, usize)` and every follower's cell with `FollowerCell(Scheme,
MappingKey, usize, ConditionDescriptor)` at spawn time, and a capture patches `Text` on exactly the
cells named by the rows it touched — the row captured into, and every row a steal emptied — found by
a plain query, no entity despawned or respawned anywhere. Two full-table scans per capture rather
than tracking which columns actually moved: a steal's `retain` can shift every later slot of the row
it took from, so "repaint the whole row" is the right answer as often as the precise one would be,
for far less code.

**`MappingKey` alone does not name a row, and the first version of the two tags above forgot it —
found by playing it, not by review.** `Hyperspace`'s keyboard cell refused every capture; logging
`start_capture`'s lookup showed it resolving to `Hyperspace`'s _gamepad_ mapping instead —
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

### Chunk 61: Exclusive contexts

`Menu` consumed only the controls it named — `Navigate`, `Accept`, `Back`, `Confirm` — so `Thrust`
and `Fire` kept answering while the settings screen covered the ship, blind but still flying. R8.2's
opt-in-per-binding model was doing exactly what it says on the tin; the bug was asking it to do a
context-level job. New requirement, R7.8: a context can declare itself `exclusive`, and every
lower-priority context is treated as inactive for as long as it is.

**Steam's own model settled the shape before any code was written.** Its Action Sets are mutually
exclusive — switching replaces the active set outright — while Action Set Layers are additive,
stacking a partial override on top without disturbing what is underneath. Requirements.md already
carried both halves as separate, unbuilt requirements (R7.3 for the layer, R7.7 for the set) without
either ever having been designed. R7.8 is the set half, and once framed that way the mechanism chose
itself: reuse `deactivate`'s existing cancel-in-flight behaviour (R7.4) rather than invent a third
context state, and reuse priority order rather than a named grouping — a context above the exclusive
one's priority is untouched by construction, which is R8.2's own "the screenshot key should still
work" worked example, arrived at for free.

**The mechanism is one `Option<i32>`, not a per-schedule map like `ConsumedControls`.** The instinct
going in was to mirror `ConsumedControls`'s per-schedule bookkeeping, since both describe something
claimed across a frame that spans multiple fixed ticks. But a control's actuation is genuinely
per-tick data — the reason a fixed tick's consumption claim has to be released before the next one —
while a context's _activity_ does not reset between fixed ticks at all; it only changes on an
activate/deactivate edge. So the ceiling only needs a monotonic raise and one reset at the top of the
frame, and it composes stacked exclusive contexts correctly with no extra code: an exclusive context
only raises the ceiling if it is itself still active _after_ being checked against whatever a
higher-priority one already set, so a third exclusive context shadowing a second correctly stops the
second from also shadowing the first.

**Found by the sweep the session was asked to do, not by review of the diff:** two places in
Roadmap.md were already reasoning from the bug this chunk fixes, and both would have gone stale
silently. Chunk 33 listed "a modal that blocks an action while it is open" as one of `BlockedBy`'s
two motivating cases — it is exactly this bug, and the fix landed at the context level rather than
the condition level, so the motivation is gone from under it. And chunk 49 planned to prove itself in
Disasteroids by deleting `Swallowed` and watching a focused button stop double-firing `Fire` — but
`Swallowed`'s only job was stopping `Fire` from firing behind `Space`, and once `Flying` is fully
shadowed while `Menu` is exclusive, `Fire` cannot fire regardless of what `Swallowed` or R8.2a's
dispatch gap does. Chunk 61 deletes `Swallowed` for a reason that has nothing to do with chunk 49,
and leaves chunk 49 with no acceptance vehicle in Disasteroids until it finds a different one.

**`.consume()` throughout `Menu`'s own bindings turned out to be dead weight, for the same reason.**
Every one of them existed to stop `Flying`/`Shell` from also firing on a shared control; once the
whole context is shadowed rather than merely outranked per-control, consuming is provably inert.
Stripped along with the doc comment explaining why they were there.

**Coupling the simulation's pause to `ToggleSettings`'s own observer was rejected after tracing one
more step.** The first version had the action that opens the screen also drive `Game` to `Paused`.
Closing happens four ways — `Back`, Cancel, Confirm, and (once duplicated for the reason below)
`ToggleSettings` again — and only one of them would have handed the pause back, leaving the other
three exits stuck paused with no visible cause. Worse: since `ToggleSettings` was already reachable
regardless of `Game`'s state (`Shell` has no activation condition), a player who paused first and
then opened settings would have had the close path _unpause_ a game that was already paused before
the screen existed. Landed instead as a second, independent `run_if` on `Simulating` — the set
`pause.rs` already exports for exactly this — gated on `Settings::Hidden` rather than `Game`. Nothing
needs to remember what the state was before, because nothing is mutated: `Simulating` already ANDs
whatever conditions are configured on it.

**`Menu` becoming exclusive blocked `Shell` entirely, including the binding that opens `Menu` in the
first place** — found while updating the example, not designed for up front. `ToggleSettings` is
duplicated into `Menu` rather than reached through `Shell`, which is deliberately inconsistent with
`Pause`: `Pause` stays unreachable until the screen closes, and that is accepted rather than fixed,
because pausing from inside an already-modal screen is not a control this game needs and splitting
`Shell` into two priority tiers to preserve it would be more machinery than the problem is worth.

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
(`captured`, and `back`'s mid-capture branch). And the help text never said how to _start_ a
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

### Chunk 45: Presets

R19.12's own text ("a preset applies through the same path a rebind does") turned out to collide
with what chunk 38 actually built, and finding that collision before writing any code is what this
chunk mostly was. `overrides::refusal` refuses any row whose `Rebinding` is not `Here` — right for a
capture, since a `Fixed` row is a design decision the player's own screen must not second-guess, but
wrong for a preset, whose entire reason to exist is moving rows a capture screen never offers (every
gamepad binding in Disasteroids, since none call `.mappable()`). Presets could not move a single pad
button under the code as it stood.

**The fix threads provenance through the existing apply path rather than adding a third `Rebinding`
state.** A new state would have forced every already-correct `Fixed` declaration in every game using
this crate to be revisited for a fact about the binding that had not changed, which is exactly what
R19.13 (presets cost nothing when unused) rules out. Instead `apply_overrides_with_preset(world,
overrides, preset)` threads `preset` down through `rewrite` to `refusal`, which now takes a
`preset_authorized: bool` and exempts only `NotRebindable` for the rows a preset actually names —
every other refusal reason (capacity, scheme, shape, reserved) still applies exactly as it does to a
capture. `apply_overrides` itself keeps its exact signature, passing `None` at its one call site: a
game with no presets pays nothing, not even a new argument to thread through.

**`overrides` has to be the whole working copy, never only the preset's own rows — found by tracing
`rewrite`'s own module doc rather than by writing code first.** `rewrite` starts over from the
pristine declaration on every call ("a diff, not a snapshot," and the
`the_defaults_survive_being_overridden` test proves it), so two sequential apply calls do not
compose: a smaller second call would silently revert every row it does not mention. That is what
settled "starting point, not layer" for what a preset selection _is_ — the other question this chunk
had to answer, left open in the roadmap. A preset's rows are written into the same working copy a
manual capture writes into, indistinguishably, and nothing anywhere persists "which preset is
currently selected" as an identity — a screen that wants to show one recomputes it, by comparing what
is bound against each registered preset's own rows. This is also the only answer compatible with
`rewrite`'s non-composing nature: a "layer" that reapplies later and reconciles against whatever the
player has since changed would need machinery this crate does not have. It keeps chunk 23's eventual
persisted format exactly the `Overrides` shape it already designed — no second field for "which
preset."

**`Preset` (`src/preset.rs`) is a name paired with an `Overrides`, nothing more** — no crate-owned
registry, for the same reason `Overrides` itself has none: a game keeps its own list, the same way
Disasteroids already keeps its own `PendingOverrides`. Building Southpaw's one row needed
`mappings(world)` at runtime rather than a `const`, since a `MappingKey` cannot be constructed outside
this crate — the same reason `start_capture` resolves a row by asking the world rather than holding
one.

**Southpaw moves `Turn`'s gamepad row from the left stick's X axis to the right stick's.** Disasteroids
has no `Move`/`Look` twin-stick pair to literally swap two sticks between, as the roadmap's "Southpaw
is the honest test case" language suggested — checked before building rather than assumed, since the
game turned out not to match the sentence describing it. One `Fixed` axis row moved by a preset
exercises exactly the same mechanism a two-stick swap would (a `GamepadAxis` control is no different
to `rewrite` than a `KeyCode` one), without adding gameplay scope a presets chunk has no business
adding.

**The settings screen's Confirm/Cancel button row turned out to already be the right model for
heterogeneous scene lists, once bsn!'s own grammar was read rather than guessed at.** `Children [
({cancel_button()}), ({confirm_button()}) ]` splices two _different_ `impl Scene` types side by side
because each is its own `({expr})` splice; a `Vec<impl Scene>` spliced as `{tables}` only works
because both entries come from the same `table()` call. Wrapping the gamepad table and its new preset
button row in a `Node`-column child of the outer `Children [...]` list — rather than trying to make a
mixed `Vec` — followed directly from that, once the distinction was clear.

**Rust 2024's default `impl Trait` capture rule needed an explicit `+ use<>` on both new button
functions — found by the compiler, not by review.** `preset_row(presets: &[Preset], ...)` and
`preset_button(preset: &Preset, ...)` build an owned scene from what they're handed and keep no
borrow past return, but the edition's default still ties the returned `impl Scene` to the input
reference's lifetime unless told otherwise, which does not outlive `screen`'s own local `presets`
`Vec`. `table()` never hit this because it takes `&'static str` and an owned `Vec` — this chunk was
the first to pass a non-`'static` reference through a function returning `impl Scene`.

**The controller touching the view directly hid two real bugs behind one another — found by playing
it, not by review.** The first landed version had `preset_pressed` call `repaint_row` and a
`recolor_preset_buttons` helper directly, mirroring `resolve_capture`'s own established style. Two
things were wrong with it, and each one hid the other:

- **Pressing `Default` after `Southpaw` changed nothing.** `preset_pressed` only ever wrote the
  _pressed_ preset's own rows into the pending copy — fine for `Southpaw`, which has one row, fatal
  for `Default`, whose whole content is an empty `Overrides`. Writing zero rows left whatever the
  last preset had written untouched, so `Default` never actually became the working copy's answer;
  it only ever looked like a no-op. The fix reads every row _any_ registered preset might touch and
  resets it first, before writing the pressed one's own rows on top — "starting point, not layer"
  said this already, in the doc comment, without the code actually doing it.
- **The gamepad table's own text never updated at all, first press or later.** `repaint_row` finds a
  cell by its `RebindCell` tag, and `cells()` only ever attached that tag to a _capturable_
  (`Rebinding::Here`) cell — exactly the ones a preset never needs to move, since every gamepad row
  is `Fixed`. `Turn`'s cell had no tag `repaint_row` could find it by, so every preset press silently
  repainted nothing; only closing and reopening the screen, which rebuilds the table straight from
  `mappings(world)`, ever showed the real state. `CellRole` gained a `Fixed(Scheme, MappingKey,
usize)` case distinct from a blank/nonexistent slot, and a new `RowCell` tag is attached to _every_
  principal cell regardless of whether `RebindCell` also is — identity for finding a cell again is
  now separate from permission to capture into it.

**Both were symptoms of the same thing, raised directly:** a controller (an observer, or a command
queue it pushed) was reaching into the view and repainting it inline, rather than only writing the
model and letting something else notice. The fix follows a pattern the crate already had, reviewed
and shipped — `present::PromptGeneration` plus `resource_changed::<PromptGeneration>()`, which
`examples/common/prompt_ui.rs::refresh_prompts` already consumes exactly this way. `PendingOverrides`
needed no new counter: an ordinary `ResMut`/`Mut` write already marks a resource changed, so
`redraw_pending`, gated on `resource_changed::<PendingOverrides>` and run in `PostUpdate` ahead of
`UiSystems::Prepare`, is the only place anything reads the pending copy back out to touch a `Text`,
`BorderColor` or `BackgroundColor`. `resolve_capture` was simplified the same way — it no longer
threads a `touched: Vec<(MappingKey, Vec<Control>)>` out to repaint the rows it changed; every row is
just re-derived and repainted on the next redraw pass, cheap at this table's size for the same reason
`repaint_row`'s own two-full-table-scan design already was. Fixing the second bug also fixed the
first for free: once redraw is a full re-derivation from `(mappings(world), PendingOverrides)`
instead of a list of touched rows, there is no "which cells changed" bookkeeping left to get wrong.

**Preset construction was raw plumbing, raised directly, and worth fixing before landing.** The first
version of `presets()` hand-rolled a `mappings(world)`-then-`Overrides::bind` dance the same way
`start_capture` does for a single row — correct, but exactly the boilerplate `add_context`'s own
builder exists to spare an author. `Preset::build(world, name, |preset| { preset.bind::<Turn>(scheme,
controls); })` is now real crate API (`src/preset.rs`), not example-only code, since any game
building a preset needs the same by-action-type resolution `Turn`'s row needed here. `bind` panics
rather than silently guessing when an action's mapping in a scheme is missing or ambiguous (a
composite has one row per part) — the same tier and the same convention `add_context`'s own
diagnostics already use (§9.5), since this is an author mistake at declaration time, not a player-
facing one.

**Not done: R18.7.** The user asked whether this chunk subsumes the PlayStation confirm/cancel
button-convention region swap. It does not — checked directly, and nothing in the crate or the
example branches on region or button convention anywhere. Untouched, separate future work.

### Chunks 23 and 55: Persistence, and the file that validates it

Landed together on the user's own call: 55 is a golden test for what 23 builds, so reviewing them
apart would mean reading the format twice. `Overrides` now has hand-written `Serialize` and a
`DeserializeSeed`-based loader behind the `serialize` feature; nothing writes the result to an
actual file, which was always the app's decision and stays one — R17.1 through R17.9 are about the
value that round-trips, not about disk I/O.

**The "cleared middle slot" case Design §10.1 describes turned out unreachable, checked before
building rather than assumed.** `Override::Controls(Vec<Control>)` has no way to express a hole —
a row is however many controls are bound, compacted from the front — and nothing in the crate or
Disasteroids' own settings screen ever produces one: a steal's `retain` (chunk 31) already just
compacts the list it took from, which chunk 31's own log entry treats as the accepted behavior, not
a bug. Building a marker for a state the type cannot hold would have been exactly the unrequested
abstraction the house style warns against, so the golden document has no case for it.

**Whether an unresolved mapping name survives a save was chunk 23's own flagged decision, and the
user picked the simpler side.** A row naming an action this build no longer declares could be kept
as a raw string alongside the resolved rows, so a rename that reverts before the next save loses
nothing — chunk 23's roadmap text called this "cheap to make now and expensive later." Declined:
`Overrides` stays keyed only by `MappingKey` exactly as chunk 38 left it, and a save simply omits
what it cannot resolve, same as `apply_overrides` already does. Revisiting this later means adding
a second store to `Overrides`, not patching one; the roadmap's own warning about the cost is now on
record rather than merely implied.

**A `MappingKey` cannot be manufactured from a loaded string, which is why loading is a
`DeserializeSeed` and not a plain `Deserialize` impl.** The type holds a `&'static str`, always
one the game's own `MappingKey::new` produced at startup — an app cannot construct one at all,
`new` is `pub(crate)`. So resolving a saved row means finding the _existing_ key among what
`declared_mappings` already returns, string-matched by `Display`, never building a new one. A name
that matches nothing has no `MappingKey` to put in an `OverrideProblem` (whose `mapping` field
requires one), so it comes back in a separate `UnresolvedMapping` list instead, carrying the raw
text — an unrecognized _control_ name, by contrast, is discovered only after its mapping already
resolved, so that one does fit `OverrideProblem` and got a new `UnknownControl` variant.

**That variant cost `OverrideProblemKind` its `Copy` derive** — a `String` field is incompatible
with it — and every call site collecting `.kind` out from behind a `&OverrideProblem` needed
`.clone()` added. Mechanical, and the compiler found every site.

**The wire shape is a hand-written `Serialize`, not a derive, because `Overrides`' own shape (a
`BTreeMap` keyed by `(Scheme, MappingKey)`) has no honest table-key representation.** Rows are
grouped by scheme into their own table, each keyed by the mapping's `Display` string; a row holding
one control writes as a bare scalar and reads back from either a scalar or a list, so a mapping
with one binding never needs brackets. `Cleared` and `NotOurs` write as the bare words `"cleared"`
and `"external"`, chosen because every real control name carries a `/` (chunk 37's own naming
scheme), so neither word can ever collide with one. `version` is serialized before `bindings`
deliberately: TOML requires a table's scalar fields to precede any nested table header, so the two
calls have to happen in that order rather than through an unordered map — the version field itself
reads but does not yet branch on anything, since only one exists so far.

**The golden literal matched what `toml::to_string` produced on the first run**, which is the
opposite of a coincidence: `BindingsTable`'s manual `serialize_map` walks a `BTreeMap<Scheme, _>`
in `Scheme`'s own declared order rather than sorting scheme names alphabetically, which is what
keeps `keyboard_mouse` ahead of `gamepad` in the file despite "g" sorting first.

### The forgiveness grooming, before chunk 34

Outside feedback that the crate shoulders responsibility that belongs to the app named two
examples: disabling an action (R3.7, chunk 35) and forgiveness windows (R6.5, chunk 34). The two do
not hold up the same way under that question.

**R3.7 stays.** "The app just ignores the event" does not reproduce what disabling buys: R3.6 gives
a bound action consumption priority over lower-priority contexts, so an ignored-but-still-bound
action keeps its control away from anything else bound to it — exactly the gap R3.7 exists to close
without requiring an unbind. Chunk 35's own note already found the mechanism mostly built
(`require_reset`, a `StateFlags` bit); what is missing is the public verb, so the cost of keeping
this is close to zero.

**R6.5 is withdrawn.** Buffering and coyote time, in the form named, turn on a crossing the crate
cannot see: landing, leaving the ground, or whatever app-domain transition "valid" means for a given
action, is state the crate has no view into, so a crate-side condition could not implement the named
pattern either, only a narrower piece of it. That piece — "was this control active in the last N
ms" — is already answered by what §3 ships: R3.2's events give the press/release edges, R3.4 gives
elapsed time in the current state measured in the app's own simulated seconds, and recording a
timestamp off an event the app already receives is composition, not a missing primitive. Chunk 34
keeps R6.4's ordered sequences, a different question — the ordering is between the crate's own
actions, never against app state — and drops the forgiveness half entirely. Requirements.md marks
R6.5 withdrawn in place, same convention as R2.4.

### The first grooming sweep, before chunks 62–64

Prompted by tunables (R19.11) having a deferred-table row that argued against its own gate.
Grepped `src/` rather than trusting the documents' account of themselves, and three more MUSTs
turned up with no destination anywhere — not even a deferred-table row: R16.1–R16.3 (focus loss and
suspend release held controls), R11.4 (a disconnected device releases them the same way), and R13.5
(input frames carry no source window to filter a binding on). Chunks 62 and 63 are the destination
for those; chunk 64 is tunables' own, with R20.2's hold-vs-toggle riding along since R19.11 already
names it as the worked example.

**R11.6 (device brand/class resolution) is flagged, not chunked.** Loosely covered by the glyphs
row's asset-pipeline gate but never named in it — left for the next sweep rather than forced into a
chunk on a guess.

**Not exhaustive.** §9, §22, and §23 were not checked against code this pass. The author asked for
another sweep later rather than one pass covering everything now.

### Chunk 62: Release on focus loss and disconnect

R16.1/R16.2 (window focus loss) and R11.4 (gamepad disconnect) landed as one mechanism: a third
`Fold` variant, `Interrupted`, rather than the blanket `deactivate`/`activate` the roadmap entry
proposed. The author picked surgical over blanket directly: an unrelated device disappearing must
not cancel an action a surviving binding is still holding, and the crate had never had to make that
call before, since `deactivate` and `shadow` are already whole-context by design.

**The blanket approach would have been wrong twice, not just coarser — found by tracing the state
machine before writing anything.** `update_action_state` reports `Completed` for an ordinary release
and `Canceled` only for an interruption (`deactivate`, `shadow`), and the distinction is real:
`Completed<A>` and `Canceled<A>` are separate observer-triggerable events, so an app's hold-to-charge
attack listens for one and its "held too briefly" feedback listens for the other. Clearing the held
state and letting the ordinary per-event fold run to completion — the simplest reading of "surgical"
— produces `Completed`, because nothing about a normal release-shaped fold pass says *this release
was forced*. Alt-tabbing mid-hold would have silently completed the action for free. `Fold::Interrupted`
exists to carry that one bit: it reads exactly like `Level` (same slots wanted, same combine), and
the only place it diverges is the branch where a binding that was firing reads at rest this pass,
which reports `Canceled` instead.

**Scoping fell out of the existing per-event fold design for free, with no per-binding bookkeeping
added.** `apply_level_event` clears only the held-state the trigger names — `held_buttons` and
`held_mouse_buttons` on `RawEvent::FocusLost`, `held_gamepad_buttons`/`held_gamepad_axes` on a
`RawGamepadEvent::Connection(Disconnected)` — and the fold that immediately follows recomputes every
binding's contribution from scratch, the same as it does for a real press or release. A slot with a
surviving binding on an unaffected device simply computes the same value it already had; nothing
extra was needed to leave it alone, and `a_surviving_binding_is_untouched_by_the_others_device_going_away`
is the test that would catch a regression back to blanket cancellation.

**`require_reset` needed no changes here, unlike `activate`.** A context reactivating can find a
control already true because `apply_level_event` keeps running against an inactive context (R7.6),
so held state can go stale *while still being sampled* — that is what `require_reset` exists to gate.
Focus loss is the opposite failure: events stop arriving at all, so a key that never transitions
never produces a new `Pressed` for the fold to see, and clearing the held state is sufficient on its
own to guarantee R16.2 — no press edge is synthesized until a real release-then-press cycle happens.

**Gamepad state is not per-device, so a disconnect clears every pad's readings at once.**
`held_gamepad_buttons`/`held_gamepad_axes` are keyed by `GamepadButton`/`GamepadAxis` alone; the
`gamepad: Entity` field on `RawGamepadButtonChangedEvent` is already dropped elsewhere in this file,
which is chunk 26/27's own gap (device routing), not a new one. Recorded in the "Known wrong today"
row rather than fixed, since building per-device tracking for one chunk's edge case would be scope
this chunk did not ask for.

**`RawEvent::FocusLost` is gated on the `keyboard` feature alone**, because `KeyboardFocusLost` is
the only trigger available — it lives behind `bevy_input`'s own `keyboard` feature, and there is no
upstream `MouseFocusLost` to fall back on. A `mouse`-only, no-`keyboard` build has no way to detect
focus loss at all; a pre-existing gap in what Bevy offers, not one this chunk could close.

**R16.3 (mobile/console suspend) stayed in the deferred table rather than riding along as originally
planned.** The roadmap entry proposed treating it as "the same trigger, not a separate design," but
nothing in this crate's supported platforms actually emits a suspend/resume signal, and R16.3 also
wants a device re-enumeration step this crate has no concept of yet — there was nothing to point the
mechanism at, so it is a gate rather than a line of code.

**A style pass after the first `cargo fmt` cost a second one — found by the author, not by review.**
New doc comments cited R-numbers and a chunk number, which `///` comments on this crate's own house
style must never do, and several ran longer than their sibling comments needed. Fixing that after
formatting reflowed lines that then needed reformatting again; the fix going forward is to check
comment style before the formatting pass, not after.

### Focus-driven dispatch, and R22.7 withdrawn

Chunk 49's blocker — a public `bevy_input_focus::FocusedInput` constructor — landed upstream this
session, which reopened `FocusedInput<KeyboardInput>` as `bevy_ui_widgets::Button`'s whole keyboard
story. A drop-in replacement for `InputDispatchPlugin` was designed and even built: `FocusBridge`, a
lowest-priority context feeding dispatch through the existing class-binding pipeline rather than a
second raw-message read (`class_dispatch` already checks `ConsumedControls` and per-context indexing
at exactly the right point). It was set aside before landing — Roadmap.md's deferred table carries the
design — because building it on spec, with no widget in tree that actually needed it, was exactly what
chunk 49's own history already warned against: chunk 30 worked around the gap with `Swallowed`, and
chunk 61 closed the one collision that motivated it for a reason that had nothing to do with dispatch.

**What replaced it is more explicit than `FocusBridge` and needed no crate change at all.** Instead of
a generic relay answering for *any* `FocusedInput`-consuming widget, Disasteroids now declares
`ButtonFocused` (`examples/common/widget_focus.rs`) — an ordinary context, active only while
`InputFocus` names an entity tagged `WidgetKind::BUTTON`, binding `Enter`, `Space` and the pad's accept
button to one action whose observer triggers `bevy_ui_widgets::Activate` at the focused entity
directly. `WidgetKind` is registered as a required component of `Button`, so nothing that spawns one
has to remember to tag it. This retired `Accept` — the gamepad-only action Disasteroids carried since
the settings screen shipped, with a doc comment explaining that "the keyboard half already works" via
`InputDispatchPlugin` — and let the game disable `InputDispatchPlugin` outright: one context now
answers for every device the same way, rather than splitting the work between a crate default and a
hand-written observer.

**The author's read, confirmed by building it rather than merely arguing it: R22.7's bubbling was
never the requirement.** `FocusedInput` bubbles because focus is the only arbitration
`bevy_input_focus` has of its own — a widget that declines a key lets it fall through to whatever is
listening further up the entity chain, window included, which is how a global shortcut survives a
widget not wanting a key. A mapper with priority and consumption already answers that question,
better: whether something else claims a control is decided by evaluation order before a
focus-activated context ever runs, and `why_not` (R22.1) can name the context that took it.
`ButtonFocused` drives a real widget on every device without dispatching a bubbling event anywhere —
an ordinary `Fired<A>` observer, at a priority that keeps it from being shadowed by `Menu`.
Requirements.md now marks R22.7 withdrawn on this basis, and R22.8 carries a note that it is answered
by composition rather than by a crate feature: `active_if` already takes any run condition, so a
focus-kind check needed nothing new, and the three cases R22.8 names (nothing focused, an ambiguous
match, a despawned focus) all fall out of the same lookup rather than wanting special-case handling —
nothing focused and a despawned focus both make it miss, and a required component keeps one entity
from ever carrying two kinds at once.

**`bevy_picking` was checked and is not part of this story.** A mouse click on a widget goes through
`Pointer<E>` events with their own `PointerTraversal`, structurally similar to `FocusedInput`'s
bubble-to-window but a wholly separate pipeline — triggered by pointer state, not by `InputFocus`, and
untouched by anything here. R22.4's "how pointer actions coexist with picking" stays exactly as open
as it was.

**What is still open.** `WidgetKind` and the `*Focused`-context pattern live in `examples/common/`
only, the same status `prompt_ui.rs` has — proven, not promoted. Whether either belongs in the crate
waits on [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native widget-kind
id and a mapper-driven remote-press mechanism: promoting a shape this crate invented first, ahead of
that conversation, would risk committing to the wrong one. A widget kind with no `*Focused` context of
its own now gets no keyboard or gamepad input at all, `InputDispatchPlugin` being fully retired in
Disasteroids — deliberate, and the same "additive, never a silent default" bet R19.13 already makes
elsewhere, but worth stating since it is a real cost of going explicit rather than a free lunch.

[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
