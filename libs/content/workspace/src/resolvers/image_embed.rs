use std::ops::Deref as _;

use egui::{
    Align2, Color32, CursorIcon, FontId, Id, OpenUrl, Pos2, Rect, Sense, Stroke, Ui, UiBuilder,
    Vec2,
};
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
        self.images.dims(url).unwrap_or(Vec2::splat(200.))
    }

    fn show(&self, ui: &mut Ui, url: &str, rect: Rect) {
        let state = self.images.get_or_load(url, self.file_id, false);
        let image_state = state.lock().unwrap().deref().clone();
        match image_state {
            ImageState::Loading => {
                show_placeholder(ui, rect, Icon::IMAGE, "Loading image...");
            }
            ImageState::Loaded(texture_id) => {
                let resp = ui.interact(rect, Id::new(texture_id), Sense::click());
                if resp.hovered() {
                    ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    ui.ctx()
                        .open_url(OpenUrl { url: url.into(), new_tab: true });
                }

                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ui.painter().add(
                        RectShape::filled(rect, 2.0_f32, Color32::WHITE).with_texture(
                            texture_id,
                            Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } },
                        ),
                    );
                });
            }
            ImageState::Failed(message) => {
                show_placeholder(ui, rect, Icon::NO_IMAGE, &message);
            }
        }
    }

    fn warm(&self, url: &str) {
        self.images.get_or_load(url, self.file_id, false);
    }

    fn seq(&self) -> u64 {
        self.images.seq()
    }
}

fn show_placeholder(ui: &mut Ui, rect: Rect, icon: Icon, caption: &str) {
    let theme = ui.ctx().get_lb_theme();
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        let rect = ui.max_rect();
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon.icon,
            FontId { size: 48.0, family: egui::FontFamily::Monospace },
            theme.neutral_fg_secondary(),
        );
        ui.painter().text(
            rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
            Align2::CENTER_BOTTOM,
            caption,
            FontId::default(),
            theme.neutral_fg_secondary(),
        );
        ui.painter().rect_stroke(
            rect,
            2.,
            Stroke { width: 1., color: theme.neutral_bg_tertiary() },
            egui::epaint::StrokeKind::Inside,
        );
    });
}
