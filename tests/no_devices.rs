//! A separate compilation unit from `src/`'s unit tests, which assume a keyboard is available —
//! this is the one place that actually runs `ActionMapPlugin` with none of `keyboard`, `mouse`, or
//! `gamepad` enabled. `cargo check` and `cargo clippy` see this configuration too, but neither runs
//! a schedule, so neither would have caught chunk 91's panic on the first `PreUpdate`.

use bevy_action_map::ActionMapPlugin;
use bevy_app::App;

#[test]
fn action_map_plugin_updates_with_no_device_features() {
    let mut app = App::new();
    app.add_plugins(ActionMapPlugin);
    app.update();
}
