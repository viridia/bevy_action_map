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
    app.add_context::<OnFoot>(|controls| {
        controls
            .bind::<Move>(Stick::Left)
            .dead_zone(DeadZone::radial(0.2));
        controls.bind::<Move>(DirectionalButtons::arrow_keys());
    });
    app.add_context::<Lobby>(|controls| {
        controls.bind::<Join>(GamepadButton::South);
        controls.bind::<Join>(KeyCode::Enter);
    });

    app.add_systems(FixedUpdate, walk);
    app.add_observer(pair_on_join);
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
/// A `Local` list of already-claimed devices rather than `join::is_claimed` against a
/// `Query<&Paired>`: two protagonists' join presses landing in the same tick both fire before
/// either `Paired` insert (a deferred command) is actually applied, so a query would see both
/// devices as still unclaimed and race for the same slot. The `Local` is updated synchronously
/// inside the observer itself, so the second press to arrive already sees the first's claim.
fn pair_on_join(
    _fired: On<Fired<Join>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut claimed: Local<Vec<DeviceHandle>>,
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
    if claimed.contains(&device) || claimed.len() >= 2 {
        return;
    }
    let slot = claimed.len() as u8;
    let Some((entity, _)) = protagonists.iter().find(|(_, p)| p.0 == slot) else {
        return;
    };
    commands.entity(entity).insert((OnFoot, Paired::to(device)));
    claimed.push(device);
}
