# Chunks landed before the changelog

The Roadmap's index of landed chunks, frozen when `CHANGELOG.md` replaced it. Many of these chunks'
commits do not name them, so this is where `scripts/show.py` finds that one landed. A chunk landed
since is found through its commits instead.

| #   | Chunk                                             |
| --- | ------------------------------------------------- |
| 1   | Workspace and module skeleton                     |
| 2   | Action identity, value, and intent                |
| 3   | Derive macros                                     |
| 4   | Input frame, keyboard only                        |
| 5   | First end-to-end slice                            |
| 6   | Axis sources and composites                       |
| 7   | Modifiers                                         |
| 8   | Gamepad and the design-stage deadzone             |
| 24  | Housekeeping                                      |
| 9   | Tick domains and the windowed drain               |
| 15  | Source channel shape                              |
| 16  | Disasteroids, first playable                      |
| 12  | Transition log and observers                      |
| 13  | Context activation lifecycle                      |
| 11  | Conditions and the scratch table                  |
| 14  | Arbitration and consumption                       |
| 32  | Activation by run condition                       |
| 17a | Runtime failures (R24.4)                          |
| 17b | Plan-build diagnostics                            |
| 36  | Type-erased inspection and the overlay            |
| 18  | Derive completion                                 |
| 19  | Mappings and localization keys                    |
| 37  | Naming a control                                  |
| 20  | Interactive capture, conflicts, reserved controls |
| 39  | A mapping holds a list of slots                   |
| 41  | Mouse buttons                                     |
| 43  | Listed by default                                 |
| 21  | The settings screen, read-only                    |
| 40  | Reverse lookup                                    |
| 47  | A binding as a text span                          |
| 29  | Directional navigation                            |
| 30  | The settings screen, interactive                  |
| 44  | Bindings that travel together                     |
| 50  | What a held control says                          |
| 53  | A context the player never sees                   |
| 38  | Applying a rebind                                 |
| 54  | Conflict policy                                   |
| 25  | Control classes and class bindings                |
| 56  | Split Friction's tileset                          |
| 57  | A generated dungeon                               |
| 58  | `follow` replaces per-binding `follows`           |
| 31  | The settings screen, rebinding                    |
| 61  | Exclusive contexts                                |
| 45  | Presets                                           |
| 23  | Persistence of overrides                          |
| 55  | A file a person can read                          |
| 62  | Release on focus loss and disconnect              |
| 64  | Tunables and hold-vs-toggle                       |
| 67  | Per-entity `apply_overrides`                      |
| 26  | Device routing (core)                             |
| 66  | The join gesture                                  |
| 68  | Split-screen cameras and protagonists             |
| 69  | AABB collision                                    |
| 27  | Split Friction's device selection                 |
| 10  | The compiled plan, and the state layout settled   |
| 22  | The deadzone chain, stages 1 and 3                |
| 52  | What the crate has accreted                       |
| 74  | One admissibility rule, not two                   |
| 75  | Four names for a 2×2, twelve times over           |
| 80  | Same-priority contexts, ordered                   |
| 81  | A rebound row keeps its declared capacity         |
| 84  | A save file from a build that came later          |
| 87  | A stick is a control                              |
| 82  | A text field beside a live context                |
| 85  | A dead zone at full deflection                    |
| 86  | `active` and `is_active`, told apart               |
| 93  | A shared toggle ignored the hold setting          |
| 79  | `ActionPhase` tells building from firing           |
| 88  | The gamepad-settings warning sees the global thresholds |
| 76  | `Unresolved`, once                                |
| 70  | Device brand and class                           |
| 91  | The crate does not do what its own documents say  |
| 17c | The two normalizes                                |
| 89  | `why_not` can see the pairing                     |
| 48  | Names that survive a glob import                  |
| 90  | A context nobody declared says so                 |
| 96  | `BindingModifier`'s blanket `Modifier` impl       |
| 97  | A binding whose only conditions are blocking fires at rest |
| 95  | Pong, shared across the single-concept demos          |
| 106 | A diagnostic overlay for Split Friction                |
| 107 | A diagnostic overlay for Pong                          |
| 99  | The smart bomb, and a charge meter                     |
| 100 | A preset can carry a tunable                          |
| 51  | The constitution, trimmed                             |
| 77  | `context.rs`'s test fixtures, deduplicated and reordered |
| 78a | `DeviceFamily` moves to `device.rs`                   |
| 78b | `binding.rs` is three files, and `mapping.rs` gains a library |
| 78c | `context.rs` is two files, plus a shared test fixture module |
| 94a | A binding can name the logical key, not only the physical one |
| 108 | Focus loss, regated                                   |
| 111 | An outside authority writes an action's value at L2   |
| 114 | Update Bevy, and migrate the BSN syntax               |
| 113 | A device's brand is a component, not a lookup at every read |
| 71  | Per-player presets, behind a pause popup              |
| 103 | A disconnect signal and a reconnect prompt            |
| 72  | Device identity, and a pad that returns to the pane that lost it |
| 72d | A pairing that survives a restart, through `bevy_settings`       |
| 92  | Bindings that survive a restart, through `bevy_settings`         |
| 92b | A pane's chosen preset survives a restart                        |
| 116 | A backend-neutral gamepad pool, and a join listener per device   |
| 117a | The comments that were wrong, and the focus placeholder          |
| 117c | The evaluator's duplication clusters, merged                      |
| 117d | Shadow, require-reset, and the test docs that narrated bugs       |
| 117e | The serialization docs, and the last public-item citations       |
| 117f | The device and frame group, and `Brand`/`Identity` told apart     |
| 117g | The binding group's listing, capacity, rescale and toggle clusters |
| 117h | The presentation group's naming, follower and prompt clusters     |
| 117i | Disasteroids' working-copy refrain, and the capture examples      |
| 117j | The other examples and `tests/`, and the last chunk references    |
| 117k | The public docs that contradicted the code                        |
| 117l | `devfmt` holds YAML frontmatter verbatim                          |
| 117m | A rewrap stops changing what the text is                          |
| 117o | A preview that shows the change, not the filename                 |
| 117n | Repacking is what `devfmt` does, and a bare run is refused        |
| 117q | The preview stops showing unchanged code                          |
| 119  | Bevy 0.20.0-rc.1, from crates.io rather than git                  |
| 118a | Capacity is the app's business, and a resource-set ceiling replaces it |
| 118b | A slot may be empty                                                    |
| 118c | A slot is addressed, not appended                                     |
| 120  | A cell the player can empty, and a refusal that says so               |
| 124  | Disasteroids reserves the way back to its own settings screen         |
| 125  | A stage after the fold, declared once per action                      |
| 126  | Opposite contributions cancel                                         |
| 127  | A composite expands into one binding per part                         |
| 35   | Disabling an action                                                   |
| 94b  | Either modifier                                                       |
| 128  | A rebinding row that shows its chord                                  |
| 129  | An override that can name a chord                                     |
| 136  | A gallery of prompts                                                  |
| 133  | An icon prompt for a chord                                            |
| 134  | A prompt names what an action is bound to                             |
| 132  | Consumption that reaches a gamepad                                    |
| 110  | A glyph for a control, and an icon prompt                             |
| 123  | Advice on a mapping set a player cannot break                         |
| 109  | Reflect where a scene, a tool or a save file reaches the type         |
| 137  | Arbitration that names whose input it is                              |
| 130  | Capture as a sensor, answering on the release                         |
| 145  | A remote driver: requirements and design                              |
| 146  | The driver's plugin                                                   |
| 147  | The driver's client and runner                                        |
| 148  | Ids in the examples                                                   |
| 150  | A virtual gamepad                                                     |
| 131  | A plan slot as one struct                                             |
| 138  | A plan compiled once, not once per context                            |
| 139  | The action registry, and what an `ActionId` can reach                 |
| 140  | A mapping key derived in one place                                    |
| 141  | A binding input has one part                                          |
| 151a | An authority is a binding for one device family                       |
| 152  | A follower rides its leader's authority                               |
| 153  | A preset skips a family an authority owns                             |
| 154  | An authority held at spawn is held over                               |
| 151b | Disasteroids on Steam                                                 |
| 155  | An authority that stops supplying a held action cancels it            |
| 151d | A delegated row                                                       |
| 157  | The controls screen as a template with a slot                         |
| 151f | The delegated row on the Steam controls screen                        |
| 151c | Prompts from Steam's origins                                          |
| 158  | Reserving an authority is an error                                    |
| 164  | Child crates under `crates/`                                          |
| 171  | The root crate moves under `crates/`                                  |
| 163  | Local multiplayer's types in the prelude                              |
| 144  | One rule for what counts as a character                               |
| 143  | One apply, for the world or for an entity                             |
| 162  | A warning for a modifier idle on a composite part                     |
| 72b  | Calibration the app sets, and measuring withdrawn                     |
| 174  | A section, printed by its anchor                                      |
| 165  | `gamepad/`, and the two pieces that exist                             |
| 167  | Rumble as a component                                                 |
| 168  | The Steam build fills them                                            |
| 169  | The design proposal                                                   |
| 175  | Every mention of a name, in one call                                  |
| 176  | A dependency's source, at the locked version                          |
| 156  | Navigating from no focus                                              |
| 179a | `Started<A>` reaches observers, with a tick-script fixture            |
| 179b | A hold charges once per tick                                          |
| 179c | Require-reset holds through a time condition                          |
| 179d | A claim arriving cancels what it took                                 |
| 179e | A claim lifting waits for a release                                   |
| 180  | `fold`'s names say what they span                                     |
| 181a | The evaluator's benchmark, and a probe for unrelated events           |
| 181b | `eval.rs` split along the lines that do not change                    |
| 181c | Longer-chord rivals replace the chord pre-pass                        |
| 181d | The affected-bindings index                                           |
| 181e | A binding's pipeline records each reading once                        |
| 181f | `docs/architecture.md` brought up to date                             |
| 183  | Bevy 0.20.0-rc.2, and the driver's gamepad message registration gone  |
| 184  | Inline prompts from the block art                                     |
| 159  | `docs/decisions.md` compressed, the longest entries first             |
| 112a | An authority beside its own family's controls                         |
| 112b | Filtering raw input at L0, with no example using it                   |
| 177  | An example, launched and checked                                      |
| 187  | A context on a resource's entity                                      |
| 188a | Steam's prompts answered per action set                               |
| 188b | A prompt lookup names its context                                     |
| 185  | `show.py` takes an issue number                                       |
| 170  | Prompt art from an ordered list of providers                          |
| 189  | One prompt component, written through templates                       |
| 190  | An inline icon prompt's size, as a `Val`                              |
| 194  | Bevy 0.20.0                                                           |
| 161  | Public items nothing outside the crate names, narrowed or tested      |
| 195  | The first publish to crates.io                                        |
| 196  | `main` on Bevy main, and a release branch for 0.1                     |
| 121  | A camera that takes the mouse, and gives it back                      |
| 203  | A binding editor with a list per action                               |
| 129b | A screen that sets a modifier                                         |
| 204  | Plans on `bevy-remote-driver`, and the driver's copy deleted          |
