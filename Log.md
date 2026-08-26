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
Disasteroids had already forced `Default` and `Clone` onto every one by hand. Four derives became
one, across the tree. The `Component` impl is written out rather than delegated, which is two
associated items in this version of Bevy and will break loudly if that changes; the escape hatch is
to implement `InputContext` by hand, which is three constants.

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

**Mappings ride the type-erased door too.** `mapping::mappings(world)` walks the same registry,
needing only `&World` because mappings come from the plan resource rather than from anything an
entity carries. Disasteroids' overlay grew four lines that list what a player would be shown — the
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

**The proof it was wrong was already in the tree.** Disasteroids had `disasteroids.thrust` and
`disasteroids.thrust_alt`, and `disasteroids.turn` and `disasteroids.turn_alt` — four rows telling a
player that two things are separate when they are the same thing bound twice. That was not written
as a workaround, it was written as the only thing that compiled, which is the more useful kind of
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
(R18.1) had none at all and is now chunk 40 — it is what "Cancel (B)" needs, and Disasteroids' own
screen spec asks for shortcut captions on its buttons. Mouse buttons are chunk 41: `Control` has no
variant for them, `InputFrame` never samples `MouseButtonInput`, and the requirements do not mention
them, so the crate claims keyboard-and-mouse and supports half of it. That one has a hard ordering
constraint — `Control::name` is the stored persistence identity, so it must land before chunk 23 or
the save format needs migrating on its first day. And §18's deferral reason was wrong: it claimed an
asset-pipeline gate, which is true of glyphs and false of R18.5 and R18.6.

### Chunk 41: mouse buttons

The crate named keyboard-and-mouse as a control scheme and supported half of it. `Control` had no
variant for a mouse button, `InputFrame` never sampled `MouseButtonInput`, and no requirement
mentioned them — so "fire on left click", which is the commonest binding in the genre Disasteroids
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

**Where it shows up.** Disasteroids' `Fire` is now Space *and* left mouse, both mappable — one row
with both slots filled, and the first two-control row in the game that is not two keys. The spare
slot moved to `Hyperspace`, so the read-only screen still has a blank cell to draw.

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

### Chunk 43: listed by default

The player-facing list was opt-in in both senses at once. A binding with no mapping was neither
rebindable nor *visible*, so the only rows a screen could draw were the ones a game had already
offered for remapping — and the commonest gamepad screen in the industry is a read-only list of what
the pad does, with the remapping owned by the platform. We could not draw it from our own data.
Disasteroids' gamepad table is exactly that screen, which is why this had to come before 21 rather
than after it.

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
say. That is chunk 44, and Disasteroids carries `private` on those three bindings until it lands —
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

### Chunk 21: the settings screen, read-only

Disasteroids has a controls screen: `F2` or Y opens it, the same control closes it, and it lists
every binding the game declared in two tables. It reads nothing but `mappings()` — no action type,
no context, no key — so the same file would draw a different game's controls unchanged. The one
action it names is the one that closes it, and it names that to render its own caption.

**The column count is the data's, and the same code draws both tables.** A row says how many
controls it can hold and the widest row in the table decides how many cells every row draws, which
comes out as three columns for the keyboard — name, primary, secondary — and two for the pad, with
the screen saying neither. That falls out of chunk 39's model being right about a fixed row: a
mapping the player cannot change has exactly the slots its defaults fill, so a table of fixed rows
is one control wide without anything having to special-case it. Hyperspace's spare secondary is the
case that proves the other half — an empty cell the player will fill, drawn rather than absent.

**A category had no way to be rendered.** A mapping's name is a `MappingKey` with a fallback label
and its category is a bare `&'static str`, so the first screen to draw headings had to write its own
title-casing next to the crate's. `mapping::fallback_label(key)` is now the same courtesy for any
key, and `MappingKey::fallback_label` is written in terms of it. That was the only thing the screen
needed that D7 did not already offer, which is the answer to this chunk's review surface.

**The screen is a game state, not a flag.** Proposed as a `Showing(bool)` resource with change
detection, corrected to a `States` enum on the same terms as `Game::Paused`: there is one fact about
whether the screen is up rather than a screen and a flag that have to agree. Closing it then needs
no code at all — `DespawnOnExit` on the root is the whole of it, since closing *is* despawning — and
the pause banner, which had carried a hand-written despawn system since chunk 13, lost it in the
same edit. Two idioms for one thing in one example was the worse half of that diff.

**It is a separate state from `Game`, so the game keeps running behind it.** Making it a third
`Game` variant would have stood `Flying` down for free — and would have destroyed what chunk 30 is
for. The ship still answers the throttle while the screen is up, which is precisely the arrangement
that chunk needs: a screen over a live game, binding the same controls at a higher priority and
consuming them. Leaving the bug visible is what makes the fix demonstrable.

**The caption is a reverse lookup done by hand.** "Press F2 or North Button to close" comes from
filtering the mapping list by `ActionId` and reading the controls back out, which is chunk 40's job
done the slow way — and it confirms that chunk's instinct that a scan should be written before an
index. It is written onto 40 as the caller that already exists.

**A screen nobody can find is not a screen.** `F1` and `F2` were discoverable only by reading the
source, so the game now carries a dim line in the corner naming both. Its text is read out of the
mapping list rather than written down — the same move the close caption makes, and for the same
reason: a string naming a control is wrong the moment somebody changes the control.

**Two label overrides, and they are the app's.** `disasteroids.turn.negative` derives as "Turn
Negative" and a player should read "Turn Left", which is what a catalogue is for: the screen answers
for the two keys whose derived text is wrong and leaves the rest to the fallback. That the fallback
is legible for every other row is the point of it existing.
### Chunk 40: reverse lookup

The question every other path through the crate throws away the answer to. `Prompts` is a trait with
one method — given an action and a scope, the controls that would fire it now — and `BindingTable`
is this crate's answer to it. Disasteroids' two hand-rolled captions read through it instead of
scanning the mapping list.

**The two lists diverged, and that is the finding.** `mappings()` and a reverse lookup look like the
same query filtered differently, and they are not. A mapping list is what the game *declared*: a
controls screen has to draw a row whether or not anything is carrying the context, because the row
is a fact about the game. A prompt is what would fire *now*, so it is empty for a context nobody
carries and for one that is switched off, and it includes a `private` binding — `private` says the
row would duplicate another row, which is a statement about the list and not about whether the key
works. Once that was clear the implementation followed: the lookup reads the compiled plan through
its own type-erased door rather than filtering `mappings()`, and the two doors sit beside each other
on `DeclaredContext`.

**The first thing it broke was the caller it was built for.** Disasteroids' corner hint ran in
`Startup`, alongside — and unordered against — the `Startup` system that spawns the context whose
controls it names. The mapping list did not care, so nothing had ever needed that ordering; a lookup
that asks what is live does, and an unordered pair is a coin toss the build makes rather than a bug
that reproduces. It moved to `PostStartup`. That is the whole cost of the runtime/declared
distinction showing up in one line of a game, and it is worth knowing that it shows up at all: a
caller that used to be order-independent is not any more.

**Consumption has two readings and the literal one is wrong.** R18.2 says the answer must reflect
consumption, and the obvious implementation consults `ConsumedControls` — which holds what has
actually been claimed *this tick*. A claim lands only while the claiming action fires, so a caption
built on it would flicker as the player pressed things. What a prompt wants is the standing fact: a
control bound with `consume` in a stronger active context never reaches the weaker one. That is
computed from the plans and from which contexts are live, and it moves only when a context
activates. The ordering it needs — render tick before fixed tick, then priority, then declaration —
is §5.2's rule, reused rather than reinvented, which is also what makes it the ranking of the
result.

**The ranking says what it does not know.** Contexts rank, bindings rank within a context, and
devices do not rank at all, because nothing tracks which one the player is holding (R18.6). Ordering
keyboard before gamepad would have been a guess wearing a ranking's clothes. A caller that knows
passes a `Scope`; Disasteroids' corner hint passes `Scheme::KeyboardMouse` and takes the first,
which is the shape a caller that *does* know has. *Groomed straight afterwards:* the hole is not a
hole. R18.6 is withdrawn and the device is the caller's parameter for good, so this is the answer
rather than a placeholder for one.

**The return type is not a `Control`, and this was the cheap moment.** R18.9's point is that a
backend's origins are its own enumeration, covering device families we have no variant for, so
`Origin` is either one of ours or a name-plus-label pair from somewhere else. Both answer `name()`
and `fallback_label()` — the two strings chunk 37 established — so a caption renders one without
asking where it came from. Nothing in tree constructs the foreign variant yet; the deferred table's
R18.9 row loses its origin half and keeps its glyph half.

**The chord came along, the conditions did not.** A binding requiring a modifier alongside its own
control reports both, because a prompt that dropped it renders `Ctrl+S` as "S" — wrong rather than
unpolished, which is the line this chunk's review surface asked to be drawn. R18.3's structured
descriptor stays unbuilt on the other side of it: a held binding and a tapped one on the same key
still produce the same prompt, and `Afterburner` is the case in tree.

**A scan, and the index is still not warranted.** §10's sketch assumed the inverse of the plan's
control index. Writing the scan first was the honest order and the answer is that the callers are a
handful of captions rebuilt when a screen opens. Chunk 47 is what would change it, and its own
answer there is change detection rather than an index — so the index is the move after that one
fails, not before.

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

### Housekeeping, between 47 and the next chunk

Three commits with no chunk between them: the example renamed, `rebind` renamed to `mapping`, and
the document corrections both renames turned up.

**Disasteroids, and the reason the pun had to go.** The example was Dead Zone, which was funny and
pedagogically backwards — a reader meeting it while learning what a deadzone *is* meets a game that
has nothing to do with one. The mechanical half was worse than the joke: its action paths were
`dead_zone.thrust` and `dead_zone.flight`, so the game's namespace sat one token from the crate's
own `DeadZone` in `rebind.rs` and `binding.rs`, the two files that teach both. Two `///` comments
were using the example's namespace to explain `fallback_label` on docs.rs, where no reader has an
example to refer to; those moved to `gameplay.*`, which the rest of the file already used.

**`rebind` became `mapping`, and the argument was not that it holds mappings.** `binding.rs` holds
bindings, composites, modifiers and conditions and is named for the primary one; a module holding
mappings, tunables and presets and named for an *operation* performed on them was the odd one out.
The rename restores the pairing the module doc already claimed — `binding` is the developer's model,
`mapping` is the player's. `Rebinding`, `mappable` and every prose "rebind" as a verb stay; the type
is chunk 48's call and is a good name for what it says.

**The same rename lesson, for the third time.** Chunk 39 left two stale spellings that chunk 43
found; this pass left one that the *first* sweep of this session missed. It survived because the
deferred table's Tunables row does not spell the name: it said tunables were wanted "by a game named
after a deadzone". No grep for the old title finds a paraphrase of the old title. Chunk 43's entry
already says a rename this size has to be a reading rather than a grep — three instances is a
pattern, and the reading has to cover the places that describe the thing instead of naming it.

**Two ground-rule-5 findings, from re-reading the prelude instead of the list.** Chunk 48 enumerated
a dozen bare nouns; the prelude exports sixteen. `Obstacle`, `Timestamp`, `Rebinding` and `Actions`
were missing, and a name that chunk does not list is a name it walks past — `Timestamp` being the
one another crate is most likely to export too. Separately, R18.3's condition half had no
destination in either document: chunk 47 landed the chord half of the descriptor and said the
condition half "stays unbuilt", and nothing carried it. It is now a deferred row gated on chunk 44,
which is the chunk that takes `private` off `Afterburner` and so the moment a held binding and a
tapped one appear on one screen. 44 carries a bullet saying so, because a gate nobody is watching
is not a destination.

### Chunks 29 and 30: a screen you can move around

Landed as one chunk, because the two halves only test each other. The crate gained two combinators
and a screen to prove them on: a stick or a D-pad now moves the selection on Disasteroids' controls
screen, the game underneath keeps flying and never hears the keys the screen has taken, and the
whole thing can be operated from an Xbox pad without touching the keyboard.

**The two names were the review surface, and the argument for both is that neither is about
navigation.** `.compass(CompassPoints::Four)` rounds a 2D value to a compass point and throws the
magnitude away; `.on_change()` fires on the ticks a value differs from the tick before. Eight-way
movement wants the first for its own reasons and it is the cheapest condition in the set, needing
only the previous value `Scratch` already carries. What makes them a menu is that they compose:
rounding alone still fires every tick, change detection alone fires on every wobble across a
boundary, and together they fire once per point *entered*. `.pulse(0.25)` beside them is auto-repeat
out of a third combinator that was already there.

**`Scratch::prev` was lying, and had been since chunk 11.** Its doc says "the previous input value";
`evaluate` wrote `ActionValue::Bool(actuated)` over it in the shared preamble before any condition
saw it, which was all the built-in set ever needed. A condition comparing two directions cannot work
from that, and neither can anyone's custom one. Storing the whole value costs nothing — every other
arm reads `prev.to_bool()`, which is the same answer either way — and the fix is one line. Worth
recording because the field's documentation was correct and the code was not, which is the direction
that stays invisible: nobody re-reads a doc comment that already says the right thing.

**Consumption stopped following the fire and started following the verdict.** `.consume()` claimed
its controls only on ticks where the binding reported `Fired`, with a comment explaining that a
binding merely *bound* to a control should not hold it against everyone else. That reasoning is
right and the rule drawn from it was too narrow: a menu binding that fires once per direction
entered says nothing between crossings, so holding the arrow key moved the selection once and then
turned the ship on every tick after. The claim now lasts while the verdict is `Ongoing` too, which
is one rule — a binding claims its controls while it has something to say — and it fixes two cases
nobody had complained about yet: a charging `.hold()` and a part-way `.multi_tap()` were both
leaking their keys to whatever was underneath. R8.2 carries the amendment.

**The screen is its own context, and that was not the plan.** Chunk 30 was written expecting
`active_in_state(Settings::Showing)` beside `Flying`'s. What it got is better: the screen's root node
carries the `Menu` component, so the context exists for exactly as long as the screen does and there
is no activation condition anywhere. That is R22.14 — spawning must be sufficient — turning out to
be the *simpler* option rather than the permissive one, and it means despawning the screen releases
every control it took with nothing saying so.

**The prediction chunk 13 made held.** A settings screen over a running game is a higher-priority
context binding a subset of the same controls and consuming them, and it needed nothing declared
beyond that. Cross-domain consumption carried it without being asked: `Menu` is render-tick and
`Flying` is fixed-tick, and §5.2's rule that a `PreUpdate` claim stands for every fixed tick in the
frame is what makes an arrow key reach the screen and not the ship.

**What the spatial heuristic cost was one component per widget**, and the roadmap was out of date
about it. Chunk 29 said `bevy_input_focus` had the directional half only as a `SystemParam`;
`bevy_ui` has since grown `AutoDirectionalNavigation` and an `AutoDirectionalNavigator` that scores
candidates by edge distance and perpendicular overlap. So there are no navigation links in the
example at all — the table's layout is the graph — and the only placement decision is `AutoFocus` on
Cancel, which is independent of tab order and saves the screen from naming a first cell and then
keeping that name true as the table changes.

**The finding worth the chunk: consumption cannot reach a widget that reads input itself.**
`bevy_ui_widgets::Button` activates on `Enter` and `Space`, and `Space` is the ship's trigger. The
mapper claiming `Space` does nothing about it, because the keyboard event reaches the button through
`InputDispatchPlugin`, which is the only thing turning a global key event into a focused one and
asks the mapper nothing. So R8.2 was met between contexts and unmet against the widget beside them
— written up as R8.2a, since a requirement that is true of the mechanism and false in the game needs
to say so in the requirements rather than in a log entry.

The first reading of this was that the fix belonged in Bevy. It does not: `DefaultPlugins` is a
plugin *group*, so an app can disable one member and add a mapper-aware replacement, which makes it
chunk 49 behind the `focus` feature and no upstream conversation at all. The workaround until then
is an action bound to `Enter` and `Space` that consumes them and has no observer anywhere —
Disasteroids' `Swallowed` — which is ugly in exactly the way that keeps it from being mistaken for
the design.

**Two things declined, both with gates rather than intentions.** R22.7's bubbling dispatch was chunk
29's stated deliverable and is not built: bubbling exists so that something can intercept, and this
screen has nothing that wants to swallow a direction, so a `FocusedInput` would have been ceremony
around a four-line call to the navigator. The gate is a widget that intercepts — a slider, a scroll
area, a text field. And R22.5's "initial delay + repeat rate" is one number rather than two, because
the pulse's clock starts on the tick the change fires; the gate is a screen long enough for equal to
be wrong.

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