//! A debug overlay showing what the input layer thinks is happening.
//!
//! Press `F1` (or Select on a pad) to toggle it. It lists every context, whether it is active, and
//! every action in it with its phase and — the useful part — what is stopping it firing when it is
//! not.
//!
//! Nothing here names an action or a context. `dump` hands back whatever has been declared, so this
//! file would work unchanged in a different game with different actions.

use bevy::prelude::*;
use bevy_action_map::inspect::dump;
use bevy_action_map::prelude::*;
use bevy_action_map::rebind::mappings;
use core::fmt::Write;

use crate::actions::ToggleOverlay;

#[derive(Component, Default, Clone)]
struct OverlayText;

#[derive(Resource, Default)]
pub(crate) struct Showing(bool);

pub fn plugin(app: &mut App) {
    app.init_resource::<Showing>();
    app.add_systems(Startup, panel.spawn());
    // Exclusive, because reading contexts whose types are not known here means asking the world to
    // build the queries. It is a debug overlay; it can have the world for a moment.
    app.add_systems(Update, redraw);
}

/// Flips the overlay on and off.
///
/// Attached to the shell context's entity, so it hears the action wherever that lives.
pub(crate) fn toggle(_: On<Fired<ToggleOverlay>>, mut showing: ResMut<Showing>) {
    showing.0 = !showing.0;
}

fn panel() -> impl Scene {
    bsn! {
        OverlayText
        Text::new("")
        TextFont { font_size: 13.0_f32 }
        TextColor(Color::srgb(0.6, 0.9, 0.7))
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
        }
    }
}

fn redraw(world: &mut World) {
    if !world.resource::<Showing>().0 {
        set_text(world, String::new());
        return;
    }

    let mut out = String::new();
    for context in dump(world).contexts {
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
    }

    // What the player would be shown, from the same world. Nothing below names an action: the
    // mapping list is the whole of what a rebinding screen needs, and this is the smallest thing
    // that reads it.
    out.push_str("\nrebindable\n");
    for mapping in mappings(world) {
        // Both halves of the row are keys with a fallback, so a game that ships a translation
        // catalogue swaps in two lookups here and nothing else changes.
        //
        // A mapping holds an ordered *list*, so the second half is every control in it — this is a
        // read-only dump and there is no reason to hide the secondary. A settings screen draws
        // them as separate cells; here they are joined, and joining is the app's decision rather
        // than the crate's.
        let bound = mapping
            .slots
            .iter()
            .map(|control| control.fallback_label())
            .collect::<Vec<_>>()
            .join(", ");
        // Empty slots the player could still fill, so the dump says how wide the row is rather
        // than only what is in it.
        let room = match mapping.capacity.slots() {
            Some(slots) if slots > mapping.slots.len() => {
                format!("  (+{} free)", slots - mapping.slots.len())
            }
            Some(_) => String::new(),
            None => "  (+ more)".into(),
        };
        // Whether the row is a button or a label on the real screen. Everything is listed; only
        // some of it is changeable, which is what `mappable` declares and what this column shows.
        let _ = writeln!(
            out,
            "    {:<22} {:<9} {bound}{room}",
            mapping.key.fallback_label(),
            match mapping.rebinding {
                Rebinding::Here => "[rebind]",
                Rebinding::Fixed => "[fixed]",
            },
        );
    }

    set_text(world, out);
}

fn set_text(world: &mut World, text: String) {
    let panels: Vec<Entity> = world
        .query_filtered::<Entity, With<OverlayText>>()
        .iter(world)
        .collect();
    for panel in panels {
        if let Some(mut target) = world.get_mut::<Text>(panel) {
            target.0 = text.clone();
        }
    }
}
