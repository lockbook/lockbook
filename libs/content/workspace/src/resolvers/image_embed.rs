use std::ops::Deref as _;

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Ui, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;

use crate::resolvers::EmbedResolver;
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

impl EmbedResolver for ImageEmbedResolver {
    fn size(&self, url: &str) -> Vec2 {
        // a withheld image of unknown size leaves room to say how to load it
        let withheld = self.images.withheld_in(url, self.file_id);
        let unknown = if withheld { Vec2::new(440., 160.) } else { Vec2::splat(200.) };
        self.images.dims(url).unwrap_or(unknown)
    }

    fn is_loaded(&self, url: &str) -> bool {
        self.images.is_loaded(url)
    }

    fn show(&self, ui: &mut Ui, url: &str, rect: Rect, rounding: CornerRadius) {
        let state = self.images.get_or_load(url, self.file_id, false);
        let image_state = state.lock().unwrap().deref().clone();
        match image_state {
            ImageState::Loading => {
                show_placeholder(ui, rect, Icon::IMAGE, "Loading image...");
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
                show_placeholder(ui, rect, Icon::NO_IMAGE, &message);
            }
            ImageState::Withheld => {
                show_placeholder(ui, rect, Icon::IMAGE, "Click to load image");
            }
        }
    }

    fn is_withheld(&self, url: &str) -> bool {
        self.images.is_withheld(url)
    }

    fn allow(&self, url: &str) {
        self.images.allow(url);
    }

    fn prefetch(&self, url: &str) {
        self.images.get_or_load(url, self.file_id, false);
    }

    fn seq(&self) -> u64 {
        self.images.seq()
    }
}

fn show_placeholder(ui: &mut Ui, rect: Rect, icon: Icon, caption: &str) {
    let theme = ui.ctx().get_lb_theme();
    let color = theme.neutral_fg_secondary();
    // Clip so a tiny thumbnail's icon/caption can't spill over the card or text.
    let painter = ui.painter().with_clip_rect(rect);

    // Caption (e.g. an error message) only where it fits, the icon above it;
    // otherwise the icon alone.
    let captioned = rect.width() >= 160.0 && rect.height() >= 64.0;
    let icon_rect = if captioned { rect.with_max_y(rect.max.y - 28.0) } else { rect };
    let icon_size = (icon_rect.width().min(icon_rect.height()) * 0.6).clamp(10.0, 48.0);
    painter.text(
        icon_rect.center(),
        Align2::CENTER_CENTER,
        icon.icon,
        FontId { size: icon_size, family: egui::FontFamily::Monospace },
        color,
    );
    if captioned {
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, 8.0),
            Align2::CENTER_BOTTOM,
            caption,
            FontId::default(),
            color,
        );
    }
    painter.rect_stroke(
        rect,
        2.,
        Stroke { width: 1., color: theme.neutral_bg_tertiary() },
        egui::epaint::StrokeKind::Inside,
    );
}
