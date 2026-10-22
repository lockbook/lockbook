use std::ops::Deref as _;

use egui::os::OperatingSystem;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Ui, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;

use crate::resolvers::EmbedResolver;
use crate::style::TypeRole;
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::image_cache::{ImageCache, ImageState};

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
    /// An image that waits (yellow) or names nothing (red) never loads.
    fn held(&self, url: &str) -> bool {
        self.images.withheld(url, self.file_id) || self.images.broken(url, self.file_id)
    }
}

/// A placeholder's side, in points, as the layout divides sizes by the
/// screen's scale.
const PLACEHOLDER_PT: f32 = 160.0;

impl EmbedResolver for ImageEmbedResolver {
    fn size(&self, url: &str) -> Vec2 {
        let placeholder = Vec2::splat(PLACEHOLDER_PT * self.images.pixels_per_point());
        if self.held(url) {
            return placeholder;
        }
        self.images.dims(url).unwrap_or(placeholder)
    }

    fn is_loaded(&self, url: &str) -> bool {
        !self.held(url) && self.images.is_loaded(url)
    }

    fn withheld(&self, url: &str) -> bool {
        self.images.withheld(url, self.file_id)
    }

    fn load_anyway(&self, url: &str) {
        self.images.show_anyway(self.file_id, url);
    }

    fn show(&self, ui: &mut Ui, url: &str, rect: Rect, rounding: CornerRadius) {
        let theme = ui.ctx().get_lb_theme();
        if self.images.withheld(url, self.file_id) {
            let touch = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
            let tap = if touch { "Tap" } else { "Click" };
            if self.images.off_the_web(url) {
                show_hidden(ui, rect, rounding, &format!("{tap} to load"), Hidden::Web);
            } else {
                show_hidden(ui, rect, rounding, &format!("{tap} to show"), Hidden::Reader);
            }
            return;
        }
        if self.images.broken(url, self.file_id) {
            // what's missing; hover says why
            let name = url.split(['#', '?']).next().unwrap_or(url);
            let name = name
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(name);
            let name = urlencoding::decode(name).map_or(name.into(), |n| n.into_owned());
            show_placeholder(ui, rect, Icon::NO_IMAGE, &name, Some(theme.fg().red));
            return;
        }
        let state = self.images.get_or_load(url, self.file_id, false);
        let image_state = state.lock().unwrap().deref().clone();
        match image_state {
            ImageState::Loading => {
                show_placeholder(ui, rect, Icon::IMAGE, "Loading image...", None);
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
                show_placeholder(ui, rect, Icon::NO_IMAGE, &message, None);
            }
        }
    }

    fn prefetch(&self, url: &str) {
        if !self.held(url) {
            self.images.get_or_load(url, self.file_id, false);
        }
    }

    fn seq(&self) -> u64 {
        self.images.seq()
    }
}

/// A quiet box: hairline frame, icon and caption in secondary ink. `dot`
/// marks one that waits or names nothing, in the color its link draws in,
/// as the completion picker marks files past the scope.
fn show_placeholder(ui: &mut Ui, rect: Rect, icon: Icon, caption: &str, dot: Option<Color32>) {
    let theme = ui.ctx().get_lb_theme();
    let color = theme.neutral_fg_secondary();
    // Clip so a tiny thumbnail's icon/caption can't spill over the card or text.
    let painter = ui.painter().with_clip_rect(rect);

    let icon_size = (rect.width().min(rect.height()) * 0.6).clamp(10.0, 48.0);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.icon,
        FontId { size: icon_size, family: egui::FontFamily::Monospace },
        color,
    );
    // Caption only where it fits; otherwise the icon alone.
    let font = FontId::proportional(TypeRole::Mono.size());
    let fits = ui.fonts(|f| {
        f.layout_no_wrap(caption.into(), font.clone(), color)
            .size()
            .x
    }) <= rect.width() - 12.0;
    if fits && rect.height() >= 64.0 {
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, 8.0),
            Align2::CENTER_BOTTOM,
            caption,
            font,
            color,
        );
    }
    painter.rect_stroke(
        rect,
        2.,
        Stroke { width: 1., color: theme.neutral_bg_tertiary() },
        egui::epaint::StrokeKind::Inside,
    );
    if let Some(dot) = dot {
        let inset = DOT + crate::style::Space::Xs.pts();
        painter.circle_filled(rect.right_top() + Vec2::new(-inset, inset), DOT, dot);
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
    let glyph = (rect.width().min(rect.height()) * 0.3).clamp(14.0, 32.0);
    painter.text(
        rect.center() - Vec2::new(0.0, glyph * 0.4),
        Align2::CENTER_CENTER,
        match why {
            Hidden::Reader => crate::style::phosphor::EYE_SLASH,
            Hidden::Web => crate::style::phosphor::GLOBE,
        },
        crate::style::phosphor_font_id(glyph),
        ink,
    );
    painter.text(
        rect.center() + Vec2::new(0.0, glyph * 0.6),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(TypeRole::Mono.size()),
        ink,
    );
    if why == Hidden::Reader {
        let inset = DOT + crate::style::Space::Xs.pts();
        painter.circle_filled(rect.right_top() + Vec2::new(-inset, inset), DOT, theme.fg().yellow);
    }
}
