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
> **Phases I–VI are in [Log-archive.md](./Log-archive.md)** — chunks 1 through 36, closed and no
> longer consulted. This file starts at Phase VII, which is the work in progress.

---

## The shape of the record

Two kinds of entry appear below, and the distinction is the useful one:

- **A chunk landed and taught us something.** The lesson usually became an amendment to the
  requirements or a new item on the roadmap, and the entry says which.
- **A chunk landed and claimed more than it delivered.** These are recorded at least as carefully,
  because the pattern in them is the most useful thing this document contains.

---

## Phase VII — the player-facing model

### Chunk 18: derive completion

Small, as advertised, and it made three things smaller.

**`category` and `consume` are associated constants with defaults**, so every hand-written
`InputAction` impl in the tests kept compiling without knowing they exist. `CATEGORY` is a
localization key on the same terms as the path, and it lives on the action rather than on each
binding because four movement bindings sharing one category is four chances to disagree. `CONSUMES`
is the action-level default that bindings inherit; `without_consuming` is the exception one binding
at a time, which is the direction R8.2 did not have a way to say before.

**The registry grew metadata rather than a second registry.** It already mapped path to `ActionId`;
it now holds an `ActionInfo` beside each, and `ActionId::from_path` is what turns a name read from a
settings file back into something to look up. It answers `None` for a path this build does not
declare, which is exactly the case R17.2 wants reported rather than dropped — a binding saved
against an action since renamed.

**`#[derive(InputContext)]` now emits `Component`, `Default`, `Clone` and `Copy`.** A context that
is not a component is unusable — every entry point in the crate requires it — and the scene work in
Dead Zone had already forced `Default` and `Clone` onto every one by hand. Four derives became one,
across the tree. The `Component` impl is written out rather than delegated, which is two associated
items in this version of Bevy and will break loudly if that changes; the escape hatch is to
implement `InputContext` by hand, which is three constants.

Worth noting what did *not* happen: Design §9.3 said this chunk would register actions in the
**reflect** type registry. That would key them by Rust type path, which is the identity D8 spent a
requirement rejecting. What persistence and a rebinding screen need is a lookup by *declared* path,
which is our own registry, and it is what was built.

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

### Chunk 19: mappings

The chunk existed to answer one question, and the answer was better than the design's own sketch.

**Could a mapping address one part of a composite?**
`BindingSource::Directional2(DirectionalButtons)` compiles the four keys as one thing, and
`for_each_control` visited them in order without naming them — so nothing in the model could say
"the key that moves you forward". The fix was small once stated: parts are a named enum, and
`for_each_part` yields the name beside the control. Up, down, left, right; negative and positive for
a two-button axis; `Whole` for a binding that reads one control. A stick and the mouse report
`Whole` too, which is the right answer rather than a shrug — they are one thing to a player, and
what they get instead of per-part rebinding is a tunable.

**`mappable()` takes no arguments, and both halves of that are decisions Design §9.7 had the other
way.** It sketched `mappable_parts(Scheme::Kbm, ["forward", "back", "left", "right"])`:

- *The part names were the author's.* That is a positional list to keep in step with a struct's
  fields, and a second vocabulary for four things that already have names. The composite knows its
  parts; the key derives as `gameplay.move.up`, and the catalogue is where `up` becomes "Move
  Forward". Supplying "forward" at the binding would be naming the same part twice, in the one place
  no translator will look.
- *The scheme was declared.* But the binding's own controls already say whether it is keyboard or
  gamepad, and declaring it is a third thing that can disagree with what is actually bound. It is
  now inferred, and a binding whose parts span both devices is refused when the context is declared.

The result is that declaring a whole composite mappable is one call with nothing to get wrong, which
is what the chunk's review surface asked for. §9.7 has been rewritten to match, with the reasoning,
rather than left disagreeing with the code.

**The collision R19.15 predicted needed two checks, not one.** Two bindings of one action in one
context is a plan-build diagnostic like any other. The same action mappable in *two* contexts is
not: a plan is compiled without seeing the others. What made it findable was chunk 36's registry of
declared contexts — built for a debug overlay, and now the only thing in the crate that can see two
contexts at once. Both tests pass, and the second would have been unwriteable a chunk ago.

**Mappings ride the type-erased door too.** `rebind::mappings(world)` walks the same registry,
needing only `&World` because mappings come from the plan resource rather than from anything an
entity carries. Dead Zone's overlay grew four lines that list what a player would be shown — the
smallest possible consumer of D7's model, and enough to see that the keys read correctly without a
catalogue.

**Not doing:** tunables and presets. R19.11 and R19.12 are declarations over the same bindings and
neither is needed until something adjusts a value; capture (chunk 20) is what turns this list into a
screen that changes anything.

### Chunk 37: naming a control

Unclaimed work, found by asking what chunk 21's screen would actually print. Nothing in the crate
could turn a control into text — the overlay was printing `Key(KeyW)` through `Debug` — and R18.3,
which asks for a structured descriptor plus a fallback renderer, was claimed by no chunk at all.
§18 sits in "deliberately deferred" gated on asset-pipeline questions, but that gate is about
*glyphs*; the text half is not asset-dependent and was falling through the gap. Fourth
destination-less item found this way, after R3.7, unknown controls and R22.2.

**One name, two jobs.** `key/KeyW` is both what a settings file stores (R17.9) and the key an app's
catalogue answers to (R18.3). That is the same economy `MappingKey` already has, and it means a
rebinding row is two keys and two lookups with nothing in it this crate renders.
`fallback_label` is the readable half for a game with no catalogue.

**The table is written out rather than derived, and the size is the point.** Two hundred-odd lines
of `KeyA => "A"` looks like something a macro over Bevy's variant names should generate, which is
exactly what R17.9 forbids: it would put every saved binding at the mercy of a rename in a crate we
do not control. Written out, an upstream rename is a compile error in an exhaustive match and the
stored string stays put. It also let the labels say what controls *are* rather than what Bevy calls
them — `LeftTrigger` is a bumper and `LeftTrigger2` is the trigger, and a player reading "Left
Trigger" for the shoulder button would be told something false.

**The `--features libm` build caught another one.** The table macro was defined unconditionally and
used only behind device features, so the build with no devices at all warned about it. Third time
that configuration has found something no other build sees.

**What is not fixed, and is now written down where it will be read.** The fallback answers for a US
keyboard, so a physical binding shows an AZERTY player the wrong letter — R12.2 calls that a bug and
the crate cannot fix it alone, because nothing in Bevy reports what a physical key produces on the
current layout outside an event that already happened. Two things can: an app supplying the control
half of its catalogue per layout, and — once capture exists — remembering the logical key observed
at the moment a player bound something, which is right for every binding they chose themselves.
Design §10.3 records both rather than leaving the gap to be rediscovered.

**Also noted while here: 17c shrank again.** R17.5 wants `Reflect` so third-party modifiers
round-trip, but §10.1 stores controls rather than modifier chains, so a custom modifier never
reaches a saved override file. What still needs it is serializing whole binding definitions, which
is deferred — so nothing scheduled depends on 17c any more.

### Chunk 20: interactive capture

The chunk as written covered five things, and reading it against the code found that two of them
could not be built: conflict *policies* and reset-to-default both need somewhere to write an answer,
and the overrides store §10.1 designs belongs to chunk 23. So the chunk split. Capture, the class
vocabulary, exclusions, reserved controls and conflict *detection* landed here; applying a rebind
became chunk 38, sitting where chunk 31 needs it. Detection is a pure query over the mapping list
and was buildable today, which is where the seam naturally was.

**A session is a component, and the first framing of that was wrong.** The proposal said "on the
entity being rebound", which read as the player or context entity — and a settings screen reached
from the main menu has neither, so the objection was fair on the words used. What was meant is any
entity the caller picks, normally the settings row the player activated, which exists from the main
menu exactly as it does from a pause menu. Capture touches no context entity at all: it reads L1,
which the sampler fills whether or not anything is spawned. What the component buys over a global
session is that "which row is listening" is answered by where it *is*, rather than by the screen
keeping that state beside a session and holding the two in step.

**Arming costs a frame, and the frame is the feature.** The press that opened the capture is still in
the queue when the session arrives, so a session reading the queue immediately binds whichever key
the player activated the row with — the classic version of this bug. A session skips what is already
queued on its first run.

**Excluded and reserved both refuse, and conflating them would have lost the useful half.** An
excluded control is silent, because it is not being refused: it is busy doing its normal job, which
is precisely how the key that cancels a capture reaches the thing that cancels it. A reserved control
is loud, because a player who pressed it meant to bind it and is owed the reason. The example makes
the distinction visible — `Escape` skips a row while `F1` is refused out loud — and it is read from
Bevy's own button state there, with no context spawned anywhere, which is R19.5 demonstrated rather
than asserted.

**Reserving's second half is the half that matters,** and it is what settles OQ-10. Taking no
mapping stops a player rebinding the settings key away; refusing it across the scheme stops them
binding something else *over* it. Only the first is the obvious reading, and only the first is
useless alone. Reserving and declaring a mapping contradict each other, which is a new plan-build
error and a new row in the diagnostics catalogue.

**Writing the example found a real bug, which is what examples are for.** Binding one action to a
key and to a pad button, both mappable, was reported as a duplicate mapping key — but R19.15 says
uniqueness is per *scheme*, and §10.1 stores the two in separate tables. The check was stricter than
the requirement, in the direction that refuses the ordinary way to write a game offering rebinding
on both devices. Both collision checks are now keyed by scheme and name together.

**The class vocabulary came out one short of what the roadmap expected.** There is no
any-directional class, because no single *control* reports a position in two dimensions — a stick is
two axes, a directional composite is four buttons. Since a player rebinds one part at a time, the
case it would serve never reaches capture, and a mapping that accepts `Axis2` is a stick bound
whole, which §9.7 gives a tunable instead. `CaptureSession::for_slot` returns `None` there rather
than offering a capture nothing could satisfy.

**The crate touched an entity after handing it to an observer, which is a rule rather than a
detail.** Capture triggered `Captured` and *then* queued the removal of the session component. An
observer is entitled to do anything to the entity it is answered on, including despawning it — a
settings row that closes when it is answered is ordinary — and the example did exactly that: it
despawned the answered row and spawned the next one, which took the freed index, so the crate's
queued removal found a live entity of the wrong generation. Removal now precedes the trigger, and is
fallible besides, since one run can answer several sessions and the first observer may despawn a
later one's entity. The observer also now sees the component already gone, so "is this row still
listening" reads the same from inside an observer as from outside.

Worth recording because the reproduction failed: the same observer, writing the same deferred
despawn-and-replace, does not error in a small headless app, because whether an observer's deferred
commands run before or after the ones already queued differs between that and a real `DefaultPlugins`
game. The unit test that does pin the fix asserts the *contract* — the component is gone by the time
the observer runs — rather than the crash. A test asserting the crash would have passed before the
fix, and a test that cannot fail is worse than no test.

**Conflicts are detected and deliberately not carried on the event.** Answering "what else holds
this" means reading every declared context, which capture cannot do from the middle of the input
pipeline — and it is the caller's question anyway, since what to do about a clash is a policy. Two
limits are stated rather than hidden: comparison is at control granularity, so chord-differentiated
bindings are reported as overlapping (a false positive, which is the safe direction), and a clash
across two contexts is reported as *possible*, because whether two contexts are ever live together is
a fact about the game's activation rules and not about its bindings.

### Chunk 39: a mapping holds a list

The model could not express "Jump has two keyboard bindings in one row", which is the arrangement
every shipped game's keyboard table has. A mapping held one control, so two mappable bindings of one
action in one scheme were a *collision*, and the only way to ship "W or Up Arrow" was a second row
under an alias name.

**The proof it was wrong was already in the tree.** Dead Zone had `dead_zone.thrust` and
`dead_zone.thrust_alt`, and `dead_zone.turn` and `dead_zone.turn_alt` — four rows telling a player
that two things are separate when they are the same thing bound twice. That was not written as a
workaround, it was written as the only thing that compiled, which is the more useful kind of
evidence. Both aliases are gone; each is now one row holding two controls.

**A list with a capacity, rather than a fixed two or an unbounded list.** The prior art splits three
ways — games use a small fixed number and label the columns "Primary" and "Secondary", tools
(Blender, VS Code) grow an "add shortcut" button, engines offer unbounded as an authoring surface —
and a capacity covers all three without making a game pay for the editor's case. `Capacity::UpTo(n)`
or `Capacity::Any`, and `slots()` is what a table asks to know how many columns to draw.

**Capacity is inferred and raised, never lowered,** which is the part that keeps the common case
free of ceremony. A plain `mappable` asks for one; several bindings feeding one mapping take the
widest anything asked for; and afterwards no mapping is narrower than the defaults it already holds.
So declaring two mappable bindings produces a two-slot row with nobody writing "2", and
`mappable_upto` exists for the other case — one default shipped, a second slot left for the player.

**The collision check inverted before it was right, and the mistake is worth recording** because the
obvious edit is the wrong one. Keying the uniqueness set by `(scheme, key, action)` reads like "a
repeat of the same action is fine", and does the exact opposite: the same action inserting the same
tuple twice is still a duplicate, so the merge case was refused and the collision case passed. What
it needs is a *map* from `(scheme, key)` to the action that claimed it, reported only when a later
claimant differs. Two tests said so immediately, in opposite directions, which is why there were two.

**The cross-context check does not consult the action, and the asymmetry is deliberate.** Two
mappable bindings of one action inside one context are a primary and a secondary and merge. The same
two in two different contexts stay a collision, same action or not, because they are separate rows
in contexts that may be live at different times while the overrides store is keyed by mapping alone.
It is worth stating in the source next to the check, because "same action, and still an error" reads
as a bug otherwise.

**Capture had to name a slot.** A mapping holding a list means a capture has to say *which* slot it
is filling, or the answer has nowhere to go but the front of the row and the secondary column can
never be filled. `CaptureSession::for_slot` names one and `for_mapping` takes slot zero; `Captured`
echoes it back. Slots past the capacity are refused, and so is any slot more than one past what the
mapping holds now — a hole in a list whose order is what primary and secondary *mean* would be a
silent promotion of the secondary later on.

**What is not answered, and where it went.** A control repeated across two slots of one mapping is
not reported by `conflicts`, which excludes the whole target mapping rather than the one slot.
That is a policy question and the policies are chunk 38's, so it is written onto chunk 38 rather
than left as a comment — along with the fact that 38's pending-override set now holds a list per
row, which a set valued by one control would get away with until someone edited a secondary.

**The public docs cite documents the reader does not have, and this chunk only found it.** Around
forty doc comments name an `R`-number, a `§`, an `OQ` or a `D`-decision — eighteen in `capture.rs`
alone, several of them in the module-level block that is its docs.rs landing page. On docs.rs the
requirements and the design document do not exist, so a developer is told that something satisfies
R19.5 and has no way to find out what R19.5 is. The house style already draws this line; nothing had
been checking it. None of chunk 39's own additions are affected, which is how it was noticed at all
— writing to the rule made the surrounding text look wrong. It goes to chunk 28 rather than here,
because it spans four files and none of it changes behaviour, and folding it in would have made this
diff unreadable for the thing it is actually about.

**The two nouns were the wrong way round, and finding out cost the chunk a rename.** The first
version called the row a `Slot` and then needed a second word for one position in it; "cell" was
what came to hand, and it never sat right — a cell belongs to the table a screen draws, so "a slot
holds cells" reads as a drawing inside a data structure. Chasing that discomfort turned up the
actual fault: `slot` already meant *an indexed position in a list* twice over in this crate, in the
plan's per-action state array and in the evaluator, and the public `Slot` was the one use that meant
something else. The good name was on the wrong thing.

So the row became `Mapping` — which also repairs a verb/noun mismatch nobody had noticed, since
`.mappable()` had been declaring a `Slot` — and `slot` moved to the position, where it agrees with
both existing uses. `cell` survives only as a presentation word: **a screen draws one cell per
slot**, and that sentence is the whole of the relationship.

Worth recording how it was done, because the obvious way is a trap. Renaming both nouns at once
would have meant a window in which `slot` meant the row in some files and the position in others,
with nothing to tell a reader or a compiler which. It went in two passes instead — `Slot`→`Mapping`
everywhere first, green, then `cell`→`slot` — so at no point did one word have two live meanings.
The checks after each pass are also why the state-array `slot` in `plan.rs` and `eval.rs` came
through untouched: it was enumerated first and protected explicitly, rather than trusted to a
regex.

**Grooming found three things with no destination, which is two more than expected.** Reverse lookup
(R18.1) had none at all and is now chunk 40 — it is what "Cancel (B)" needs, and Dead Zone's own
screen spec asks for shortcut captions on its buttons. Mouse buttons are chunk 41: `Control` has no
variant for them, `InputFrame` never samples `MouseButtonInput`, and the requirements do not mention
them, so the crate claims keyboard-and-mouse and supports half of it. That one has a hard ordering
constraint — `Control::name` is the stored persistence identity, so it must land before chunk 23 or
the save format needs migrating on its first day. And §18's deferral reason was wrong: it claimed an
asset-pipeline gate, which is true of glyphs and false of R18.5 and R18.6.

### Chunk 41: mouse buttons

The crate named keyboard-and-mouse as a control scheme and supported half of it. `Control` had no
variant for a mouse button, `InputFrame` never sampled `MouseButtonInput`, and no requirement
mentioned them — so "fire on left click", which is the commonest binding in the genre Dead Zone
belongs to, could not be written at all.

**Why it came before the settings screen and before persistence.** Persistence is the hard
constraint: `Control::name` is the stored identity (R17.9), so a variant added after a save format
ships means either a migration or a name chosen to fit around what is already written. The screen is
the soft one — a rebinding table a player cannot put a mouse button into is a hole they find in the
first minute, and finding it after the table is drawn means changing the table.

**A variant is never just a variant.** It reached `Control`, `ButtonControl`, `BindingSource`,
`BindingSourceSpec`, `RawEvent`, the sampler, the plugin's message registration, the evaluator's
held state and its pressed predicate, `capture::arrival`, and the name and label tables — the same
spread chunk 37 covered for the variants that already existed. The one place it did *not* need work
is the mapping model, which is what having a `ButtonControl` abstraction was for: a mouse button is a
composite part and a chord member for free.

**`MouseButton` is `Hash` but not `Ord`,** so the held set is a `HashSet` where the keyboard's is a
`BTreeSet`. Worth a comment in the source, because the asymmetry looks like carelessness otherwise
and R10.3 will eventually want both snapshot-able.

**The stored name and the shown label part company, deliberately, for the thumb buttons.** They are
stored as `mouse/Back` and `mouse/Forward` — what the backend calls them, and the stored string
must not drift — and shown as **Mouse 4** and **Mouse 5**, which is what every other settings
screen the player has seen calls them. Exactly the call §10.3 already made when it decided
`LeftTrigger` renders as a bumper. An unnamed button reads as "Mouse Button 7" rather than
"Mouse 7", so a raw index can never be mistaken for one of those two.

**A feature combination nobody had ever built was broken, and only a sweep found it.** Widening the
`any(keyboard, gamepad)` cfg groups to include `mouse` created a configuration that had not
existed before — mouse alone, no keyboard, no gamepad — and it did not compile:
`CompiledBinding::chord` and
the `ButtonState` import were both gated on keyboard-or-gamepad, and one `cfg(not(...))` fallback had
been left un-widened, so `chord_len` was specified twice. Building all eight combinations of the
three device features is now the check that catches this, and it is worth doing whenever a cfg group
changes rather than only when a feature is added. The `--features libm` build alone would not have
found it; that configuration has no devices at all.

**Where it shows up.** Dead Zone's `Fire` is now Space *and* left mouse, both mappable — one row
with both slots filled, and the first two-control row in the game that is not two keys. The
spare slot moved to `Hyperspace`, so the read-only screen still has a blank cell to draw.

**Not doing the wheel, and it now has a destination.** It is a delta on its own channel rather than a
button, wants the `Line`/`Pixel` normalization R13.3 describes, and shares nothing with a button but
the device. Nothing in tree asks for it, so it went to the deferred table rather than being written
badly here.

**The requirements gap was the real deliverable.** R4.1a states the bindable control set outright,
and R13.0 gives mouse buttons the section they should always have had — §13 separates position,
motion and buttons in its own problem statement and then had requirements for only the first two.

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
  that is always active is not a mode — Dead Zone's `Shell` holds `Pause` unconditionally and belongs
  in the base set, leaving only the state-gated `Flying` as a layer. What genuinely does not
  translate is consumption: a layer shadows or it does not, and there is no equivalent of one context
  claiming a control for a frame. Recorded rather than solved. §10.2 asked for this to be settled
  once rather than twice and it now is, in the same place as R19.15.
- **Suppression is L0.** Above.

**The dependency question answered itself.** `steamworks` is `std`-only, `unsafe` FFI beneath, and
wants the redistributable at link time; this crate is `no_std`, `forbid(unsafe_code)`, and minimal by
manifest comment. So the real backend is someone else's crate and nothing under `src/` may name
Steam — which is a constraint on the seam, not a packaging note, and the thing that tests it is a
mock living entirely in `examples/`. That is chunk 42, and its acceptance criterion is a non-diff:
Dead Zone's pad becomes backend-owned and every file except `actions.rs` is untouched.
`actions.rs:96` already says "the pad is left alone: console and Steam remapping already own that",
which was written as policy and becomes true.

**One correction.** R18.9 said backend glyphs arrive as opaque handles or raw image bytes. Steam
returns a filesystem path, which is neither and which the app has to load. The requirement's
substance was right and only its enumeration of shapes was short — but the same gap one level up is
load-bearing: a backend's *origins* are its own enumeration of physical controls, covering device
families we have no `Control` for, so a reverse lookup returning `Vec<Control>` quietly makes the
trait ours-only. That is now chunk 40's second review surface.

### Chunk 43: listed by default

The player-facing list was opt-in in both senses at once. A binding with no mapping was neither
rebindable nor *visible*, so the only rows a screen could draw were the ones a game had already
offered for remapping — and the commonest gamepad screen in the industry is a read-only list of what
the pad does, with the remapping owned by the platform. We could not draw it from our own data. Dead
Zone's gamepad table is exactly that screen, which is why this had to come before 21 rather than
after it.

**Two questions had been fused into one flag.** *May the player change this* is the developer's call,
because a fixed binding is a design decision. *May the player see this* is the player's business, and
the two want opposite defaults. Splitting them gives three states — listed and fixed, listed and
rebindable, unlisted — and `Rebinding { Here, Fixed }` on the mapping is what a screen reads to
decide whether the row gets a button. R19.10 was rewritten around the split; R4.7 now says only the
rebindable half, which is all it ever meant.

**The flip was proposed as `.listed()` and is better as `.private()`.** An opt-in verb would have
been the third thing to remember to write, and the failure mode of forgetting it is invisible: the
binding works and simply never appears. Opt-out fails the other way — the wrong thing shows up on a
screen, which is a bug you see. The escape hatch is named for what it means rather than for the
mechanism, since a game author's question is "is this the player's business", not "is this listed".

**Four checks had to narrow, and three existing tests found it.** Key uniqueness, the
rebinding-disagreement check, the cross-context collision report and `conflicts` all exist to protect
the override store, and a fixed row never reaches it. Left as they were, they turned an ordinary
arrangement into an error the moment listing became the default: one action bound in two contexts is
now two listed rows under one name, which is what R19.13 promises a game that offers no rebinding at
all. So all four require at least one side to be rebindable, and the diagnostic returns the moment a
`mappable` is added to either — which is the moment it can do harm. Worth noting that the tests
failed for the *right* reason and I nearly widened the exception instead of narrowing the check.

**`private` and `mappable` panic on each other, in both orders.** One says the player may not see it
and the other says they may change it, and a builder chain that quietly picked a winner would be a
silent wrong answer in the one place a wrong answer is invisible. `.private()` asserts the binding is
not already rebindable; `declare_mapping` asserts the mapping is still there, which covers
`.private().mappable()` without a second check.

**It found the gap chunk 44 exists for.** Listing by default put `Afterburner` on the screen beside
`Thrust`, under its own name, holding the same three controls — and the question it raised was not
about listing at all: if those are two mappings, a player can rebind `Thrust` to `J` and leave the
afterburner on `W`, and then put `Fire` on `W` and afterburn by holding fire. Nothing collides, so
`conflicts()` cannot see it; the failure is a *separation* that should not have been possible.
Afterburner is a logical extension of Thrust and should move with it, which the model has no way to
say. That is chunk 44, and Dead Zone carries `private` on those three bindings until it lands —
which produces the right screen and none of the linkage, and the comment there says so.

**Two rename regressions from chunk 39, found on the way.** `slot` had two live meanings before that
chunk renamed one of them, and my protect-list missed two places where the surviving meaning — the
state-array position — was written in prose: Design's "Action → slot assignment" and chunk 33's
"slots ordered topologically". Both had been silently converted to "mapping" and both were wrong.
The lesson is that a protect-list keyed on identifiers does not cover prose, and the sweep for a
rename this size has to be a reading rather than a grep.

**What is still opt-in, and deliberately.** Rebinding stays a declaration, and §0's accessibility
paragraph says why that is uncomfortable: the jam entry's likely outcome is a game with zero
remappable controls. The listing flip pays some of that back for free — a game that declares nothing
still has controls a player can read, which is the readable half of R20.1 — and the obligation the
paragraph names, making the accessible path *cheap*, is unchanged.
