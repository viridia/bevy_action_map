//! Lets [`bevy-remote-driver`](https://github.com/viridia/bevy_remote_driver) run plans against an
//! example, without the example knowing it is under test. Each example includes this file alone,
//! with `#[path]` and `mod remote_driver;`, rather than all of `common`.

use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    remote::{RemotePlugin, http::RemoteHttpPlugin},
};

/// Enables the Bevy Remote Protocol on the port the driver gives in `BEVY_REMOTE_DRIVER_PORT`, and
/// the frame count it reads. Without the variable it adds nothing, so a normal start opens no port.
pub struct RemoteDriverPlugin;

impl Plugin for RemoteDriverPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(port) = std::env::var("BEVY_REMOTE_DRIVER_PORT") {
            let port = port
                .trim()
                .parse()
                .expect("BEVY_REMOTE_DRIVER_PORT is not a port");
            app.add_plugins((
                RemotePlugin::default(),
                RemoteHttpPlugin::default().with_port(port),
                FrameTimeDiagnosticsPlugin::default(),
            ));
        }
    }
}
