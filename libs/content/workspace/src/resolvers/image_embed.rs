use std::ops::Deref as _;

use egui::os::OperatingSystem;
use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Ui, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;

use crate::file_cache::{FilesExt as _, Reach, web_link};
use crate::resolvers::EmbedResolver;
use crate::style::chrome::phosphor;
use crate::style::{TypeRole, phosphor_font_id};
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::image_cache::{ImageCache, ImageKey, ImageState};

/// The url of the image a wiki embed names (`![[photo.png]]`): its title in
/// brackets, resolved in [`ImageEmbedResolver`], whose note it's relative to.
pub fn wiki_embed_key(title: &str) -> String {
    format!("[[{title}]]")
}

/// The title in a [`wiki_embed_key`].
pub fn wiki_embed_title(url: &str) -> Option<&str> {
    url.strip_prefix("[[")?.strip_suffix("]]")
}

pub struct ImageEmbedResolver {
    images: ImageCache,
    file_id: Uuid,
}

impl ImageEmbedResolver {
    pub fn new(images: ImageCache, file_id: Uuid) -> Self {
        Self { images, file_id }
    }
}

impl ImageEmbedResolver {
    /// The image `url` shows in this note, as a link in it would reach it,
    /// and whether it waits for the reader to ask for it: a file past the
    /// note's scope, which opens different things for different readers,
    /// like a yellow link; or a web image while the reader hasn't let the
    /// app contact linked sites. None for one naming nothing: a red image.
    fn locate(&self, url: &str) -> Option<(ImageKey, bool)> {
        let files = self.images.files();
        let files = files.read().unwrap();
        let (key, reach) = match wiki_embed_title(url) {
            Some(title) => {
                let (id, reach) = files.locate_wikilink(title, self.file_id)?;
                (ImageKey::File(id), reach)
            }
            None => match files.locate_link(url, self.file_id) {
                Some((id, reach)) => (ImageKey::File(id), reach),
                None if web_link(url) => (ImageKey::Web(url.into()), Reach::Scope),
                None => return None,
            },
        };
        let waits = (reach != Reach::Scope || self.images.off_the_web(&key))
            && !self.images.shown_anyway(self.file_id, &key);
        Some((key, waits))
    }

    /// The image `url` shows, unless it waits or names nothing: those never
    /// load.
    fn shown(&self, url: &str) -> Option<ImageKey> {
        self.locate(url)
            .and_then(|(key, waits)| (!waits).then_some(key))
    }
}

/// A placeholder's side, in points, as the layout divides sizes by the
/// screen's scale.
const PLACEHOLDER_PT: f32 = 160.0;

impl EmbedResolver for ImageEmbedResolver {
    fn size(&self, url: &str) -> Vec2 {
        let placeholder = Vec2::splat(PLACEHOLDER_PT * self.images.pixels_per_point());
        self.shown(url)
            .and_then(|key| self.images.dims(&key))
            .unwrap_or(placeholder)
    }

    fn is_loaded(&self, url: &str) -> bool {
        self.shown(url)
            .is_some_and(|key| self.images.is_loaded(&key))
    }

    fn withheld(&self, url: &str) -> bool {
        self.locate(url).is_some_and(|(_, waits)| waits)
    }

    fn load_anyway(&self, url: &str) {
        if let Some((key, _)) = self.locate(url) {
            self.images.show_anyway(self.file_id, &key);
        }
    }

    fn show(&self, ui: &mut Ui, url: &str, rect: Rect, rounding: CornerRadius) {
        let theme = ui.ctx().get_lb_theme();
        let Some((key, waits)) = self.locate(url) else {
            // what's missing; hover says why
            let url = wiki_embed_title(url).unwrap_or(url);
            let name = url.split(['#', '?']).next().unwrap_or(url);
            let name = name
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(name);
            let name = urlencoding::decode(name).map_or(name.into(), |n| n.into_owned());
            show_placeholder(ui, rect, phosphor::IMAGE_BROKEN, &name, Some(theme.fg().red));
            return;
        };
        if waits {
            let touch = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
            let tap = if touch { "Tap" } else { "Click" };
            if self.images.off_the_web(&key) {
                show_hidden(ui, rect, rounding, &format!("{tap} to load"), Hidden::Web);
            } else {
                show_hidden(ui, rect, rounding, &format!("{tap} to show"), Hidden::Reader);
            }
            return;
        }
        let state = self.images.get_or_load(&key, false);
        let image_state = state.lock().unwrap().deref().clone();
        match image_state {
            ImageState::Loading => {
                show_placeholder(ui, rect, phosphor::IMAGE, "Loading image…", None);
            }
            ImageState::Loaded(texture_id) => {
                // Paint only — interaction (open vs. select) is driven by the
                // fragment's `Sense::click` scope via `handle_image_interactions`.
                // No allocation: paint runs for off-screen neighbor rows too,
                // and advancing the layout cursor there displaces siblings
                // laid out after the editor (the mobile toolbar, #4892).
                ui.painter()
                    .add(RectShape::filled(rect, rounding, Color32::WHITE).with_texture(
                        texture_id,
                        Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } },
                    ));
            }
            ImageState::Failed(message) => {
                show_placeholder(ui, rect, phosphor::IMAGE_BROKEN, &message, None);
            }
        }
    }

    fn prefetch(&self, url: &str) {
        if let Some(key) = self.shown(url) {
            self.images.get_or_load(&key, false);
        }
    }

    fn seq(&self) -> u64 {
        self.images.seq()
    }

    fn copy_image(&self, url: &str, _ctx: &egui::Context) {
        if let Some(key) = self.shown(url) {
            self.images.copy_image(&key);
        }
    }
}

/// A quiet box: icon over caption in secondary ink, laid out as a hidden
/// image is ([`paint_icon_caption`]). `dot`
/// marks one that waits or names nothing, in the color its link draws in,
/// as the completion picker marks files past the scope.
fn show_placeholder(
    ui: &mut Ui, rect: Rect, icon: &'static str, caption: &str, dot: Option<Color32>,
) {
    let theme = ui.ctx().get_lb_theme();
    let color = theme.neutral_fg_secondary();
    // Clip so a tiny thumbnail's icon/caption can't spill over the card or text.
    crate::style::paint_doc_border(ui.painter(), rect, 2, &theme);
    let painter = ui.painter().with_clip_rect(rect);

    paint_icon_caption(&painter, rect, icon, caption, color);
    if let Some(dot) = dot {
        let inset = DOT + crate::style::Space::Xs.pts();
        painter.circle_filled(rect.right_top() + Vec2::new(-inset, inset), DOT, dot);
    }
}

/// An icon over its caption, centred together in `rect`; the caption only
/// where it fits.
fn paint_icon_caption(
    painter: &egui::Painter, rect: Rect, icon: &str, caption: &str, ink: Color32,
) {
    let glyph = (rect.width().min(rect.height()) * 0.3).clamp(14.0, 32.0);
    let galley = painter.layout_no_wrap(caption.into(), TypeRole::Mono.font_id(), ink);
    let fits = galley.size().x <= rect.width() - 12.0 && rect.height() >= glyph * 2.4;
    let icon_at = if fits { rect.center() - Vec2::new(0.0, glyph * 0.4) } else { rect.center() };
    painter.text(icon_at, Align2::CENTER_CENTER, icon, phosphor_font_id(glyph), ink);
    if fits {
        let at =
            rect.center() + Vec2::new(-galley.size().x / 2.0, glyph * 0.6 - galley.size().y / 2.0);
        painter.galley(at, galley, ink);
    }
}

/// Radius of the state dot in a placeholder's corner.
const DOT: f32 = 3.0;

/// Why an image is hidden.
#[derive(PartialEq)]
enum Hidden {
    /// A file that differs by reader.
    Reader,
    /// A web image, not fetched until asked.
    Web,
}

/// A hidden image, as a spoiler chip: filled, a glyph, the caption, and for
/// a file that differs by reader, the yellow dot. A web image shows a globe.
fn show_hidden(ui: &mut Ui, rect: Rect, rounding: CornerRadius, caption: &str, why: Hidden) {
    let theme = ui.ctx().get_lb_theme();
    let ink = theme.neutral_fg_secondary();
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, rounding, theme.neutral_bg_secondary());
    paint_icon_caption(
        &painter,
        rect,
        match why {
            Hidden::Reader => crate::style::phosphor::EYE_SLASH,
            Hidden::Web => crate::style::phosphor::GLOBE,
        },
        caption,
        ink,
    );
    if why == Hidden::Reader {
        let inset = DOT + crate::style::Space::Xs.pts();
        painter.circle_filled(rect.right_top() + Vec2::new(-inset, inset), DOT, theme.fg().yellow);
    }
}
