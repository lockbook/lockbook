//! Provider brand glyphs for the picker: bundled monochrome SVGs (LobeHub
//! icon set, MIT) rasterized once into alpha masks and tinted at draw time,
//! so they follow the theme like text. Similar brands in this space (Groq
//! vs Grok) make a wordlist-only menu genuinely confusable.

use std::collections::HashMap;

use egui::load::SizedTexture;
use egui::{Color32, ColorImage, TextureHandle, TextureOptions};

/// Keyed by provider name (= template name). `generic` is the fallback mark
/// for names we don't recognize: hand-made files and `custom`.
const GLYPHS: &[(&str, &[u8])] = &[
    ("openai", include_bytes!("icons/openai.svg")),
    ("anthropic", include_bytes!("icons/anthropic.svg")),
    ("google", include_bytes!("icons/google.svg")),
    ("xai", include_bytes!("icons/xai.svg")),
    ("openrouter", include_bytes!("icons/openrouter.svg")),
    ("groq", include_bytes!("icons/groq.svg")),
    ("cerebras", include_bytes!("icons/cerebras.svg")),
    ("together", include_bytes!("icons/together.svg")),
    ("ollama", include_bytes!("icons/ollama.svg")),
    ("generic", include_bytes!("icons/generic.svg")),
];

/// Lazily-rasterized glyph textures, one per (glyph, physical size) actually
/// drawn — pixel-exact at every size and DPI, since a mask stretched past its
/// raster size goes visibly soft. Held for the tab's lifetime — handles keep
/// their textures alive; the size set is tiny (a few draw sizes × DPI).
#[derive(Default)]
pub struct Glyphs {
    textures: HashMap<(&'static str, u32), TextureHandle>,
}

impl Glyphs {
    /// The glyph for a provider name, else the generic mark, rasterized for
    /// a `logical_px` draw size at the context's current DPI. Draw tinted
    /// (the mask is white) at the row's text color.
    pub fn get(&mut self, ctx: &egui::Context, name: &str, logical_px: f32) -> SizedTexture {
        let (key, bytes) = GLYPHS
            .iter()
            .find(|(n, _)| *n == name)
            .or_else(|| GLYPHS.iter().find(|(n, _)| *n == "generic"))
            .expect("generic glyph is bundled");
        let physical = (logical_px * ctx.pixels_per_point()).ceil().max(1.0) as u32;
        let handle = self.textures.entry((key, physical)).or_insert_with(|| {
            ctx.load_texture(
                format!("chat_glyph_{key}_{physical}"),
                rasterize(bytes, physical),
                TextureOptions::LINEAR,
            )
        });
        SizedTexture::from_handle(handle)
    }
}

/// White-with-alpha mask from the SVG's coverage; color comes from the tint.
fn rasterize(bytes: &[u8], px: u32) -> ColorImage {
    let fallible = || -> Option<ColorImage> {
        let tree =
            resvg::usvg::Tree::from_data(bytes, &Default::default(), &Default::default()).ok()?;
        let size = tree.size();
        let scale = px as f32 / size.width().max(size.height());
        let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let pixels = pixmap
            .pixels()
            .iter()
            .map(|px| Color32::from_white_alpha(px.alpha()))
            .collect();
        Some(ColorImage::new([px as usize; 2], pixels))
    };
    // A bundled asset that doesn't rasterize is a build bug; render blank
    // rather than panicking a release build over an icon.
    fallible().unwrap_or_else(|| ColorImage::filled([px as usize; 2], Color32::TRANSPARENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bundled SVG parses and rasterizes to a non-empty mask.
    #[test]
    fn all_glyphs_rasterize() {
        for (name, bytes) in GLYPHS {
            let image = rasterize(bytes, 32);
            let coverage = image.pixels.iter().filter(|px| px.a() > 0).count();
            assert!(coverage > 0, "glyph '{name}' rasterized to nothing");
        }
    }
}
