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
//! Matching a newly connected pad to a waiting pane is first-come-first-served: telling a returning
//! pad apart from a stranger's needs a persistent device identity, which does not exist yet, and in
//! practice at most one pane is ever waiting at a time, since only one popup can be shown at once
//! (`popup`'s own doc comment).

use bevy::prelude::*;
use bevy_action_map::player::{DeviceConnected, DeviceDisconnected, Paired};

use crate::popup::Popup;
use crate::protagonist::{ClaimedDevices, Protagonist};

/// A protagonist's device went away, and no replacement has claimed the slot yet.
///
/// A marker rather than carrying the lost `DeviceHandle`: the popup's own text does not vary by
/// device, so no code reads the value back.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AwaitingReconnect;

pub fn plugin(app: &mut App) {
    app.add_observer(on_lost);
    app.add_observer(on_available);
    app.add_observer(open_popup_for_lost_device);
    app.add_observer(close_popup_when_resolved);
}

/// The crate's signal becomes this app's model: the pane starts waiting.
fn on_lost(lost: On<DeviceDisconnected>, mut commands: Commands) {
    commands.entity(lost.entity).insert(AwaitingReconnect);
}

/// The first still-waiting pane claims a newly connected pad.
fn on_available(
    connected: On<DeviceConnected>,
    waiting: Query<(Entity, &Protagonist), With<AwaitingReconnect>>,
    mut claimed: ResMut<ClaimedDevices>,
    mut commands: Commands,
) {
    let Some((entity, protagonist)) = waiting.iter().next() else {
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
