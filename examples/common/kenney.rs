//! Kenney's input prompts, under `assets/input_prompts/`, as a provider of prompt art.
//!
//! Every tier the crate resolves against has a directory of its own, and the art is drawn the same
//! for both layouts, each scaling it to its own height.

use std::collections::HashSet;

use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy_action_map::device::GamepadBrand;
use bevy_action_map::prelude::*;
use bevy_action_map_ui::PromptArt;

/// Adds Kenney's art to the prompts, asked after every provider added before this plugin.
pub fn plugin(app: &mut App) {
    let manifest = manifest();
    app.world_mut()
        .get_resource_or_init::<PromptArt>()
        .push(move |glyph, _| icon_path(glyph, &manifest));
}

/// Which `<tier>/<control>` pairs have art, parsed from the manifest
/// `scripts/import_input_prompts.py` writes, so resolution never opens a file to discover one is
/// missing.
fn manifest() -> HashSet<String> {
    const MANIFEST: &str = include_str!("../../assets/input_prompts/manifest.txt");
    MANIFEST
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The path segment a tier's art is filed under.
fn tier_str(tier: GlyphTier) -> &'static str {
    match tier {
        GlyphTier::KeyboardMouse => "keyboard_mouse",
        GlyphTier::Gamepad(GamepadBrand::Xbox) => "xbox",
        GlyphTier::Gamepad(GamepadBrand::PlayStation) => "playstation",
        GlyphTier::Gamepad(GamepadBrand::Nintendo) => "nintendo",
        GlyphTier::Gamepad(GamepadBrand::Generic) => "generic",
    }
}

/// Where a glyph's art lives, or `None` where the manifest has none.
///
/// A Mac takes `macos/` first where it has an entry, for the keys it labels differently: Option
/// for Alt, and Command for Super.
fn icon_path(glyph: &Glyph, manifest: &HashSet<String>) -> Option<AssetPath<'static>> {
    let Glyph::Own(tier, origin) = glyph else {
        return None;
    };
    let name = origin.name();
    let mac = format!("macos/{name}");
    let key = if cfg!(target_os = "macos")
        && *tier == GlyphTier::KeyboardMouse
        && manifest.contains(&mac)
    {
        mac
    } else {
        format!("{}/{name}", tier_str(*tier))
    };
    manifest
        .contains(&key)
        .then(|| format!("input_prompts/{key}.png").into())
}
