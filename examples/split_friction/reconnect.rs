//! A lost pad pauses its pane; a new one, if any pane is waiting, resumes it.
//!
//! [`DeviceDisconnected`] and [`DeviceConnected`] are one-shot notifications, not durable state —
//! what the popup actually reacts to is [`AwaitingReconnect`], the model this module derives from
//! them. Its own systems never touch [`Popup`] directly: they only insert or remove the component,
//! and a pair of redraw observers turn that into the popup opening or closing, the same
//! change-detection shape `popup::redraw_preset_label` already uses for `ActivePreset`. That keeps
//! the popup's opening and closing in one place regardless of what caused it — a lost pad here, or
//! the existing Disconnect button.
//!
//! A pad that comes back goes to the pane that lost it, matched on [`Identity`]. Two panes whose
//! pads report the same identity — identical controllers, which report identical ids on every
//! platform measured — still fall back to first-come-first-served, and so does a pad that reports
//! no identity at all. In practice at most one pane is ever waiting, since only one popup can be
//! shown at once (`popup`'s own doc comment).

use bevy::prelude::*;
use bevy_action_map::device::{DeviceFamily, DeviceHandle, DeviceId, Identity, KeyboardMouseId};
use bevy_action_map::player::{DeviceConnected, DeviceDisconnected, Paired};

use crate::popup::Popup;
use crate::protagonist::{ClaimedDevices, Protagonist};

/// A protagonist's device went away, and no replacement has claimed the slot yet.
///
/// A marker rather than carrying the lost `DeviceHandle`: the popup's own text does not vary by
/// device, so no code reads the value back.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AwaitingReconnect;

/// Which device this pane is on, remembered so a returning pad can be told from a stranger's and so
/// the pairing can be stored.
///
/// Absent when the pane has claimed nothing, or is on a pad whose platform reports no ids.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) struct KnownDevice(pub DeviceId);

pub fn plugin(app: &mut App) {
    app.add_observer(remember_identity);
    app.add_observer(on_lost);
    app.add_observer(on_available);
    app.add_observer(open_popup_for_lost_device);
    app.add_observer(close_popup_when_resolved);
}

/// Records the identity behind whatever a pane just claimed.
///
/// An observer on the pairing rather than a line at each site that pairs: joining and reconnecting
/// both insert `Paired`, and neither should have to remember to do this.
fn remember_identity(
    paired: On<Insert<Paired>>,
    panes: Query<&Paired>,
    identities: Query<&Identity>,
    mut commands: Commands,
) {
    let Ok(pairing) = panes.get(paired.entity) else {
        return;
    };
    // The keyboard's identity is a constant — there is one, and it is always there — where a pad's
    // has to be read off the entity the backend spawned for it.
    let identity = match pairing.owner_for(DeviceFamily::Gamepad) {
        Some(DeviceHandle::Gamepad(device)) => identities.get(device).ok().map(|id| id.0.clone()),
        _ if pairing.contains(DeviceHandle::KeyboardMouse) => Some(DeviceId::new(KeyboardMouseId)),
        _ => None,
    };
    if let Some(identity) = identity {
        commands.entity(paired.entity).insert(KnownDevice(identity));
    }
}

/// The crate's signal becomes this app's model: the pane starts waiting.
fn on_lost(lost: On<DeviceDisconnected>, mut commands: Commands) {
    commands.entity(lost.entity).insert(AwaitingReconnect);
}

/// The pane that lost *this* pad takes it back; failing that, the first pane still waiting takes
/// whatever turned up.
fn on_available(
    connected: On<DeviceConnected>,
    waiting: Query<(Entity, &Protagonist, Option<&KnownDevice>), With<AwaitingReconnect>>,
    identities: Query<&Identity>,
    mut claimed: ResMut<ClaimedDevices>,
    mut commands: Commands,
) {
    let arrived = match connected.device {
        DeviceHandle::Gamepad(device) => identities.get(device).ok(),
        _ => None,
    };

    let returning = arrived.and_then(|arrived| {
        waiting
            .iter()
            .find(|(.., known)| known.is_some_and(|known| known.0 == arrived.0))
    });
    let Some((entity, protagonist, _)) = returning.or_else(|| waiting.iter().next()) else {
        return;
    };
    claimed.reassign(protagonist.0, connected.device);
    commands
        .entity(entity)
        .insert(Paired::to(connected.device))
        .remove::<AwaitingReconnect>();
}

/// Redraw, not control flow — opens because a pane just started waiting, not because of whatever
/// caused it.
fn open_popup_for_lost_device(
    added: On<Insert<AwaitingReconnect>>,
    protagonists: Query<&Protagonist>,
    mut next: ResMut<NextState<Popup>>,
) {
    if let Ok(protagonist) = protagonists.get(added.entity) {
        next.set(Popup::Open(protagonist.0));
    }
}

fn close_popup_when_resolved(_: On<Remove<AwaitingReconnect>>, mut next: ResMut<NextState<Popup>>) {
    next.set(Popup::Closed);
}
