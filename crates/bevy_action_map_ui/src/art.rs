//! Where the art for an icon prompt comes from.

use bevy_action_map::prelude::{ControlOrigin, Glyph, GlyphTier};
use bevy_asset::AssetPath;
use bevy_ecs::resource::Resource;

/// Which of the two ways a prompt draws its icon.
///
/// An art set may ship each control at more than one size, and a provider uses this to pick the one
/// nearest the size it will be drawn at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconLayout {
    /// Inside a line of text, sized from the text's font.
    Inline,
    /// In a UI node of its own, filling the node's height.
    Block,
}

/// One source of prompt art: given a glyph and how it will be drawn, the asset to load for it, or
/// `None` where this source has no art for that glyph.
type Provider = Box<dyn Fn(&Glyph, IconLayout) -> Option<AssetPath<'static>> + Send + Sync>;

/// Where icon prompts find their art: an ordered list of providers, asked in turn until one answers.
///
/// This crate ships no art. Each art set your game uses is a provider, and you add them in order of
/// preference, so that a platform's own art for its controller can take priority over a general
/// set that also covers the keyboard:
///
/// ```ignore
/// fn platform_art(app: &mut App) {
///     app.world_mut()
///         .get_resource_or_init::<PromptArt>()
///         .push(|glyph, _layout| match glyph {
///             Glyph::External(path) => Some(AssetPath::from(path.clone())),
///             _ => None,
///         });
/// }
/// ```
///
/// A control no provider has art for is drawn as text, and so is every control when the list is
/// empty.
///
/// Whether a control has art also decides which art it gets. A pad of a brand your art does not
/// cover falls back to generic pad art, so which brands the providers cover between them changes
/// which glyph a prompt asks for.
#[derive(Resource, Default)]
pub struct PromptArt(Vec<Provider>);

impl PromptArt {
    /// Adds a provider, asked after every provider added before it.
    pub fn push(
        &mut self,
        provider: impl Fn(&Glyph, IconLayout) -> Option<AssetPath<'static>> + Send + Sync + 'static,
    ) {
        self.0.push(Box::new(provider));
    }

    /// The art to load for `glyph`, from the first provider with any.
    pub fn path(&self, glyph: &Glyph, layout: IconLayout) -> Option<AssetPath<'static>> {
        self.0.iter().find_map(|provider| provider(glyph, layout))
    }

    /// Whether any provider has art for `origin` in `tier`.
    ///
    /// This is the coverage question [`resolve_glyph`](bevy_action_map::prelude::resolve_glyph)
    /// asks, so a closure calling it is what to pass there.
    pub fn has_art(&self, tier: GlyphTier, origin: &ControlOrigin, layout: IconLayout) -> bool {
        self.path(&Glyph::Own(tier, origin.clone()), layout)
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn external(path: &str) -> Glyph {
        Glyph::External(path.into())
    }

    #[test]
    fn the_first_provider_to_answer_wins() {
        let mut art = PromptArt::default();
        art.push(|glyph, _| match glyph {
            Glyph::External(path) if path == "pad" => Some("first/pad.png".into()),
            _ => None,
        });
        art.push(|_, _| Some("second/any.png".into()));

        assert_eq!(
            art.path(&external("pad"), IconLayout::Block),
            Some("first/pad.png".into())
        );
        assert_eq!(
            art.path(&external("key"), IconLayout::Block),
            Some("second/any.png".into())
        );
    }

    #[test]
    fn an_empty_list_has_no_art() {
        let art = PromptArt::default();
        let origin = ControlOrigin::Foreign {
            name: String::from("key"),
            label: String::from("Key"),
            family: None,
            class: None,
            glyph: None,
        };

        assert_eq!(art.path(&external("pad"), IconLayout::Inline), None);
        assert!(!art.has_art(GlyphTier::KeyboardMouse, &origin, IconLayout::Inline));
    }

    #[test]
    fn coverage_is_asked_of_every_provider() {
        let mut art = PromptArt::default();
        art.push(|_, _| None);
        art.push(|glyph, layout| match glyph {
            Glyph::Own(..) if layout == IconLayout::Inline => Some("own.png".into()),
            _ => None,
        });
        let origin = ControlOrigin::Foreign {
            name: String::from("key"),
            label: String::from("Key"),
            family: None,
            class: None,
            glyph: None,
        };

        assert!(art.has_art(GlyphTier::KeyboardMouse, &origin, IconLayout::Inline));
        assert!(!art.has_art(GlyphTier::KeyboardMouse, &origin, IconLayout::Block));
    }
}
