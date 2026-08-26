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

### Chunk 47: a binding as a text span

The presentation half of R18, made authorable. A component beside `TextSpan` names an action and
fills in its own string, so a template says "Press ⟨whatever fires this⟩" with nothing in it naming
a control. Disasteroids' two hand-formatted captions are now spans, and the `format!` and the `join`
are gone from both.

**The dependency graph decided where this lives, and that was not the plan.** The chunk was written
as two components — one requiring `TextSpan`, one requiring `Text` — until the question "what does
that cost" got asked. `TextSpan` is `bevy_text`, which drags `bevy_asset`, `bevy_image` and
`bevy_log`, the crate this project's own manifest says it avoided for being std-only. `Text` is
`bevy_ui`, which drags the render stack — and `bevy_ui` already depends on `bevy_input` and
`bevy_input_focus`. An input crate depending on it inverts the layering and forecloses `bevy_ui`
ever using action maps itself, which is a worse outcome than any amount of convenience is worth. So
the split is by dependency weight rather than by subject: the crate keeps the lookup and the
staleness signal, both free, and everything that draws is `examples/common/prompt_ui.rs` — shared
by `#[path]`, covered by `tests/prompt_ui.rs` so it is tested rather than merely compiled, and
waiting on a deferred row whose gate is Bevy taking the crate upstream. The two-component design
went with it: a whole `Text` that is nothing but a prompt is a `Text` with one span under it, which
is how Bevy already models it.

**A resource that is touched, not an event, and the reasoning is worth keeping.** R18.5 wants
invalidation that is not a per-frame poll. A broadcast event was the first proposal and the counter
won on two grounds: three changes in one frame — a rebind, a context switching off, the entity
carrying it despawning — coalesce into one pass rather than three sweeps over every prompt on
screen, and the pass happens at a point in the schedule the reader picks, which matters because a
caption wants rewriting before layout measures it. The half that made it uncontroversial is that
resources are now entities: the touch is written as an *insert* rather than a mutable deref, so
hooks fire, so a consumer can observe the resource instead of owning a system. There is a test
pinning that, because it is a claim about Bevy rather than about us.

**Component change detection is not an alternative, and it is worth writing down why.** The obvious
implementation watches `Changed<InputContextState<C>>`. Evaluation writes that component every
frame, so the filter is true constantly and detects nothing. What works is a signal raised
deliberately, which happens to travel on the change-tick mechanism.

**The third firing point is the one nobody would have written.** Two were obvious: a context
activating, and a binding changing — that one is chunk 38's to raise, since nothing can rebind yet.
The third is an instance of a context arriving or leaving, because the answer is folded over every
entity carrying the context, so it moves from empty to non-empty with nothing calling `activate`.
That is a hook on `InputContextState<C>` rather than on `C`, registered where `read_bindings`
already is. It is also chunk 40's `PostStartup` ordering, arriving as a fix rather than a
constraint: the corner hint went back to `Startup`, because a span asks after the fact and asks
again when the answer moves, so which system ran first stopped mattering.

**Absence as a third state, for the device a bare prompt speaks for.** The chunk had to decide what
a span with no companions renders, and chunk 40 had already refused to rank devices. The framing
that settled it is that nearly every game has one answer for its whole runtime — a console title
names pad buttons even with a keyboard plugged in — so this is configuration rather than state, set
once and usually never touched. `PromptDevice` therefore has no default: **absent** means the game
has not said and is diagnosed where prompts are drawn, and **present holding `None`** means the game
deliberately has no primary device. A default would have been a guess that fails silently, with
every prompt in the game naming the wrong control and nothing reporting it.

**A rename, and the sweep it exposed.** `Scope` and `Origin` became `PromptScope` and
`ControlOrigin` — a prelude glob-imported into a BSN template puts a bare noun beside components
from three other crates, and `Scope` in particular means something different in half of Bevy. The
sweep found a dozen more (`Scheme`, `Mapping`, `Part`, `Phase`, `Intent`, the four transition
events), which became chunk 48 rather than riding along in a feature chunk. That chunk has a shelf
life: it costs a rename today and a deprecation story the moment anything outside this repository
depends on the crate.

**What the class narrowing cost, and what it bought.** `PromptScope` grew `of(ControlClass)`, which
meant `ControlOrigin::Foreign` growing a `class` beside the `scheme` it already carried — symmetric,
and the alternative was a narrowing no backend-supplied control could ever satisfy. An origin whose
reporter said nothing about its class is excluded from a narrowed answer deliberately: handing a
caller who asked for a button something nobody has claimed is one is worse than handing them
nothing.

**Fell short of its own description in one place.** R18.5 lists a keyboard *layout* change among the
things that must invalidate a prompt, and nothing in Bevy reports either the current layout or a
change to it — the same gap §10.3 records for `fallback_label`. The requirement is annotated rather
than quietly satisfied: it is unobservable rather than unbuilt, and it becomes schedulable if winit
surfaces the layout.

### Chunk 44: bindings that travel together

`.follows::<A>()` on a binding: it rides `A`'s mapping, contributes no row of its own, and moves
with that row when the row moves. Disasteroids' three `Afterburner` bindings drop `private` for it,
which was the chunk's stated acceptance test and is the whole of its diff in the example.

**The bug it fixes is a gameplay bug, and it was latent rather than absent.** Rebind Thrust to `J`
with the old model and the afterburner stays on `W` — and if the player later puts Fire on `W`,
holding Fire afterburns. Nothing collides, so conflict detection could never have caught it: the
failure is a *separation* that should not have been possible, and `conflicts()` looks for two rows
holding one control. Chunk 38 was the deadline rather than the discoverer, which is why this landed
first.

**Resolution is by the controls, not by the name, and that is one lookup doing three jobs.** A
follower names an action, and that action usually has several bindings — one per device. Matching
the *source* picks the right one without either side naming a device, checks "the same controls"
rather than trusting them, and guarantees that a follower's controls are its principal's slots,
which is what will let a sub-row inherit the row's columns. It also rules out a chain of followers
for free: a follower has no mapping, so nothing can ride one.

**The roadmap's own check would have failed the roadmap's own acceptance test.** It said the target
must "exist, be `mappable`, be in the same scheme, and read the same controls" — but Disasteroids'
pad `Afterburner` follows the pad `Thrust`, which is deliberately listed-and-fixed, because the pad
table is read-only and console remapping owns it. Requiring the target to be rebindable rejects it.
The check is *listed*, not mappable: following a fixed row leaves nothing to rewrite and still keeps
the duplicate off the screen, which is worth having alone. Worth recording because the plan and the
example disagreed and only the example was right.

**Two refusals rather than one, because the fix is in different places.** `FollowsNothing` is no
binding of that action reading this — a typo, or a device bound on one side only. `FollowsUnlisted`
is a binding that reads it and is `private`, so there is no mapping to lend; the repair is on the
*other* binding, and one diagnostic covering both would have named the wrong one.

**What the chunk deliberately did not do, and why the deferred table was wrong about it.** A
follower is not listed separately — so nothing about this puts `Afterburner` back on a screen, which
is what the deferred row for R18.3's condition half predicted would happen and used as its gate. Two
things in that row were untrue. `private` was never what concealed the held-versus-tapped prompt
collision: `a_private_binding_still_answers_a_prompt` has asserted since chunk 40 that a hidden
binding answers prompts, so the collision has been reachable all along and what conceals it is that
nothing in tree *prompts* `Afterburner`. And a gate that trips on a chunk which does not trip it is
not a gate.

So the row is deleted and the work is **chunk 50**, with a number rather than a gate. Asking what
the player wants to see is what settled it: a player looking up "how do I boost?" cannot find out
from the controls screen today, and will not be able to after this chunk either. The answer is one
row, one set of keys, and a subordinate line beneath it — dimmer, not activatable, carrying its own
whole formula ("Hold W") rather than a diff against the row above, because a bare "hold" in a cell
is a qualifier with no control in it. That needs R18.3's condition half, which also has a second
consumer in `PromptSpan`, which is what makes it a chunk instead of a corner of this one.

### Chunk 50: what a held control says

`ConditionDescriptor` in `condition.rs`: `None`, `Hold { duration }`, `MultiTap { count }`, derived
from a binding's own `Vec<BindingCondition>` by `describe` — first match wins, and `HoldAndRelease`
reads as `Hold` because the player still has to hold the control even though release is what fires.
Its `fallback_format` is the whole formula ("Hold W", "W ×2") rather than a qualifier alone, on the
same reasoning `fallback_label` already carries: a catalogue gets the pieces separately and composes
its own word order, and only a game with no catalogue gets English glued together.

Two consumers, as planned. `Prompt` gained a `condition` field, populated in `read_bindings`
alongside the chord it already carried; `examples/common/prompt_ui.rs`'s `caption` runs the chord
through first and the condition second, so a held chord renders "Hold Ctrl+S" rather than dropping
one or the other. `Mapping` gained `followers: Vec<Follower>` — action, path, and the follower's own
condition — built in a second pass over `InputContextBuilder::mappings` because a follower's row is
found by its *leader's* declaration, which wants the whole binding list resolved rather than whatever
`mappings` has accumulated so far. `examples/disasteroids/settings.rs` draws each follower as a
`line` indented under its principal, sharing `cells`' shape but never `changeable`, so the selection
cannot land on it.

**The bug worth recording: a follower rides a binding, and a row can be several bindings.**
Disasteroids' `Thrust` is one row with two keyboard slots — `KeyW` and `ArrowUp`, two separate
`mappable` bindings merged by key and scheme — and `Afterburner` declares `.follows::<Thrust>()`
once per key, because `leader_of` matches by exact source and a follower can only ride one binding
at a time. Both follows-declarations resolve to leader bindings that feed the *same* mapping row, so
the naive second pass pushed `Afterburner` onto that row's `followers` twice — one identical
subordinate line drawn under the other. The fix is a dedup by the follower's action within one row's
`followers` before pushing, which is also the right answer for the case that looks similar and
isn't: two *different* followers, each riding a different slot of one row, are two real facts about
it and both belong. `a_follower_riding_every_slot_of_a_row_is_still_one_sub_row` pins the one that
looked like a corner case and turned out to be the acceptance test's actual shape.

**Running Disasteroids to check the acceptance test found a second, unrelated thing.** The settings
screen is now visibly taller than its two device tables account for, because `Menu` binds
`Navigate`, `Accept` and `Back` with nothing marking them as machinery rather than controls, and
listing-by-default puts all of them on the screen — exactly what chunk 53 already describes and was
written to fix. Nothing about this chunk caused it; the extra row height chunk 50 adds is what made
a pre-existing problem large enough to see. No new destination needed — 53 already is one — so
"Known wrong today" now names it instead of leaving it for the next reader to rediscover.

### Chunk 53: a context the player never sees, closed without a crate change

**The chunk as written assumed a fact that turned out to be false.** It called for a builder-level
declaration — `private` or some other spelling, on the whole context rather than on each binding —
because the alternative was said to be filtering by context at every screen, which every future
screen would then have to know how to do. But `Mapping` already carries `context: &'static str`,
set from the declaring context's path since chunk 19 built it for the mapping-collision check
(`report_mapping_collisions`). The data chunk 53 was written to add already existed; nobody had
looked before designing around its absence.

**Fixed at the one call site that has it, not in the crate.** `settings.rs`'s `screen` function
already names `Menu`, `Navigate`, `Accept` and `Back` concretely — it is the function building this
game's own screen, not the generic table renderer beneath it — so filtering `mapping.context !=
Menu::PATH` there costs one line and touches nothing `table`, `cells`, or `follower_cells` read. The
claim in this file's module doc, that nothing below `screen` names a context, stays true; the filter
sits above it.

**Why not build the crate feature anyway, for the games that will hit this later.** The case for it
was that a screen written once should work in a different game without knowing its context types —
but that was never a stated requirement, only a property chunk 19 inherited by reusing chunk 36's
type-erased registry (built for R22.2's debug overlay) for the mapping list too, and later prose
treated the inheritance as a design goal. With exactly one consumer of `mappings` in the tree today,
declaring the exclusion once in the crate and filtering it once at the call site cost the same
number of lines; the crate version added a builder method, a name to bikeshed, and a panic-ordering
check against `mappable` for a case that has not happened yet. Revisit if a second screen needs the
same exclusion and duplicates the filter — that is the point at which the crate is the one paying
for the repetition rather than one call site.

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

### Chunk 38: applying a rebind

Twenty chunks of the player-facing model and none of them could change anything. Capture reported a
choice, `mappings` read the compiled defaults, `conflicts` read the compiled defaults, and chunk
44's follower link had nothing to follow *through*. What was missing in all four cases was the same
thing: somewhere to write an answer.

**Split before it was written, into 38 and 54.** The chunk as groomed carried the store, the apply
path, the follower rewrite, `mappings()`'s meaning, the four conflict policies, R19.8's delegation
outcome, the repeat-within-a-row question and the pending working copy. That is two chunks, and the
seam is not arbitrary: a policy needs somewhere to put its answer, so every one of the deferred
items sits *on top of* the store rather than beside it. Worth noting that ground rule 1's "split it
before you write it" is easy to apply to a chunk that reads long and hard to apply to one that reads
coherent — this one read coherent.

**`Overrides` is a plain value, not a resource,** which was the author's call and changed the shape
of the module. The crate defines the structure and applies it; where it lives is the app's business,
so it can be a working copy on a settings screen, a field in a settings resource, or a payload sent
to an account service. That is the same arrangement `bevy_feathers` gives a theme map, and it is
what makes chunk 54's pending set free rather than a second type.

**Three decisions §10.1 left to whoever built it, now in §10.1.**

*Where the variant lives.* "Swapped into an entity's own state" reads as *only* the entity, and that
answer has a bug in it: an instance spawned after a rebind — a player joining, a context respawned
with a game state — silently gets the shipped bindings back. So `AppliedPlan<C>` is a second
per-context resource holding the variant plan and the variant rows, `InputContextPlan<C>` is now
literally untouched rather than untouched by convention, and its presence is exactly the answer to
"has anything been overridden here".

*What `mappings()` means*, which R17.1's note flagged as unanswerable and now is not: it reads the
variant, and `declared_mappings()` reads the defaults. Every existing caller wanted current values
and got them without changing. The one that was easy to miss is the *reverse lookup* — a prompt
names the control that would fire the action, so it reads the variant too, and the test that caught
it was the one about spawning an instance after a rebind rather than anything about prompts.

*A variant keeps the declared plan's slot allocation*, which turned out not to be an optimization.
An action whose every binding the player cleared would otherwise lose its slot and read as
**unbound** — firing the "not bound in this context" warning, which exists to catch a typo and is
exactly wrong for a control somebody deliberately emptied. Keeping the table also means action
states and require-reset flags stay aligned across the swap.

**The `Arc` change, and why the alternative lost.** `BindingModifier::Custom` and
`BindingCondition::Custom` held a `Box`, so neither `BindingSpec` nor `Plan` could be cloned, so a
variant could not be built by the obvious route. The alternative was sharing compiled per-binding
data behind an `Arc` and rewriting only the source — cheaper per apply, no public type change — but
it needs a precomputed key-to-binding index and a second code path deriving the row list. Cloning
the authored specs and recompiling makes applying *literally* the pure function §10.1 already
specified, and reuses `diagnose` on the result. Applying is rare by construction; the recompile is
not the cost worth optimizing.

**One walk, two readers.** The pass that assembles player-facing rows and the pass that rewrites
them both need "which bindings feed this row, in slot order". They were going to be two walks
agreeing by inspection, which is the failure mode worth a function to make impossible — a row built
one way and written another puts the player's control in a slot the screen is not showing it in.
`mapped_parts` is that function, and `mappings()` was refactored onto it rather than the applier
duplicating it.

**What the example found, which nothing else would have.** `examples/capture.rs` stopped at "nothing
is rebound"; it now applies what it captured and prints the row, the rider, and the untouched
declaration. Two things fell out of running it:

- *Growing a slot on one part of a composite is wrong.* An empty slot is filled by copying the
  binding beside it, which is right for a whole binding and produces "Move Down: S | S" for a
  composite — the other three directions land in their own rows a second time. Refused now, with the
  remedy being the second `mappable` composite a two-column movement table is written with anyway.
  Found by writing `mappable_upto(2)` on a directional binding to see what happened, not by reading
  the code.
- *A follower riding only some of a row's slots is drawn as riding all of them.*
  `Mapping::followers` is row-level and carries no slot, so the demo's `WallJump` — following one of
  Jump's two bindings — printed "Hold R, Hold J" when only the first was true. Chunk 44 got this
  right for Disasteroids, where `Afterburner` rides both of Thrust's bindings, so nothing in tree
  had ever shown it. The example follows both now; the modelling question is written onto chunk 31,
  with a plan-build diagnostic as the likely answer since a rider on some slots and not others is
  far more likely a missing line than an intention.

**Deliberately not here, each with a destination.** The conflict policies and the delegation outcome
→ 54. Serde, `Reflect` and the file format → 23, which also inherits the question the `MappingKey`
key raises: a row this build cannot resolve is reported and then dropped, and whether a save should
preserve it is a format decision, not an apply decision. Per-player override sets → 26, which is the
first chunk with two players; note that the variant already lives per instance, so a per-entity
apply is a second entry point rather than a second implementation.

**One new chunk, from noticing that a review note is not a destination.** Chunk 23 carried "open the
file in a text editor and see whether you can tell what it says" as a review surface, which ground
rule 5 does not accept — nobody is accountable for a note. It is now chunk 55, and it has something
to be accountable *with*: a golden TOML document in a test, which fails when the file stops reading
well. Writing 38's store made the reason concrete rather than aesthetic. `Overrides` keys rows by
`(Scheme, MappingKey)`, and a derived `Serialize` over that map emits a tuple key — unreadable in
every format and not a legal TOML table key at all. So the serialized shape has to differ from the
in-memory one deliberately, and "deliberately" is the word that wants a test under it.

### Chunk 54: Conflict policy

Smaller than the roadmap section that described it. `conflicts_pending(mappings, pending, control,
target)` in `capture.rs`: the same question `conflicts` answers, against a working copy of
`Overrides` instead of what is applied, which is what a screen holding unconfirmed rebinds needs
before either choice is committed. `conflicts` itself is now a thin call onto a shared walk with
`pending: None`, so it is unchanged in behavior and untouched in its own tests.

**Everything else the roadmap section asked for turned out to already exist, or not belong here.**
The four named policies — reject, swap, duplicate-allowed, unbind-the-other — do not need a
crate-owned enum or a `rebind()` that mutates rows on the app's behalf. `Overrides::bind`, `set` and
`get` already say everything a policy needs to say: reject is not writing, duplicate-allowed is
writing anyway, and swap and unbind-the-other are the app reading the conflicting row's own current
list — `pending.get(scheme, key)` falling back to `Mapping::slots`, the same rule
`conflicts_pending` itself uses — and writing it back with one control removed or traded. A first
pass built the crate side of that anyway, as a `ConflictPolicy` enum and an `Overrides::rebind`
resolving conflicts and writing several rows internally. Correctly rejected on review as the crate
accreting a decision that is the app's to make, not a gap the app cannot fill itself — and not a
hypothetical concern: this is feedback already heard from collaborators about the crate taking on
more than it needs to. The doc comment on `conflicts_pending` now carries the four policies as
worked examples instead, so a reader is not left to invent the pattern.

R19.8's "not ours, delegate" outcome needed nothing at all: `Override::NotOurs`, landed with the
store in chunk 38, already is that answer, read with `Overrides::get` before a screen ever starts a
capture. Nothing about it required the delegation to be phrased as an outcome a rebind attempt
returns.

**The repeat-within-one-row gap chunk 39 left** (`conflicts` excludes the whole target mapping, so
two slots of one row holding the same control is invisible to it) **resolves the same way, not with
a crate-side check.** A caller about to write a row already has the candidate list in hand — the
same list it is about to hand to `Overrides::bind` — and a duplicate in a `Vec<Control>` needs no
help from this crate to notice. Recorded here rather than turned into an API, on the same reasoning
as above.

**The roadmap's own "review surface"** — whether a policy is a value the app picks once or a
decision it makes per rebind — turned out to be answered by not building a crate-side policy API at
all: the app already owns both cases with the primitives it has, a fixed policy applied every time
or a modal that tries `Reject` first and calls its own resolution afterward. There is no outcome
type here that would have needed to carry what a policy would have done instead.

### Chunk 25: Control classes and class bindings

The binding half of R4.9. Chunk 20 landed the shape half — `ControlClass` as capture's filter
language; this adds a *binding* that targets a class, and a fourth class, `CharacterProducing`,
whose membership depends on the event rather than the control.

**Not `InputAction`.** The first design pass reused `InputAction`/`ActionOutput` so a class binding
could dispatch through the same `Fired<A>` observers everything else does, and immediately hit a
type it had no honest answer for: `ActionValue` has four shapes (`Bool`, `Axis1`, `Axis2`, `Axis3`),
none of which is "which control fired, with its original event." Making `Control` satisfy
`ActionOutput` would have meant an `into_action_value`/`from_action_value` pair that is never
actually called, `unreachable!()`'d out — dead code standing in for a conversion nothing needed. A
class binding never enters the per-tick fold, carries no modifiers or conditions, and has nothing to
hold between ticks, so it got its own trait (`ClassBinding`, one associated `PATH`) and its own
event (`ClassFired<A>`, carrying the original `RawEvent` untouched) instead. Smaller than reusing
`InputAction` would have been, once the reuse stopped being free.

**The payload is the raw event, not a synthesized `Control`.** The motivating consumer is a
text-edit widget, which wants what `bevy_input_focus`-style widgets already work with — the
logical key, the text, the repeat flag — not an identity it would have to reconstruct those from.
`RawEvent` already carries all of it and was already `Clone + Debug + PartialEq`, so it is the
payload verbatim. Different classes want different fields out of it; that is left to the app,
matching to `RawEvent::Keyboard` or reading `RawEvent::control()` for the identity-only case,
rather than this crate inventing one payload shape to fit every consumer.

**The plan's second structure is a membership test, not a re-derivation of arbitration.** Design
§4.1 describes "the per-control index" as something a class binding is consulted behind; in this
evaluator (which folds from held state each tick rather than dispatching per binding per event)
that turned out to mean exactly one thing: `Plan::indexed_controls`, every control any plain binding
in the context reads, recomputed on every compile — including a variant's, since a rebind changes
which controls are indexed even though it never touches the class list itself. A class binding
never wins a control by being more specific; it is excluded outright the moment any plain binding
names that control, which is what "it only ever wins by sitting in a higher-priority context"
(§4.1) already said, just enforced up front rather than re-litigated per tick.

**`character_producing` was measured, not reasoned, using a new example**
(`examples/ime_diagnostic.rs`) **run against real input on the author's machine.** A macOS kana
input source composed correctly: every keystroke arrived as its own `Pressed` `KeyboardInput` with
`text: Some(single kana character)`, and the matching `Released` always carried `text: None`. That
is exactly what the predicate (`text.is_some() && state == Pressed`) assumes, and it held with no
exceptions across the run. A dead key looked wrong at first — Option+I then A, which should compose
to `â`, arrived through this crate's bare diagnostic window as two independent plain letters, `i`
then `a` — but a side-by-side run of Bevy's own text-input example against the same keystroke
produced one composed character (visible as the font's missing-glyph box, not two letters), which
means the composition the diagnostic window was missing is a property of *that window* — most
likely IME not being enabled on it — and not something winit or Bevy fails to deliver in general.
So the predicate needs nothing extra for dead keys: wherever composition happens upstream, it
already lands as one `KeyboardInput` with `text: Some(the composed character)`, which is the exact
shape the predicate already recognizes. Left genuinely unmeasured: committing a multi-candidate
conversion through an IME's candidate popup. Reasoned rather than measured: it should be fine,
since that commit happens through ordinary keystrokes (Space or Enter to pick a candidate) that the
same per-event rule already judges independently — recorded as a comment next to the predicate
rather than a chunk, since nothing about it currently looks wrong.

**Deliberately not here.** Wiring an actual focused text field to `CharacterProducing` → 49, which
already owns "the rest of D4" per Requirements.md's own note by R8.4/R12.6. Nothing in-tree
declares a class binding yet; the mechanism and its tests are the whole of this chunk, same as the
roadmap section said going in.