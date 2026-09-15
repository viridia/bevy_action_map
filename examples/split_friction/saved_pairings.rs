//! What each pane keeps between sessions: which device it was on, and which preset it had chosen.
//!
//! The crate hands out a [`DeviceId`] and a preset's name and no opinion about where either goes;
//! here they go into `bevy_settings`, one section per pane:
//!
//! ```toml
//! [player1]
//! preset = "split_friction.southpaw"
//!
//! [player1.device.gamepad-model]
//! product = 2835
//! vendor = 1118
//!
//! [player2]
//! preset = "split_friction.classic"
//!
//! [player2.device.keyboard-mouse]
//! ```
//!
//! A section per pane rather than one shared one, because a pane's settings are two facts and not
//! one: the preset it had chosen, and the device it was on.
//!
//! An empty table is a pane with no device — see [`SavedDeviceId`], which exists because a reflected
//! `Option` writes its empty case as something TOML cannot spell.
//!
//! Restoring a *pad* happens on connection rather than at startup, because that is when the pad
//! exists: a platform reports the ones already plugged in as connection events during the first
//! frames, so "restore what was saved" and "a pad came back mid-game" are the same moment reached
//! twice. [`reconnect`](crate::reconnect) owns the second and this module the first, and they do
//! not overlap — a pane waiting out a disconnect is still paired, so it is never a candidate here.
//!
//! Restoring the *keyboard* happens when the pane spawns, because the keyboard never connects. It
//! is always there, so there is nothing to wait for.
//!
//! Two panes that saved identical controllers cannot be told apart, on any platform measured, so
//! the first pad to arrive goes to whichever of them is checked first. That is not a choice this
//! module could make better: the ids are equal, and nothing else distinguishes them.

use std::time::Duration;

use bevy::ecs::system::Command;
use bevy::prelude::*;
use bevy::settings::{ReflectSettingsGroup, SaveSettingsDeferred, SettingsGroup, SettingsPlugin};
use bevy_action_map::device::{DeviceHandle, DeviceId, Identity, KeyboardMouseId, SavedDeviceId};
use bevy_action_map::player::{DeviceConnected, Paired};

use crate::protagonist::{ClaimedDevices, Protagonist, claim_slot};
use crate::reconnect::KnownDevice;

/// Reverse-DNS, per `SettingsPlugin`. Names the directory the settings file lives in.
const APP_ID: &str = "org.bevy.bevy_action_map.split_friction";

/// What pane 0 keeps between sessions.
#[derive(Resource, SettingsGroup, Reflect, Default, Debug)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "player1")]
struct PlayerOneSettings {
    /// The device this pane was on, empty until it has been on one.
    device: SavedDeviceId,
    /// The preset this pane was on, empty until it has been on one.
    preset: String,
}

/// What pane 1 keeps. A second group rather than an index into one, so each pane's settings read as
/// a section a person could edit.
#[derive(Resource, SettingsGroup, Reflect, Default, Debug)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "player2")]
struct PlayerTwoSettings {
    /// The device this pane was on, empty until it has been on one.
    device: SavedDeviceId,
    /// The preset this pane was on, empty until it has been on one.
    preset: String,
}

pub fn plugin(app: &mut App) {
    // Registration has to precede `SettingsPlugin`, which reads the whole type registry once when
    // it is built and never again. `ActionMapPlugin` has already claimed the `gamepad-model`
    // identity domain by now, which is what lets a stored one resolve back to a type.
    app.register_type::<PlayerOneSettings>();
    app.register_type::<PlayerTwoSettings>();
    app.add_plugins(SettingsPlugin::new(APP_ID));

    app.add_observer(restore_on_connect);
    app.add_observer(restore_keyboard_on_spawn);
    app.add_observer(save_on_pair);
    app.add_observer(forget_on_unpair);
}

/// The preset a pane was last on, for [`claim_slot`] to start it there.
///
/// A pane that has never chosen one, and a name no preset answers to, both read as the empty string
/// — [`popup::apply_preset`](crate::popup::apply_preset) resolves that to `CLASSIC`, so the
/// fallback lives in one place rather than being decided twice.
pub fn stored_preset(world: &World, pane: Entity) -> String {
    let Some(&Protagonist(slot)) = world.get::<Protagonist>(pane) else {
        return String::new();
    };
    match slot {
        0 => world.resource::<PlayerOneSettings>().preset.clone(),
        1 => world.resource::<PlayerTwoSettings>().preset.clone(),
        _ => String::new(),
    }
}

/// A pane changed preset, so the file learns it too.
///
/// Called from [`popup::apply_preset`](crate::popup::apply_preset), the one place a pane's preset
/// changes, rather than watched for on the component. The device beside it *is* watched, and the
/// difference is that the device has no such single place — [`reconnect`](crate::reconnect) inserts
/// [`KnownDevice`] for reasons of its own — while the preset does.
///
/// Guarded on the value actually differing, and only the matching pane's resource is touched, so a
/// pane restored to the preset it was already on does not rewrite the file to say so — the change
/// detection this rests on is described on `store`.
pub fn store_preset(world: &mut World, pane: Entity, preset: &'static str) {
    let Some(&Protagonist(slot)) = world.get::<Protagonist>(pane) else {
        return;
    };
    match slot {
        0 if world.resource::<PlayerOneSettings>().preset != preset => {
            world.resource_mut::<PlayerOneSettings>().preset = preset.to_string();
        }
        1 if world.resource::<PlayerTwoSettings>().preset != preset => {
            world.resource_mut::<PlayerTwoSettings>().preset = preset.to_string();
        }
        _ => return,
    }
    SaveSettingsDeferred(Duration::from_secs_f32(0.5)).apply(world);
}

/// A pane that was on the keyboard takes it back as soon as it exists.
///
/// The keyboard raises no connection event to hang this off — it is simply always available — so
/// the pane appearing is the moment, and the settings have already loaded by then: `SettingsPlugin`
/// reads the file while it is being built, before any system runs.
fn restore_keyboard_on_spawn(
    spawned: On<Add<Protagonist>>,
    one: Res<PlayerOneSettings>,
    two: Res<PlayerTwoSettings>,
    panes: Query<&Protagonist>,
    mut claimed: ResMut<ClaimedDevices>,
    mut commands: Commands,
) {
    let Ok(protagonist) = panes.get(spawned.entity) else {
        return;
    };
    if saved_device(protagonist.0, &one, &two) != Some(DeviceId::new(KeyboardMouseId)) {
        return;
    }
    if claimed.claim(DeviceHandle::KeyboardMouse).is_none() {
        return;
    }
    commands.queue(claim_slot(spawned.entity, DeviceHandle::KeyboardMouse));
}

/// The device saved for a pane.
fn saved_device(slot: u8, one: &PlayerOneSettings, two: &PlayerTwoSettings) -> Option<DeviceId> {
    match slot {
        0 => one.device.0.clone(),
        1 => two.device.0.clone(),
        _ => None,
    }
}

/// A pad a pane was on last session turns up, so the pane takes it without anyone pressing Join.
///
/// Only unpaired panes: one already playing is not looking for a pad, and one waiting out a
/// disconnect is [`reconnect`](crate::reconnect)'s business.
fn restore_on_connect(
    connected: On<DeviceConnected>,
    one: Res<PlayerOneSettings>,
    two: Res<PlayerTwoSettings>,
    identities: Query<&Identity>,
    panes: Query<(Entity, &Protagonist), Without<Paired>>,
    mut claimed: ResMut<ClaimedDevices>,
    mut commands: Commands,
) {
    let DeviceHandle::Gamepad(device) = connected.device else {
        return;
    };
    let Ok(identity) = identities.get(device) else {
        return;
    };

    let Some((pane, _)) = panes.iter().find(|(_, protagonist)| {
        saved_device(protagonist.0, &one, &two).as_ref() == Some(&identity.0)
    }) else {
        return;
    };
    // `claim` refuses a device that already holds a slot, which is what stops the second of two
    // panes that saved the same model taking the pad the first one just took.
    if claimed.claim(connected.device).is_none() {
        return;
    }
    commands.queue(claim_slot(pane, connected.device));
}

/// A pane learned which device it is on, so the file learns it too.
///
/// Watches [`KnownDevice`] rather than `Paired`, because that is the component that exists only
/// once the device's identity is actually known — which is the thing worth storing.
fn save_on_pair(
    known: On<Insert<KnownDevice>>,
    panes: Query<(&Protagonist, &KnownDevice)>,
    one: ResMut<PlayerOneSettings>,
    two: ResMut<PlayerTwoSettings>,
    mut commands: Commands,
) {
    let Ok((protagonist, device)) = panes.get(known.entity) else {
        return;
    };
    store(
        protagonist.0,
        SavedDeviceId(Some(device.0.clone())),
        one,
        two,
    );
    commands.queue(SaveSettingsDeferred(Duration::from_secs_f32(0.5)));
}

/// Disconnect was pressed, so the pane stops being restored on the next launch.
///
/// A pad going away on its own does not reach here: that leaves [`KnownDevice`] standing precisely
/// so the pane can be handed the same pad again, this session or the next.
fn forget_on_unpair(
    known: On<Remove<KnownDevice>>,
    panes: Query<&Protagonist>,
    one: ResMut<PlayerOneSettings>,
    two: ResMut<PlayerTwoSettings>,
    mut commands: Commands,
) {
    let Ok(protagonist) = panes.get(known.entity) else {
        return;
    };
    store(protagonist.0, SavedDeviceId(None), one, two);
    commands.queue(SaveSettingsDeferred(Duration::from_secs_f32(0.5)));
}

/// Writes a pane's device into whichever resource holds it.
///
/// Takes both resources by value so change detection only fires on the one written — `ResMut`'s
/// `DerefMut` is what marks a resource changed, and `bevy_settings` decides whether to write the
/// file by asking exactly that.
fn store(
    slot: u8,
    device: SavedDeviceId,
    mut one: ResMut<PlayerOneSettings>,
    mut two: ResMut<PlayerTwoSettings>,
) {
    match slot {
        0 => one.device = device,
        1 => two.device = device,
        _ => {}
    }
}
