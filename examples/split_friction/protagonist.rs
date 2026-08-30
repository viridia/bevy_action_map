//! The two protagonists: sprites, movement, and — for now — which device drives which.
//!
//! Both read the same [`OnFoot`] context, bound to a stick and to arrow keys alike; what makes them
//! independently controlled is [`Paired`], not two different contexts (chunk 26's device routing).
//! Protagonist 1 is paired to the keyboard from the moment it spawns. Protagonist 2 spawns with no
//! context at all, and [`pair_first_gamepad`] hands it one — paired to whichever gamepad connects
//! first — the moment one does; until then it stands still. Which device goes to which protagonist
//! is hardcoded here: chunk 27 is where a player picks their own device, and there is nothing to
//! pick between yet without two protagonists already existing to pick for.

use bevy::image::TextureAtlasTemplate;
use bevy::prelude::*;
use bevy_action_map::device::DeviceHandle;
use bevy_action_map::player::Paired;
use bevy_action_map::prelude::*;
use bevy_input::gamepad::Gamepad;

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

/// Which protagonist a sprite is — `0` for the keyboard-paired one, `1` for the gamepad-paired one.
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

    app.add_systems(FixedUpdate, walk);
    app.add_systems(Update, pair_first_gamepad);
}

/// Both protagonists, as one scene — spawned at `spawn[0]` and `spawn[1]` respectively.
pub fn pair(layout: Handle<TextureAtlasLayout>, spawn: [Vec2; 2]) -> impl Scene {
    bsn! {
        Transform::default()
        Visibility::default()
        Children [
            ({keyboard_protagonist(layout.clone(), spawn[0])}),
            ({pending_protagonist(layout, spawn[1])}),
        ]
    }
}

/// Protagonist 1 — paired to the keyboard from the moment it spawns.
fn keyboard_protagonist(layout: Handle<TextureAtlasLayout>, pos: Vec2) -> impl Scene {
    bsn! {
        Protagonist(0)
        OnFoot
        Paired::to(DeviceHandle::KeyboardMouse)
        Sprite {
            image: "split_friction/tilemap_packed.png",
            texture_atlas: {TextureAtlasTemplate {
                layout: layout.into(),
                index: tileset::PROTAGONIST_1 as usize,
            }},
            custom_size: Vec2::splat(tileset::TILE_SIZE as f32),
        }
        Transform::from_translation(pos.extend(1.0))
    }
}

/// Protagonist 2 — no context yet. [`pair_first_gamepad`] adds one once a gamepad connects.
fn pending_protagonist(layout: Handle<TextureAtlasLayout>, pos: Vec2) -> impl Scene {
    bsn! {
        Protagonist(1)
        Sprite {
            image: "split_friction/tilemap_packed.png",
            texture_atlas: {TextureAtlasTemplate {
                layout: layout.into(),
                index: tileset::PROTAGONIST_2 as usize,
            }},
            custom_size: Vec2::splat(tileset::TILE_SIZE as f32),
        }
        Transform::from_translation(pos.extend(1.0))
    }
}

fn walk(
    time: Res<Time>,
    input: ActionsQuery<OnFoot>,
    mut protagonists: Query<&mut Transform, With<Protagonist>>,
) {
    let delta = time.delta_secs();
    for (entity, state) in input.iter() {
        let Ok(mut transform) = protagonists.get_mut(entity) else {
            continue;
        };
        let dir = state.value::<Move>();
        transform.translation += (dir * SPEED * delta).extend(0.0);
    }
}

/// Hands protagonist 2 an [`OnFoot`] context paired to whichever gamepad connects first — runs
/// until that happens, then never again.
fn pair_first_gamepad(
    mut commands: Commands,
    mut claimed: Local<bool>,
    gamepads: Query<Entity, With<Gamepad>>,
    unpaired: Query<(Entity, &Protagonist), Without<OnFoot>>,
) {
    if *claimed {
        return;
    }
    let Some(pad) = gamepads.iter().next() else {
        return;
    };
    let Some((entity, _)) = unpaired.iter().find(|(_, p)| p.0 == 1) else {
        return;
    };
    commands
        .entity(entity)
        .insert((OnFoot, Paired::to(DeviceHandle::Gamepad(pad))));
    *claimed = true;
}
