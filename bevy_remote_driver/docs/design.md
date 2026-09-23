# How `bevy_remote_driver` works

A driver for a running Bevy app, for automated tests. Its sections are numbered `DD1`–`DD9`, and its
requirements are [requirements.md](./requirements.md)'s `DR` numbers.

The plugin, DD3, and the client, DD4, DD5.1 and DD5.2, are built. The rest is the design the
remaining implementation chunks build to, and becomes the description of what is built as they land.
Where a claim rests on reading Bevy's source rather than on running something, it says so.

---

## 1. Shape

Two halves, joined by BRP over HTTP:

```
app under test                          client (Python, standard library only)
  RemotePlugin, RemoteHttpPlugin          launches the app, waits for the port
  RemoteDriverPlugin                      runs a plan step by step
    driver.select, driver.locate,   <-->  writes input with BRP's own methods
    driver.diagnostics; SceneReady        saves screenshots, writes the report, closes the app
```

The plugin is small on purpose. It adds a server method only where BRP cannot answer from what is
already reflected, and each of those has an upstream destination (DD8). Everything else, including
all input, goes through BRP's built-in methods (DR1.3), and the client is where the driver's actual
work happens.

`RemoteDriverPlugin` adds `RemotePlugin` and `RemoteHttpPlugin` itself, and only when the
environment variable `BEVY_REMOTE_DRIVER_PORT` is set, on that port. Without it the plugin adds
nothing, so a build containing it opens no port when started normally (DR1.5). A value that is not a
port panics at startup, since running without the port would leave the client timing out with no
reason given. An app does not add either remote plugin itself, since Bevy refuses a plugin added
twice.

## 2. What BRP already does

Measured against this crate's `minimal` example with the `bevy/bevy_remote` feature, macOS, in chunk
145's spike, except where marked read:

| Need | BRP | Finding |
| --- | --- | --- |
| Write a key | `world.write_message`, `bevy_input::keyboard::KeyboardInput` | reaches `bevy_action_map` with or without focus |
| Screenshot | `world.spawn_entity` a `Screenshot`, then `world.observe+watch` `ScreenshotCaptured` | empty when the window is covered: every byte zero, alpha included, and the same inside the app |
| Bring the window forward | `world.mutate_components`, `Window.focused` to `true` | `bevy_winit` calls winit's `focus_window`; a capture 0.1 s later is correct |
| Click | `world.write_message`, `WindowEvent::CursorMoved` then `MouseButtonInput` | read, from Bevy's `examples/remote/integration_test.rs` |
| Quit | `world.write_message`, `bevy_app::AppExit` | read: `AppExit` is a reflected `Message` |
| Connect a gamepad | `world.spawn_entity` an empty entity, then `world.write_message` `GamepadConnectionEvent` naming it | read: `gamepad_connection_system` inserts `Gamepad` onto the entity the event names |

Two behaviours matter for every test. A covered window runs at about half the frame rate, so timing
is measured in frames (DD4.3). And a window brought forward takes focus from whatever had it, the
editor included, which is the cost of a correct screenshot.

`KeyboardFocusLost` fires once when the window loses focus, and `bevy_action_map` releases held
controls on it. So a test that holds a key while someone clicks another window sees the key
released, which is the mapper behaving correctly rather than the driver failing.

## 3. The plugin

### 3.1 Selectors

`driver.select { path }` returns every entity the path matches (DR2.1). A path is an array of names.
The last name matches an entity's `Name` exactly; each earlier name must match one of that entity's
ancestors, in order from the root down, at any depth. `["Settings", "Jump", "Rebind"]` is an entity
named `Rebind` with an ancestor `Jump` that itself has an ancestor `Settings`.

An array needs no escaping, since a `Name` may contain any character, and leaves the server nothing
to parse. The client also takes a path as a string, `"Settings/Jump/Rebind"`, and splits it on `/`
before sending it; a name containing `/` is written in the array form. Reports print a path joined
with `/`.

The method walks every entity with a `Name` and checks its `ChildOf` chain, which is linear in named
entities and fine at a test's scale. A step that needs one entity treats zero or several matches as
a failure and reports the matches with their full name paths (DR2.2).

### 3.2 Readiness

BSN triggers `Ready` on each entity of a scene once it and its dependencies have finished spawning.
It is an `EntityEvent` without `Reflect`, and it is an event: a client that spawns a scene and then
starts observing can miss it.

The plugin adds a global observer that inserts `SceneReady`, a reflected marker component, on each
`Ready` target. Readiness then becomes state, which a query sees, and which is still there for a
wait that starts late (DR3.2). A `present` step with `"ready": true` selects, then asks BRP's own
`world.get_components` whether each match has `SceneReady`.

`Ready` covers the assets the scene itself names. An asset something in the scene requests after it
spawns, such as a prompt's glyph, is not among them, and nothing signals when it has loaded. A plan
waits for those with `seconds` (DR3.6). Loading runs on IO threads rather than per frame, so time is
the right measure there, where the app's own logic is measured in frames.

### 3.3 Where a node is drawn

`driver.locate { path }` resolves one UI node and returns `{ window, logical }`: the window it is
drawn in, and its centre in logical pixels, `UiGlobalTransform`'s translation divided by that
window's scale factor. The window is the one the node's camera renders to, read from
`ComputedUiTargetCamera`, which Bevy propagates from a root's `UiTargetCamera`; a node with no
camera falls back to the primary window. A node drawn to an image rather than a window is refused.
The method only locates. The client writes the click itself, using BRP (DD5.1).

### 3.4 Diagnostics

`driver.diagnostics` returns the latest value of each diagnostic in `DiagnosticsStore`, by path.
Nothing in `bevy_diagnostic` is reflected, so BRP cannot read the store itself. Every wait measured
in frames reads `frame_count` from it, which `FrameTimeDiagnosticsPlugin` records from `FrameCount`.
That plugin is not in `DefaultPlugins`, so the driver's plugin adds it when it is missing (DR1.4).
`is_plugin_added` sees only plugins added before the driver's, so an app that adds
`FrameTimeDiagnosticsPlugin` too must add it first, or Bevy panics on the duplicate. A value is an
`f64`, which holds a frame count exactly for longer than any test runs.

Returning every diagnostic rather than only the frame count costs nothing, so a test that wants
another, such as a floor on `fps`, needs only a client step to read it.

## 4. Plans and the client

The client is its own, rather than an existing harness such as Playwright or Jasmine. What makes
Playwright worth having is its browser half, which cannot reach a native window, so the rest would
be a Node toolchain wrapped around the same BRP calls. Standard-library Python costs no dependency
beyond the interpreter this project's own tooling already needs.

### 4.1 A plan

A plan is a JSON file: the example to launch and a list of steps. JSON so that the client reads it
with the standard library (DR7.3). An example outside the workspace's root package also names its
`package`, as the driver's own `testbed` does.

```json
{
  "example": "disasteroids",
  "steps": [
    { "present": "Title", "ready": true },
    { "click": "Title/Start" },
    { "frames": 30 },
    { "key": "Escape" },
    { "present": "Pause", "timeout": { "seconds": 0.4 } },
    { "screenshot": "paused" },
    { "absent": "Title" }
  ]
}
```

The three checks wait rather than look once, because what follows an input can land a frame or more
later: a menu that animates out, a value set by the next system to run.

| Step | Passes when | Fails when |
| --- | --- | --- |
| `present` | the selector matches; with `"ready": true`, a match also has `SceneReady` | the timeout is reached |
| `absent` | the selector matches nothing | the timeout is reached |
| `expect` | a component on the one selected entity has the given value | the timeout is reached |

A check's `timeout` takes `frames`, `seconds` or both, and ends at whichever is reached first; a
limit it leaves out does not apply. It defaults to 300 frames and 10 seconds (DR3.4). A timeout of
`{ "frames": 0 }` looks once, so checking that something is still gone later is a `frames` step
followed by an `absent` that looks once.

The other steps:

| Step | Does |
| --- | --- |
| `click` | locate, move the cursor, press, release, each on its own frame |
| `key` | press, one frame, release; `"hold": n` holds for `n` frames |
| `press`, `release` | one half of a key, for chords and holds that span other steps |
| `type` | a string, a press and a release per character, with Shift around a shifted one |
| `pad` | a virtual gamepad's button or axis (DD5.3) |
| `frames` | let `n` frames pass |
| `seconds` | let `n` seconds pass |
| `screenshot` | save a PNG of the primary window under the given name |

A plan's step is an object with one key naming the step, whose value is the step's first argument;
its other keys are the rest, by name. `key`, `press` and `release` take `logical_key` and `text` for
a key the US table lacks.

### 4.2 A Python plan

When a test needs a loop, a branch or a computed value, it is a Python program instead, calling the
same functions the JSON runner calls (DR7.2). The step table is the client's API: each step is a
method on a `Driver` object with the same name and arguments. The program sets `EXAMPLE`, and
`PACKAGE` where a JSON plan would, and defines `run(driver)`; it runs through the same command as a
JSON plan, and a failure's report adds the line of the program that made the failing call.

### 4.3 A run

`python3 bevy_remote_driver/client/run.py <plan>` is the one command (DR6.1). It:

1. builds the example with `cargo build --example`, and fails fast on a build error. Cargo's JSON
   messages give the binary's path and its package's directory;
2. picks a free port, and starts the binary with `BEVY_REMOTE_DRIVER_PORT` set, its output going to
   a log file. `CARGO_MANIFEST_DIR` is set to the package's directory, where Bevy looks for
   `assets/` as it would under `cargo run`. On macOS `DYLD_FALLBACK_LIBRARY_PATH` names the
   toolchain's libstd and the Bevy dylib, which `dynamic_linking` needs;
3. polls until the app answers a query for its primary window, which every key step names;
4. runs the steps;
5. writes `AppExit`, waits up to three seconds, then kills the process. This runs in a `finally`, so
   an interrupted run closes the app too (DR6.2).

Everything a run produces goes to `target/remote-driver/<example>/<plan name>/`, emptied first: the
log, the screenshots and `report.txt`. The report is also printed. A failure gives the step's number
and text, what it expected, what it found, whether the app had exited, and those paths (DR6.3). The
exit status is 0 on a pass, 1 on a failure and 2 when the plan could not run.

Consecutive inputs land on distinct frames (DR3.5). After writing an input the client reads
`frame_count`, and it does not write the next input until the count has passed that value. The read
is answered no earlier than the frame the input was, since BRP answers requests in order, so a
larger count is a later frame.

### 4.4 Screenshots

A covered window captures an empty image (DD2), so the step first sets `Window.focused` and waits a
tenth of a second. It then opens a `world.observe+watch` stream on `ScreenshotCaptured`, spawns a
`Screenshot` of the primary window, and takes the event whose entity is the one spawned. Watching
first means the capture cannot finish before anything is listening. The image arrives as BGRA bytes
in a JSON array, which the client reorders and writes as a PNG with `zlib`. A capture whose every
byte is zero fails the step rather than saving a blank file.

## 5. Input

### 5.1 Clicks

`driver.locate`, then three `world.write_message` calls of `WindowEvent`, each on its own frame:
`CursorMoved` to the centre, `MouseButtonInput` pressed, then released. Picking needs the cursor to
have moved before the press, and a press and a release in the same frame are indistinguishable from
a tap the app may treat differently.

### 5.2 Keys and text

A key is a `KeyboardInput` message naming the primary window. The client derives `logical_key` and
`text` from the key code for the printable keys of a US layout and the common named keys, so `type`
produces what a real keyboard would. A test that needs another layout, or a key the table lacks,
supplies both itself. The message goes to `KeyboardInput` directly rather than through
`WindowEvent`, which is what the spike measured.

### 5.3 Gamepads

Read, not run. The client spawns an empty entity, writes a `GamepadConnectionEvent` naming it, then
writes `RawGamepadEvent`s for its buttons and axes. `bevy_input` does the rest as it would for a
gilrs device. A real pad plugged in at the same time is a second gamepad, not a conflict.

## 6. The mapper's half

`bevy_action_map` gets a `remote` feature that registers two methods of its own (R25), so the driver
never depends on the mapper (DR1.2):

- `action_map.dump` returns what `inspect::dump` produces: each context, its instances, and each
  action's phase and value, by path. A test asks whether an action fired without the app printing
  anything.
- `action_map.authority { entity, action, value }` sets a delegated action's value on a context
  entity, as `AuthorityValues::set` does, with the action named by path through
  `ActionId::from_path`.

There is no injection at the level of a raw event, because none is needed: a message BRP writes is
sampled as a real one. The authority level exists for a context that delegates, and for nothing
else. A test of an ordinary bound action sends a key.

What this answers of `docs/issues.md`'s three candidates:

- **1048, virtual devices.** Resembles only. A test's key arrives as the real keyboard, which leaves
  a test fixture no special case to need, but on-screen sticks and bots are still the requirement's
  subject and still unserved.
- **1041, pumped sampling.** Resembles only. Measuring in frames makes a test independent of frame
  rate without stopping sampling. Rewind is still the case that would need it.
- **1025, driving a context from outside.** Unrelated. That is an in-process harness with no world,
  and a remote driver always has one.

## 7. Ids in the examples

An id is a `Name`, given in a scene with BSN's `#Name`, which already inserts one (DR2.3). No
component of the driver's own: `Name` is already there, already reflected, and already what an
inspector shows. Names are given to what a plan needs to select, and not to every entity.

Each example adds `RemoteDriverPlugin` in its `main`. The examples take the driver as a
dev-dependency, and `bevy` gains its `bevy_remote` feature there, which every example build then
compiles. The plugin is inert unless the port variable is set. Adding the plugin and the names is an
intended diff in `examples/`, which the chunk adding them says it is.

## 8. Where each piece ends up

| Piece | Home | Why |
| --- | --- | --- |
| Selection by name path | upstream, a filter on `world.query` | any app's test wants it, and it needs nothing but `Name` and `ChildOf` |
| Where a UI node is drawn, for a click | upstream, `bevy_remote` or `bevy_ui` | the scale factor and target-camera steps are Bevy's own knowledge, which every client repeats |
| Readiness as state | upstream, `bevy_scene` | a `Reflect` derive on `Ready` would not fix the race; the scene spawner leaving a component would |
| Diagnostics over BRP | upstream, `Reflect` on `DiagnosticsStore` and what it holds | about as small as reflecting `FrameCount` alone, and makes every diagnostic readable through `world.get_resources`, frame count included |
| Bringing the window forward before a screenshot | the client | a policy for tests, not a property of screenshots |
| Keys, text, gamepads, clicks | the client | the messages are already reflected |
| Plans, the runner, the report | the driver crate | upstream has no reason to want a Python client |
| Action state and authority values | `bevy_action_map`, `remote` feature | the mapper's own types |

Every server method the plugin adds is a stopgap for an upstream gap. If all four close, the plugin
is `SceneReady`'s observer at most, and the crate is its client. That is the expected end state, and
why the plugin is kept this small.

## 9. Writing a test

For an agent writing a test: the instructions DR7.4 requires. `plans/testbed/` holds a JSON plan and
a Python plan that between them use every step.

1. **Find what to select.** Read the example's scenes for `#Name`s. If what the test needs has none,
   add one where the entity is declared, rather than selecting by component or position.
2. **Write a JSON plan** under `bevy_remote_driver/plans/<example>/`, with the steps of DD4.1. Start
   with a `present` on the screen's root with `"ready": true`. Name the `package` if the example is
   not the root crate's. Use a Python plan only for what JSON cannot say.
3. **Name a component by its full type path**, such as `bevy_ui::widget::text::Text`, as BRP does.
   `expect` compares only the keys its value gives, so give the fields the test is about and no
   others.
4. **Wait for a condition, then frames, then seconds.** A covered window runs slower, so a margin in
   seconds for the app's own logic passes in front and fails behind the editor. `seconds` is for
   assets loaded after a scene is ready, such as prompt glyphs, and nothing else.
5. **Run it with the one command, and nothing else:**
   `python3 bevy_remote_driver/client/run.py <plan>`, from anywhere in the workspace. Every approval
   prompt brings the editor forward and covers the window, so a run split across several commands
   tests a different app from the one that runs in one. Do not start the app in one command and
   drive it in another.
6. **Read a failure from the report first**, then the log, then the screenshots, in that order. The
   report names the step, and the log says what the app did. An exit status of 2 is the plan or the
   build, not the app.
7. **A key released unexpectedly is focus.** If someone clicked another window during a hold, the
   mapper released the key. Rerun it before looking for a bug.
8. **Leave the screenshots for the person reading the run.** A screenshot shows that something is
   drawn. It does not replace an `expect` or an `absent`, which are what make a test fail.
