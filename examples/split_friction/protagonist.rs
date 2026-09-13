//! The two protagonists: sprites, movement, and how each one is claimed.
//!
//! Both read the same [`OnFoot`] context, bound to a stick and to arrow keys alike; what makes them
//! independently controlled is [`Paired`], not two different contexts (chunk 26's device routing).
//! Neither carries [`OnFoot`] or [`Paired`] at spawn. [`Lobby`] — a third context, bound only to
//! [`Join`], never paired, so it reads every device — is what [`pair_on_join`] listens to: the
//! first still-unclaimed device to press its button claims the next protagonist in spawn order, 0
//! then 1. This replaces chunk 68's hardcoded pairing (protagonist 1 always the keyboard,
//! protagonist 2 always the first gamepad).

use bevy::image::TextureAtlasTemplate;
use bevy::prelude::*;
use bevy_action_map::device::DeviceHandle;
use bevy_action_map::player::Paired;
use bevy_action_map::prelude::*;

use crate::popup::{self, OpenMenu, Popup};
use crate::saved_pairings;
use crate::tileset;

/// Where a protagonist is trying to move, this tick.
#[derive(InputAction)]
#[action(path = "split_friction.move", output = Vec2, intent = Directional2)]
pub struct Move;

/// The context both protagonists move under — one context type, two independently paired
/// instances. Fixed tick, so both protagonists integrate position at the simulation rate rather
/// than the frame rate.
#[derive(InputContext)]
#[context(path = "split_friction.on_foot", tick = Fixed)]
pub struct OnFoot;

/// One context, never paired, that reads every device until [`pair_on_join`] claims it for a
/// protagonist.
#[derive(InputContext)]
#[context(path = "split_friction.lobby", tick = Render)]
pub struct Lobby;

/// The join gesture (chunk 66). [`Lobby`]'s only binding — A on a pad, Enter on the keyboard.
#[derive(InputAction)]
#[action(path = "split_friction.join", output = bool, intent = Button)]
pub struct Join;

/// Which protagonist a sprite is — `0` or `1`, spawn order, and the order [`pair_on_join`] claims
/// them in.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub struct Protagonist(pub u8);

const SPEED: f32 = 90.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<ClaimedDevices>();
    app.add_context::<OnFoot>(|controls| {
        controls
            .bind::<Move>(Stick::Left)
            .dead_zone(DeadZone::radial(0.2));
        controls.bind::<Move>(DirectionalButtons::arrow_keys());

        controls.bind::<OpenMenu>(GamepadButton::Start);
        controls.bind::<OpenMenu>(KeyCode::Escape);
    });
    app.add_context::<Lobby>(|controls| {
        controls.bind::<Join>(GamepadButton::South);
        controls.bind::<Join>(KeyCode::Enter);
    });

    app.add_systems(FixedUpdate, walk.run_if(in_state(Popup::Closed)));
    app.add_observer(pair_on_join);
}

/// Which device, if any, has claimed each of the two protagonist slots.
///
/// A resource rather than the `Local` a single-writer system could get away with: the popup's own
/// Disconnect (`popup::disconnect_pressed`) needs to free a slot too, so both sides have to reach
/// the same state. Indexed by slot rather than a flat list of claimed devices — a list can say
/// *how many* are claimed, but once Disconnect can free one from the middle, only a slot can say
/// *which* protagonist a freed device should be able to rejoin.
#[derive(Resource, Default)]
pub struct ClaimedDevices([Option<DeviceHandle>; 2]);

impl ClaimedDevices {
    /// Claims the first free slot for `device`, or `None` if it already holds one or both slots
    /// are taken.
    pub fn claim(&mut self, device: DeviceHandle) -> Option<u8> {
        if self.0.contains(&Some(device)) {
            return None;
        }
        let slot = self.0.iter().position(Option::is_none)?;
        self.0[slot] = Some(device);
        Some(slot as u8)
    }

    /// Frees whichever slot `device` holds, if any.
    pub fn release(&mut self, device: DeviceHandle) {
        for slot in &mut self.0 {
            if *slot == Some(device) {
                *slot = None;
            }
        }
    }

    /// Points an already-claimed slot at a different device.
    ///
    /// What a broken pairing healing itself needs: the old device is gone, so `claim` — which
    /// refuses a slot that already holds one — is not the right call here.
    pub fn reassign(&mut self, slot: u8, device: DeviceHandle) {
        if let Some(entry) = self.0.get_mut(slot as usize) {
            *entry = Some(device);
        }
    }
}

/// Both protagonists, as one scene, plus the [`Lobby`] context that pairs them — spawned at
/// `spawn[0]` and `spawn[1]` respectively, neither claimed yet.
pub fn spawn(layout: Handle<TextureAtlasLayout>, spawn: [Vec2; 2]) -> impl Scene {
    bsn! {
        Transform::default()
        Visibility::default()
        Lobby
        Children [
            @{protagonist(layout.clone(), 0, tileset::PROTAGONIST_1, spawn[0])}
            --
            @{protagonist(layout, 1, tileset::PROTAGONIST_2, spawn[1])}
        ]
    }
}

/// One protagonist, unclaimed — [`pair_on_join`] adds [`OnFoot`] and [`Paired`] once a device
/// claims it.
fn protagonist(layout: Handle<TextureAtlasLayout>, index: u8, tile: u8, pos: Vec2) -> impl Scene {
    bsn! {
        Protagonist(index)
        Sprite {
            image: "split_friction/tilemap_packed.png",
            texture_atlas: {TextureAtlasTemplate {
                layout: layout.into(),
                index: tile as usize,
            }},
            custom_size: Vec2::splat(tileset::TILE_SIZE as f32),
        }
        Transform::from_translation(pos.extend(1.0))
    }
}

fn walk(
    time: Res<Time>,
    map: Res<crate::Map>,
    input: ActionsQuery<OnFoot>,
    mut protagonists: Query<(&Protagonist, &mut Transform)>,
) {
    let delta = time.delta_secs();

    // Snapshotted before either protagonist might move, so each one's collision test sees where
    // the other actually was this tick rather than a position this same pass already updated.
    let positions: Vec<(u8, Vec2)> = protagonists
        .iter()
        .map(|(protagonist, transform)| (protagonist.0, transform.translation.truncate()))
        .collect();

    for (entity, state) in input.iter() {
        let Ok((protagonist, mut transform)) = protagonists.get_mut(entity) else {
            continue;
        };
        let other = positions
            .iter()
            .find(|(index, _)| *index != protagonist.0)
            .map_or(Vec2::splat(f32::INFINITY), |(_, pos)| *pos);

        let wanted = state.value::<Move>() * SPEED * delta;
        let pos = transform.translation.truncate();
        let moved = crate::collision::resolve(&map.0, pos, other, wanted);
        transform.translation += moved.extend(0.0);
    }
}

/// Claims one device for one protagonist the moment [`Join`] fires, in spawn order — protagonist 0
/// first, then 1.
///
/// `Fired<Join>` says the action fired, not which of its two bindings did — an ordinary action's
/// value is device-agnostic by design, the same reason [`Move`] never says which stick moved it.
/// So this reads the raw button state directly to find out, exactly the question [`Join`] itself
/// cannot answer. **Not backend-safe**: `Gamepad` and `ButtonInput<KeyCode>` do not exist under a
/// Steam authority (Roadmap's deferred table), so this works only against `gilrs`. Keyboard first:
/// only one keyboard exists, where several pads might have pressed A the same tick, and picking the
/// first found over picking the keyboard first would starve whichever pad lost the race on a tick
/// both fired.
///
/// [`ClaimedDevices`] rather than `join::is_claimed` against a `Query<&Paired>`: two protagonists'
/// join presses landing in the same tick both fire before either `Paired` insert (a deferred
/// command) is actually applied, so a query would see both devices as still unclaimed and race for
/// the same slot. `ClaimedDevices` is updated synchronously inside the observer itself, so the
/// second press to arrive already sees the first's claim.
fn pair_on_join(
    _fired: On<Fired<Join>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut claimed: ResMut<ClaimedDevices>,
    protagonists: Query<(Entity, &Protagonist)>,
    mut commands: Commands,
) {
    let device = if keys.just_pressed(KeyCode::Enter) {
        Some(DeviceHandle::KeyboardMouse)
    } else {
        gamepads
            .iter()
            .find(|(_, gamepad)| gamepad.just_pressed(GamepadButton::South))
            .map(|(entity, _)| DeviceHandle::Gamepad(entity))
    };
    let Some(device) = device else {
        return;
    };
    let Some(slot) = claimed.claim(device) else {
        return;
    };
    let Some((entity, _)) = protagonists.iter().find(|(_, p)| p.0 == slot) else {
        return;
    };
    commands.queue(claim_slot(entity, device));
}

/// Puts a pane into play on a device.
///
/// What "claimed" means in one place, since a press is no longer the only way in: a pairing
/// restored from settings starts a pane exactly the same way.
pub(crate) fn claim_slot(pane: Entity, device: DeviceHandle) -> impl Command {
    move |world: &mut World| {
        let preset = saved_pairings::stored_preset(world, pane);
        // `OnFoot`'s state tables are attached by an `on_add` hook that queues the insert, and the
        // queue is flushed as this statement ends — so the context is on the pane by the next line,
        // which is what `apply_preset` needs. A per-entity apply reaching a pane with no context
        // yet would skip it in silence.
        world.entity_mut(pane).insert((OnFoot, Paired::to(device)));
        // `apply_preset` is what inserts `ActivePreset`, so a pane is stamped once, with the preset
        // it is actually bound to. Stamping a default here first would have the save observer write
        // Classic over the stored name in the moment before the stored name was read back.
        popup::apply_preset(world, pane, &preset);
    }
}
