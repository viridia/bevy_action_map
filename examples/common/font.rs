//! A default font with the characters the examples draw.
//!
//! Bevy's built-in font covers printable ASCII and nothing else, so `—`, `×` and an accented letter
//! all draw as boxes. This replaces it with FiraSans, the face `bevy_feathers` ships, at the default
//! font's own id: every span that names no `font` of its own draws in that one, so no span changes.
//!
//! This keeps the example screens legible and is not a technique to copy. Bevy 0.20.0-rc.1 has no
//! supported way to set an app-wide font (bevyengine/bevy#25842, answered on `main` by
//! bevyengine/bevy#25847), so this overwrites the slot the `default_font` feature fills. A game
//! should set its font the way its own UI theme does.

use bevy::asset::AssetId;
use bevy::prelude::*;

/// Replaces Bevy's default font. Add it after `DefaultPlugins`, whose text plugin inserts the font
/// this one replaces.
///
/// At build time rather than in a startup system, because text layout registers each font id once,
/// the first time it sees it, and never looks at that id's contents again.
pub fn plugin(app: &mut App) {
    let font = Font::from_bytes(include_bytes!("../../assets/fonts/FiraSans-Regular.ttf").to_vec());
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(AssetId::default(), font)
        .expect("the default font's id is a UUID, which never fails to insert");
}
