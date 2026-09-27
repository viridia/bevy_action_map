# Driving a running Bevy app from a test

A specification for testing a running app from outside, over the Bevy Remote Protocol: two small
additions to Bevy, and a vocabulary of test steps that a client in any language can implement. It is
drawn from a working prototype, used for end-to-end tests of an input-mapping crate's examples.

Three gaps the prototype worked around have since closed upstream: diagnostics over BRP
([bevy#25824](https://github.com/bevyengine/bevy/pull/25824)), clicks in physical pixels
([bevy#25890](https://github.com/bevyengine/bevy/pull/25890)), and writable gamepad messages
([bevy#25904](https://github.com/bevyengine/bevy/pull/25904)). What remains is smaller than it was.

## What Bevy needs

### Selecting entities by a path of names

A test refers to things by name ("the Rebind button in the Jump row"), and `world.query` filters
only by component type. Finding one entity from outside today means fetching every `Name`, then
walking `ChildOf` chains from the client, a round trip per ancestor.

Proposed: a `name_path` filter on `world.query`.

```json
{ "filter": { "name_path": ["Settings", "Jump", "Rebind"] } }
```

- The last name matches an entity's `Name` exactly.
- Each earlier name matches one of its ancestors, in order from the root down, at any depth. Unnamed
  entities in between are skipped, so a scene names only what a test refers to.
- An array rather than a `/`-separated string, so a name may contain any character and the server
  parses nothing.

The result is an ordinary query result, combining with `with` and `without`. A test that wants one
entity treats zero or several matches as its failure. The prototype also returns each match's full
name path, so that failure can say which match was which; whether a query should return that is
open.

### Readiness as state

A test has to wait for a scene to finish spawning before touching it. `bevy_scene` announces that
with `Ready`, an entity event: a client that starts watching after the scene spawned misses it, and
deriving `Reflect` on `Ready` would not change that.

Proposed: `bevy_scene` leaves a reflected marker component on each entity it triggers `Ready` for,
so readiness is state a query sees whenever it looks. The prototype does this with a global
observer, and calls the marker `SceneReady`. If a component on every scene entity is too much to
impose, it can be opt-in, on a setting of the scene plugin.

`Ready` covers the assets a scene names. An asset requested after the scene spawns, such as an image
a system loads later, is not covered, and a test waits for it by time.

### Already possible

- **Counting frames:** `diagnostics.get` for `frame_count`, recorded by
  `FrameTimeDiagnosticsPlugin`, which is not in `DefaultPlugins`.
- **Clicking a UI node:** read its `UiGlobalTransform`, which is in physical pixels, and follow
  `ComputedUiTargetCamera` to the camera's `RenderTarget` for the window. All of it is reflected, so
  no server method is needed.
- **Everything else a test does**, from keys to screenshots, uses BRP's existing methods.

## A test's vocabulary

A test is a list of steps. The same steps can be written as a program or, when the test is a flat
list, as JSON that any client can run:

```json
{
  "example": "disasteroids",
  "steps": [
    { "present": "Title", "ready": true },
    { "click": "Title/Start" },
    { "key": "Escape" },
    { "present": "Pause", "timeout": { "seconds": 0.4 } },
    { "screenshot": "paused" },
    { "absent": "Title" }
  ]
}
```

**Checks** wait rather than look once, because the effect of an input can land a frame or more
later.

| Step | Passes when |
| --- | --- |
| `present` | the path matches; with `"ready": true`, the match is ready too |
| `absent` | the path matches nothing |
| `expect` | a component on the one matched entity has the given value |

A check's `timeout` takes `frames`, `seconds` or both, and ends at whichever comes first. The
default is 300 frames and 10 seconds; `{ "frames": 0 }` looks once.

**Actions:**

| Step | Does |
| --- | --- |
| `click` | moves the cursor to the node's centre, presses, releases |
| `key` | presses and releases a key; `"hold": n` holds it for `n` frames |
| `press`, `release` | one half of a key, for chords and holds spanning other steps |
| `type` | a string, with Shift around each shifted character |
| `pad` | a virtual gamepad's button pressed and released, or with `"value"`, a control left there |
| `frames`, `seconds` | lets time pass |
| `screenshot` | saves a PNG of the primary window |

**Rules every client follows.** These are what made the prototype's tests reliable:

- **Each input lands on its own frame.** After writing one, read `frame_count`, and write the next
  only once it has advanced. BRP answers requests in order, so a larger count is a later frame. A
  press and a release in one frame are missed by a system that reads `pressed()`.
- **App logic is timed in frames, loading in seconds.** A covered window can run at half its frame
  rate, while assets load on IO threads regardless.
- **A screenshot brings its window forward first.** A covered window captures an image of zeros. The
  client watches `ScreenshotCaptured` before spawning the `Screenshot`, and fails on an all-zero
  capture rather than saving it.
- **A key carries its `logical_key` and `text`**, derived from the key code for a US layout, so
  `type` produces what a real keyboard would.
- **A virtual gamepad** is an empty entity, a `GamepadConnectionEvent` naming it, then
  `RawGamepadEvent`s. It reports no vendor or product id, so a game shows it as an unrecognised pad
  on every machine.

## Clients

Who writes the test decides what the client should be:

- **An acceptance test in CI** wants minimal dependencies and the project's own language: Rust, on
  `BrpClient` ([bevy#25837](https://github.com/bevyengine/bevy/pull/25837)), with each test an
  ordinary test.
- **A probe written on the spot**, by a developer or an LLM, and then thrown away, wants no project
  to set up. The prototype's client is Python with no dependencies beyond the standard library,
  which also runs the JSON form.

This proposal does not ask Bevy to ship either. The vocabulary and the rules above are the contract,
and a JSON test runs the same on any client that implements them.

## The prototype

[`bevy_remote_driver`](https://github.com/viridia/bevy_action_map/tree/main/crates/bevy_remote_driver),
in the `bevy_action_map` repository. Its plugin adds `driver.select`, a method standing in for the
`name_path` filter, and the `SceneReady` observer, along with stopgaps for locating a node and
reading diagnostics that the changes above make redundant. Beside it are the Python client and tests
against two examples.
[`docs/design.md`](https://github.com/viridia/bevy_action_map/blob/main/crates/bevy_remote_driver/docs/design.md)
has the full step reference.
