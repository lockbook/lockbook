use std::ops::Deref as _;

use egui::{
    self, Align2, Color32, CursorIcon, FontId, Id, OpenUrl, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};
use epaint::RectShape;

use crate::tab::markdown_editor::{
    theme::Theme,
    widget::inline::image::cache::{ImageCache, ImageState},
};

/// Initialize with an EmbedResolver to draw embedded content based on a URL.
/// The width for the embed will determined by the editor as the editor's width
/// minus the indentation for Markdown blocks that the embed is nested in.
trait EmbedResolver {
    /// How tall will the embed be for the given `url` with `max_size` space
    /// available? Used to place subsequent elements in the document; supports
    /// only rendering what's visible in the scroll view. The result is cached
    /// until the document or window size changes. The result must not exceed
    /// `max_size.y`.
    fn height(&self, url: &str, max_size: Vec2) -> f32;

    /// Show the embed. Just draw your embed in the rect; the Ui's cursor will
    /// not be in any particular state and any effect on the Ui's cursor will be
    /// ignored.
    fn show(&self, url: &str, rect: Rect, theme: &Theme, ui: &mut Ui);

    /// When did the state of the resolver last change in a way that affects how
    /// embeds should be layed out? Signals that layout should change e.g. when
    /// an image has completed loading. The particular value doesn't matter as
    /// long as it always goes up when the layout should change.
    fn last_modified(&self) -> u64;
}

impl EmbedResolver for ImageCache {
    fn height(&self, url: &str, max_size: Vec2) -> f32 {
        if let Some(image_state) = self.map.get(url) {
            let image_state = image_state.lock().unwrap().deref().clone();
            match image_state {
                ImageState::Loading => image_size(Vec2::splat(200.), max_size).y,
                ImageState::Loaded(texture_id) => {
                    let [image_width, image_height] =
                        self.ctx.tex_manager().read().meta(texture_id).unwrap().size;
                    let size = image_size(Vec2::new(image_width as _, image_height as _), max_size);

                    size.y
                }
                ImageState::Failed(_) => image_size(Vec2::splat(200.), max_size).y,
            }
        } else {
            0.
        }
    }

    fn show(&self, url: &str, rect: Rect, theme: &Theme, ui: &mut Ui) {
        let top_left = rect.left_top();
        let width = rect.size().x;

        if let Some(image_state) = self.map.get(url) {
            let image_state = image_state.lock().unwrap().deref().clone();
            match image_state {
                ImageState::Loading => {
                    let icon = "\u{e410}";
                    let caption = "Loading image...";

                    let size = image_size(Vec2::splat(200.), rect.size());
                    let rect = Rect::from_min_size(top_left, Vec2::new(width, size.y));

                    ui.allocate_ui_at_rect(rect, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId { size: 48.0, family: egui::FontFamily::Monospace },
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().text(
                            rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                            Align2::CENTER_BOTTOM,
                            caption,
                            FontId::default(),
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            2.,
                            Stroke { width: 1., color: theme.bg().neutral_tertiary },
                        );
                    });
                }
                ImageState::Loaded(texture_id) => {
                    let [image_width, image_height] =
                        self.ctx.tex_manager().read().meta(texture_id).unwrap().size;

                    let size =
                        image_size(Vec2::new(image_width as _, image_height as _), rect.size());
                    let padding = (width - size.x) / 2.0;
                    let image_top_left = top_left + Vec2::new(padding, 0.);
                    let rect = Rect::from_min_size(image_top_left, size);

                    let resp = ui.interact(rect, Id::new(texture_id), Sense::click());
                    if resp.hovered() {
                        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        ui.output_mut(|o| o.open_url = Some(OpenUrl::new_tab(url)));
                    }

                    ui.allocate_ui_at_rect(rect, |ui| {
                        ui.painter().add(RectShape {
                            rect,
                            rounding: (2.).into(),
                            fill: Color32::WHITE,
                            stroke: Stroke::NONE,
                            blur_width: 0.0,
                            fill_texture_id: texture_id,
                            uv: Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } },
                        });
                    });
                }
                ImageState::Failed(message) => {
                    let icon = "\u{f116}";
                    let caption = format!("Could not show image: {message}");

                    let size = image_size(Vec2::splat(200.), rect.size());
                    let rect = Rect::from_min_size(top_left, Vec2::new(width, size.y));

                    ui.allocate_ui_at_rect(rect, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId { size: 48.0, family: egui::FontFamily::Monospace },
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().text(
                            rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                            Align2::CENTER_BOTTOM,
                            caption,
                            FontId::default(),
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            2.,
                            Stroke { width: 1., color: theme.bg().neutral_tertiary },
                        );
                    });
                }
            }
        }
    }

    fn last_modified(&self) -> u64 {
        self.last_modified.lock().unwrap().deref().clone()
    }
}

fn image_size(texture_size: Vec2, max_size: Vec2) -> Vec2 {
    // make sure images can be viewed in full by capping their height and width to the viewport
    // todo: though great on mobile, images look too big on desktop

    let width_capped_size = Vec2::new(max_size.x, texture_size.y * max_size.x / texture_size.x);
    let height_capped_size = Vec2::new(texture_size.x * max_size.y / texture_size.y, max_size.y);

    if width_capped_size.length() < height_capped_size.length() {
        width_capped_size
    } else {
        height_capped_size
    }
}
