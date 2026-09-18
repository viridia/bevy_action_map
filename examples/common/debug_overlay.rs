//! The F1 panel: every context, whether it is active, and every action in it with its phase and
//! what is stopping it from firing.
//!
//! Nothing here names an action or a context — [`dump`] hands back whatever the running game
//! declared, so this file works unchanged in a different game with different ones. Toggling it is
//! entirely a game's own affair: this module takes no position on what key, button or menu item
//! shows the panel, only on what the panel says once it is showing. A game wires its own action to
//! [`toggle`] — Disasteroids binds `F1` and a pad's Select to its own `ToggleOverlay`; Split
//! Friction and Pong bind `F1` alone, on the working assumption that debugging is a keyboard
//! affair unless a game says otherwise on its own action.
//!
//! A game wanting a line of its own — Disasteroids' debris count, say — draws it in a separate
//! `Text` node rather than asking this module for a hook: the two are unrelated data, and unrelated
//! data does not need to share a string.

use bevy::prelude::*;
use bevy_action_map::inspect::dump;
use bevy_action_map::mapping::mappings;
use bevy_action_map::prelude::*;
use core::fmt::Write;

/// One slot of a row as text: whatever has to be held, then the control itself.
///
/// The same composition the prompt captions use, and for the same reason — a row that dropped the
/// modifier would print `Ctrl+N` as "N", so the panel and the caption would disagree about one
/// binding.
fn slot_label(slot: &BoundSlot) -> String {
    let mut text = String::new();
    for held in &slot.with {
        text.push_str(&held.fallback_label());
        text.push('+');
    }
    text.push_str(&slot.control.fallback_label());
    text
}

/// The panel's own root entity. `pub` so a game with more than one camera can find it and attach
/// its own `UiTargetCamera` — this module spawns onto whatever Bevy treats as the default UI
/// camera, which is the right answer for a single-camera game and the wrong one for a game like
/// Split Friction, where nothing here can know which of several cameras is safe to ride on.
#[derive(Component, Default, Clone)]
pub struct OverlayPanel;

/// Whether the panel is currently drawn.
#[derive(Resource, Default)]
pub struct Showing(pub bool);

pub fn plugin(app: &mut App) {
    app.init_resource::<Showing>();
    app.add_systems(Startup, panel.spawn());
    app.add_systems(Update, redraw);
}

/// Flips the panel on and off. Not an observer itself — call it from whatever your game's own
/// toggle action fires, the same way [`redraw`] never asks what an action is.
pub fn toggle(mut showing: ResMut<Showing>) {
    showing.0 = !showing.0;
}

/// A flex container rather than one `Text`: a game with a handful of live contexts already runs
/// this taller than the window, and `flex_wrap` is what lets a context that would run off the
/// bottom start a new column instead — the panel grows sideways rather than off the screen.
///
/// Backed by translucent black, the same as Split Friction's own join prompt: the text color below
/// is chosen against a dark background, and nothing here controls what a game draws underneath it.
fn panel() -> impl Scene {
    bsn! {
        OverlayPanel
        BackgroundColor({Color::BLACK.with_alpha(0.55)})
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            bottom: Val::Px(8.0),
            padding: {UiRect::all(Val::Px(6.0))},
            flex_direction: FlexDirection::Column,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(24.0),
        }
    }
}

/// One block of the panel's own text — one context, or the rebindable list — as a wrappable flex
/// item. [`redraw`] rebuilds the whole set each time rather than editing them in place: the number
/// of contexts and mappings changes at runtime, and this is a debug panel, not a per-tick budget.
fn block(text: String) -> impl Scene {
    bsn! {
        Text::new(text)
        TextFont { font_size: 13.0_f32 }
        TextColor(Color::srgb(0.6, 0.9, 0.7))
    }
}

fn redraw(world: &mut World) {
    let Some(panel) = world
        .query_filtered::<Entity, With<OverlayPanel>>()
        .iter(world)
        .next()
    else {
        return;
    };

    if let Some(children) = world.get::<Children>(panel) {
        let old: Vec<Entity> = children.iter().collect();
        for child in old {
            world.despawn(child);
        }
    }

    if !world.resource::<Showing>().0 {
        return;
    }

    let mut blocks = Vec::new();

    for context in dump(world).contexts {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{} [{:?} {}]",
            context.path, context.tick, context.priority
        );

        if context.instances.is_empty() {
            out.push_str("  (nobody is carrying this)\n");
        }

        for instance in context.instances {
            let _ = writeln!(
                out,
                "  {} {}",
                if instance.active { "on " } else { "off" },
                instance.entity
            );
            for action in instance.actions {
                // The obstacle is the whole point: an action that is not firing looks identical
                // from a call site whether its context is asleep, something outranked it, or the
                // player simply is not pressing anything.
                let _ = writeln!(
                    out,
                    "    {:<26} {:?} {:?}",
                    action.path, action.state.phase, action.obstacle
                );
            }
        }
        blocks.push(out);
    }

    // What the player would be shown, from the same world. Nothing below names an action: the
    // mapping list is the whole of what a rebinding screen needs, and this is the smallest thing
    // that reads it.
    let mut rebindable = String::from("rebindable\n");
    for mapping in mappings(world) {
        // Both halves of the row are keys with a fallback, so a game that ships a translation
        // catalogue swaps in two lookups here and nothing else changes. An emptied slot prints as a
        // dash rather than being skipped: this is a dump of what the row holds, and a gap between
        // two controls is part of that.
        let bound = mapping
            .slots
            .iter()
            .map(|slot| slot.as_ref().map_or_else(|| "—".to_string(), slot_label))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            rebindable,
            "    {:<22} {:<9} {bound}",
            mapping.key.fallback_label(),
            match mapping.rebind_policy {
                RebindPolicy::Here => "[rebind]",
                RebindPolicy::Fixed => "[fixed]",
            },
        );
    }
    blocks.push(rebindable);

    let children: Vec<Entity> = blocks
        .into_iter()
        .map(|text| {
            world
                .spawn_scene(block(text))
                .expect("a plain Text/TextFont/TextColor scene has no asset dependencies")
                .id()
        })
        .collect();
    world.entity_mut(panel).add_children(&children);
}
