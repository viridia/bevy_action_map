# Steam examples

Examples that read the pad through Steam Input rather than through Bevy's gamepad support. They need
a running Steam client, a pad, and a layout bound by hand, so nothing in CI runs them;
`scripts/verify.sh --full` only checks that they compile.

This is its own Cargo workspace, so `steamworks-sys` never builds as part of the crate's. macOS
only, for now: every setup step below was measured on macOS, and Windows has not been tried.

## Disasteroids

The base game from `examples/disasteroids`, with Steam owning the pad. `main.rs`, `actions.rs`,
`glyphs.rs` and `steam.rs` are this crate's own; every other module is the base game's, linked in by
path.

The keyboard plays exactly as it does in the base game. Every pad binding there is an `Authority`
binding here, fed by `steam.rs` from Steam's action data. The build leaves out `bevy_gilrs`, so the
pad reaches the game through Steam alone and never also as an ordinary gamepad.

### Setup

1. **Install the action manifest.** Steam loads a local manifest for a borrowed app id only from
   inside its own app bundle (`docs/steam.md` S4):

   ```sh
   cp steam_examples/disasteroids/game_actions_480.vdf \
     ~/Library/Application\ Support/Steam/Steam.AppBundle/Steam/Contents/MacOS/controller_config/
   ```

   Only one manifest can be installed for app 480 at a time, so this replaces whatever is there,
   including the probe's. Steam reads it at each launch, so the client need not restart (S23). A
   manifest Steam cannot accept fails silently: the log's "resolved 0 of 14 actions" is the only
   sign (S22).

2. **Run it**, with Steam running and logged in:

   ```sh
   cargo run --manifest-path steam_examples/Cargo.toml --bin disasteroids
   ```

   `cargo run` finds Steam's library in the vendored SDK. A binary started any other way needs
   `libsteam_api.dylib` copied next to it from `steamworks-sys`'s
   `lib/steam/redistributable_bin/osx/` (S15).

3. **Bind the layout.** The layout Valve publishes for app 480 binds Spacewar's actions, so against
   this manifest every binding is empty and every action reads at rest, with no error anywhere
   (S16). A real game publishes a default layout for its own app id, so its players never do this
   step; a borrowed id cannot, and the demo ships no layout file in its place, because one would go
   stale with every retitled action (S25). With the game running, open the controls screen with F2
   and press **Change in Steam** under the pad's table, with the mouse or with the arrow keys and
   Enter. That opens Steam's binding panel, which forks a personal copy (S25). Bind every action in
   both sets. The copy is per controller type, so a different kind of pad starts blank again. The
   Library's own controller configurator is the wrong one: it configures whatever launched the game,
   not app 480. Doing it the way the base game does:

   | Set | Action | Control |
   | --- | --- | --- |
   | Flying | Thrust | right trigger |
   | Flying | Turn | left stick |
   | Flying | Fire | A |
   | Flying | Smart bomb | right bumper |
   | Flying | Hyperspace (double-tap) | B |
   | Flying | Pause | Menu |
   | Flying | Debug overlay | View |
   | Flying | Controls screen | Y |
   | Controls screen | Move selection | left stick |
   | Controls screen | Apply and close | X |
   | Controls screen | Back | B |
   | Controls screen | Clear cell | left bumper |
   | Controls screen | Close controls screen | Y |
   | Controls screen | Press selected button | A |

   A layout can unbind every way the pad has into and out of the controls screen, and on macOS the
   pad has no other way to reach a configurator (S29). The keyboard is the way back: F2 opens the
   screen, and the button under the pad's table opens the panel again.

   The afterburner is not in the list. It follows Thrust, so holding the trigger opens it up as it
   does in the base game.

### Prompts

While Steam reports a pad, the game's prompts name its controls in Steam's own art, read from the
client's install, and follow the layout: a change in Steam's binding panel shows on the next frame.
Without a pad they name keys, as in the base game. A missing install leaves the pad's prompts as
Steam's words for its controls.

### What does not work yet

- **One pad.** The first pad Steam lists flies the ship; a second does nothing.
