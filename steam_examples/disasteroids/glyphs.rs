//! Steam's own glyphs, loaded from inside the Steam client through an asset source of their own.
//!
//! Steam answers a glyph lookup with an absolute path into its install (S17), which no asset source
//! reaches. The `steam://` source here reads exactly one directory, and refuses anything that does
//! not resolve inside it, rather than trusting a path an SDK handed over.

use std::path::{Path, PathBuf};

use bevy::asset::AssetPath;
use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader, VecReader,
};
use bevy::prelude::*;

use crate::common::prompt_ui::ExternalArt;

const SOURCE: &str = "steam";

/// The glyphs come in three themes, and Steam's answer names one of them without being asked. This
/// one draws a control in white, which reads on the game's black background; `dark` draws it in
/// black, and an outline-only glyph such as View vanishes against it.
const THEME: &str = "light";

/// Where the Steam client keeps its glyphs, one directory per theme.
///
/// One per platform Steam runs on, and only macOS has been measured.
#[cfg(target_os = "macos")]
fn glyph_dir() -> Option<PathBuf> {
    std::env::home_dir().map(|home| {
        home.join(
            "Library/Application Support/Steam/Steam.AppBundle/Steam/Contents/MacOS/\
             controller_base/images/api",
        )
    })
}

/// Registers the `steam://` source and tells the prompts how to reach it. Ahead of `AssetPlugin`,
/// which builds its sources once.
///
/// Without a Steam install to read, neither happens, and the pad's prompts fall back to Steam's own
/// words for its controls.
pub fn plugin(app: &mut App) {
    let Some(root) = glyph_dir().and_then(|dir| dir.canonicalize().ok()) else {
        warn!("Steam's glyphs are not installed where expected; pad prompts will be text");
        return;
    };
    app.register_asset_source(
        AssetSourceId::from(SOURCE),
        AssetSourceBuilder::new(move || Box::new(Confined(root.clone()))),
    );
    app.insert_resource(ExternalArt(art));
}

/// The asset path for one of Steam's glyphs, in the theme chosen above and at the size the prompt
/// draws at.
///
/// Steam always answers with the 128-pixel art, `<theme>/<name>_md.png`. An inline prompt takes the
/// 32-pixel `_sm` beside it, since an inline image draws at its own size. Only the file's name is
/// kept, relative to the source's root: the asset server refuses an absolute path from any source.
/// A path in any other shape is not loaded.
fn art(path: &str, block: bool) -> Option<AssetPath<'static>> {
    let stem = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix("_md.png"))?;
    let size = if block { "md" } else { "sm" };
    let path = Path::new(THEME).join(format!("{stem}_{size}.png"));
    Some(AssetPath::from_path_buf(path).with_source(SOURCE))
}

/// Reads files under one directory, and nothing outside it.
struct Confined(PathBuf);

impl Confined {
    /// The file `path` names, once it is known to be inside the root. Resolved before the check, so
    /// neither `..` nor a symlink can reach out of it.
    fn confine(&self, path: &Path) -> Result<PathBuf, AssetReaderError> {
        let full = self.0.join(path);
        full.canonicalize()
            .ok()
            .filter(|full| full.starts_with(&self.0))
            .ok_or(AssetReaderError::NotFound(full))
    }
}

impl AssetReader for Confined {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let bytes = std::fs::read(self.confine(path)?)?;
        Ok(VecReader::new(bytes))
    }

    // Steam ships no `.meta` files, and none may be written into its install.
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(self.confine(path)?.is_dir())
    }
}
