use indexmap::IndexMap;
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::{Buffer, u_transform_to_bezier};
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Element, Image, WeakPathPressures};
use resvg::usvg::{NonZeroRect, Transform};

use crate::tab::svg_editor::element::BoundedElement;
use crate::tab::svg_editor::history::History;
use crate::tab::svg_editor::tools::selection::SelectedElement;
use crate::tab::{CanvasPasteContent, ClipContent, ExtendedInput as _};

use super::element::PromoteBufferWeakImages;
use super::util::transform_rect;
use super::viewport::get_rect_identity_transform;
use super::{InsertElement, SVGEditor, Tool};

impl SVGEditor {
    pub fn handle_clip_input(&mut self, ui: &mut egui::Ui) {
        if self.read_only {
            return;
        }
        let mut svg_pastes = Vec::new();
        for custom_event in ui.ctx().read_events() {
            let (content, position) = match custom_event {
                crate::Event::CanvasPaste { content: CanvasPasteContent::Svg(text), position } => {
                    svg_pastes.push((text, Some(position)));
                    continue;
                }
                crate::Event::CanvasPaste {
                    content: CanvasPasteContent::Image(data),
                    position,
                } => (vec![ClipContent::Image(data)], Some(position)),
                crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
                    (content, None)
                }
                _ => continue,
            };
            for clip in content {
                let (name, data) = match clip {
                    ClipContent::Image(data) => (None, data),
                    ClipContent::NamedImage { name, data } => (Some(name), data),
                    ClipContent::Files(..) => unimplemented!(), // todo: support file drop & paste
                };
                // Validate before importing so unsupported clipboard data cannot
                // panic or leave an orphan attachment in the account.
                let Ok(img) = image::load_from_memory(&data) else { continue };
                let Ok(file) =
                    crate::tab::import_image(&self.lb, self.open_file, name.as_deref(), &data)
                else {
                    continue;
                };

                let paste_pos = position.unwrap_or_else(|| {
                    ui.input(|r| {
                        r.pointer.hover_pos().unwrap_or(
                            self.input_ctx
                                .last_touch
                                .unwrap_or(ui.available_rect_before_wrap().center()),
                        )
                    })
                });

                self.insert_image(
                    ui,
                    data,
                    egui::vec2(img.width() as f32, img.height() as f32),
                    file.id,
                    paste_pos,
                );
            }
        }
        ui.input(|r| {
            for event in &r.events {
                if let egui::Event::Paste(payload) = event {
                    // The SVG parser also accepts XML declarations, comments and
                    // leading whitespace, as commonly provided by other apps.
                    if payload.contains("<svg") {
                        svg_pastes.push((payload.clone(), None));
                    }
                }
            }
            for (payload, position) in svg_pastes {
                let mut pasted_buffer = Buffer::new(payload.trim_start_matches('\u{feff}'));

                pasted_buffer
                    .promote_weak_images(self.viewport_settings.master_transform, &self.lb);

                if !pasted_buffer.elements.is_empty() || !pasted_buffer.weak_images.is_empty() {
                    let mut container = egui::Rect::NOTHING;
                    for el in pasted_buffer.elements.iter() {
                        let child = el.1.bounding_box();

                        container.min.x = container.min.x.min(child.min.x);
                        container.min.y = container.min.y.min(child.min.y);

                        container.max.x = container.max.x.max(child.max.x);
                        container.max.y = container.max.y.max(child.max.y);
                    }

                    let paste_pos = position.unwrap_or_else(|| {
                        r.pointer.hover_pos().unwrap_or(
                            self.input_ctx
                                .last_touch
                                .unwrap_or(ui.available_rect_before_wrap().center()),
                        )
                    });
                    let delta =
                        paste_pos - container.center() * self.viewport_settings.master_transform.sx;

                    let transform = Transform::identity()
                        .post_scale(
                            self.viewport_settings.master_transform.sx,
                            self.viewport_settings.master_transform.sy,
                        )
                        .post_translate(delta.x, delta.y);

                    let new_ids = duplicate_elements(
                        pasted_buffer.elements,
                        pasted_buffer.weak_path_pressures,
                        &mut self.buffer,
                        &mut self.history,
                        Some(transform),
                    );

                    self.toolbar.active_tool = Tool::Selection;

                    self.toolbar.selection.selected_elements = new_ids
                        .iter()
                        .map(|&id| SelectedElement { id, transform: Default::default() })
                        .collect();
                }
            }
        });
    }

    fn insert_image(
        &mut self, ui: &egui::Ui, data: Vec<u8>, image_size: egui::Vec2, href: Uuid,
        paste_pos: egui::Pos2,
    ) {
        let img_rect = egui::Rect::from_min_size(paste_pos, egui::vec2(image_size.x, image_size.y));

        let transform = if ui.available_rect_before_wrap().width() < img_rect.width() * 2.0
            || ui.available_rect_before_wrap().height() < img_rect.height() * 2.0
        {
            get_rect_identity_transform(ui.available_rect_before_wrap(), img_rect, 0.5, paste_pos)
        } else {
            Some(Transform::identity().post_translate(
                paste_pos.x - img_rect.center().x,
                paste_pos.y - img_rect.center().y,
            ))
        }
        .unwrap_or_default();

        let fitted_img_rect = transform_rect(img_rect, transform);

        let id = Uuid::new_v4();
        self.buffer.elements.insert(
            id,
            Element::Image(Box::new(Image {
                data: resvg::usvg::ImageKind::PNG(data.into()),
                visibility: resvg::usvg::Visibility::Visible,
                transform,
                view_box: NonZeroRect::from_xywh(
                    fitted_img_rect.min.x,
                    fitted_img_rect.min.y,
                    fitted_img_rect.width(),
                    fitted_img_rect.height(),
                )
                .unwrap(),
                href,
                opacity: 1.0,
                diff_state: DiffState::new(),
                deleted: false,
            })),
        );

        self.history
            .save(super::Event::Insert(vec![InsertElement { id }]));
        self.toolbar.active_tool = Tool::Selection;
        self.toolbar.selection.selected_elements =
            vec![SelectedElement { id, transform: Transform::identity() }];
    }
}

pub fn duplicate_elements(
    src_elements: IndexMap<Uuid, Element>, src_pressure_map: WeakPathPressures,
    target: &mut Buffer, history: &mut History, maybe_transform: Option<Transform>,
) -> Vec<Uuid> {
    let mut new_ids = Vec::with_capacity(src_elements.iter().count());
    for (id, el) in src_elements.iter() {
        let new_id = Uuid::new_v4();
        let mut new_el = el.clone();

        if let Element::Path(_) = el {
            let maybe_pressure = src_pressure_map.get(id).map(|vec| vec.to_owned());
            if let Some(pressures) = maybe_pressure {
                target.weak_path_pressures.insert(new_id, pressures);
            }
        }

        if let Some(transform) = maybe_transform {
            match &mut new_el {
                Element::Path(path) => {
                    path.data.apply_transform(u_transform_to_bezier(&transform));
                    path.diff_state = DiffState::new();
                }
                Element::Image(image) => {
                    if let Some(new_vbox) = image.view_box.transform(transform) {
                        image.view_box = new_vbox;
                    };
                    image.diff_state = DiffState::new();
                }
                Element::Text(_) => {}
            }
        }

        target.elements.insert_before(0, new_id, new_el);

        new_ids.push(new_id);
    }

    history.save(super::Event::Insert(
        src_elements
            .iter()
            .enumerate()
            .map(|(i, _)| InsertElement { id: *new_ids.get(i).unwrap_or(&Uuid::new_v4()) })
            .collect(),
    ));

    new_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WsPersistentStore;
    use egui::{Context, RawInput, Rect, pos2, vec2};
    use lb_rs::blocking::Lb;
    use lb_rs::model::core_config::{ClientType, Config};

    const SVG: &str = r#"<?xml version="1.0"?>
        <svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M 10 10 L 30 10 L 30 30 L 10 30 Z"/>
        </svg>"#;

    fn editor() -> (Context, SVGEditor) {
        let ctx = Context::default();
        let path = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let lb = Lb::init(Config {
            writeable_path: path.to_string_lossy().into_owned(),
            logs: false,
            stdout_logs: false,
            colored_logs: false,
            background_work: false,
            client_type: ClientType::Unknown,
        })
        .unwrap();
        let cfg = WsPersistentStore::new(false, path, false);
        let editor = SVGEditor::new(
            br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#,
            &ctx,
            lb,
            Uuid::new_v4(),
            None,
            &cfg,
            false,
        );
        (ctx, editor)
    }

    fn frame(ctx: &Context, f: impl FnOnce(&mut egui::Ui)) {
        ctx.begin_pass(RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
            events: vec![egui::Event::PointerMoved(pos2(700., 500.))],
            ..Default::default()
        });
        ctx.root_ui(|root| {
            egui::CentralPanel::default().show(root, f);
        });
        ctx.end_pass().textures_delta.clear();
        ctx.pop_events_where(&mut |_| true);
    }

    #[test]
    fn canvas_svg_paste_uses_anchor_at_zoom_and_supports_undo() {
        let (ctx, mut editor) = editor();
        editor.viewport_settings.master_transform = Transform::from_row(2., 0., 0., 2., 90., 80.);
        let anchor = pos2(200., 150.);
        ctx.push_event(crate::Event::CanvasPaste {
            content: CanvasPasteContent::Svg(SVG.into()),
            position: anchor,
        });
        frame(&ctx, |ui| editor.handle_clip_input(ui));
        assert_eq!(editor.buffer.elements.len(), 1);
        let (&id, element) = editor.buffer.elements.first().unwrap();
        assert!(element.bounding_box().center().distance(anchor) < 0.01);
        assert!((element.bounding_box().width() - 40.).abs() < 0.01);
        assert_eq!(editor.toolbar.selection.selected_elements[0].id, id);
        editor.history.undo(&mut editor.buffer);
        assert!(matches!(&editor.buffer.elements[&id], Element::Path(p) if p.deleted));
        editor.history.redo(&mut editor.buffer);
        assert!(matches!(&editor.buffer.elements[&id], Element::Path(p) if !p.deleted));
    }

    #[test]
    fn pasted_image_is_centered_selected_and_undoable() {
        let (ctx, mut editor) = editor();
        let anchor = pos2(250., 175.);
        let href = Uuid::new_v4();
        frame(&ctx, |ui| {
            // Attachment import supplies the bytes and file id; exercise the same
            // insertion path without requiring an account or a server.
            editor.insert_image(ui, vec![], vec2(80., 60.), href, anchor);
        });
        let (&id, element) = editor.buffer.elements.first().unwrap();
        assert_eq!(element.bounding_box().center(), anchor);
        assert_eq!(editor.toolbar.selection.selected_elements[0].id, id);
        assert!(matches!(element, Element::Image(image) if image.href == href));
        editor.history.undo(&mut editor.buffer);
        assert!(matches!(&editor.buffer.elements[&id], Element::Image(image) if image.deleted));
        editor.history.redo(&mut editor.buffer);
        assert!(matches!(&editor.buffer.elements[&id], Element::Image(image) if !image.deleted));
    }

    #[test]
    fn invalid_clipboard_image_does_not_panic_or_add_an_element() {
        let (ctx, mut editor) = editor();
        ctx.push_event(crate::Event::CanvasPaste {
            content: CanvasPasteContent::Image(b"not an image".to_vec()),
            position: pos2(200., 150.),
        });
        frame(&ctx, |ui| editor.handle_clip_input(ui));
        assert!(editor.buffer.elements.is_empty());
        assert!(!editor.history.has_undo());
    }

    #[test]
    fn read_only_canvas_ignores_paste() {
        let (ctx, mut editor) = editor();
        editor.read_only = true;
        ctx.push_event(crate::Event::CanvasPaste {
            content: CanvasPasteContent::Svg(SVG.into()),
            position: pos2(200., 150.),
        });
        frame(&ctx, |ui| editor.handle_clip_input(ui));
        assert!(editor.buffer.elements.is_empty());
        assert!(!editor.history.has_undo());
    }

    #[test]
    fn canvas_menu_cancellation_reverts_selection_preview() {
        use crate::tab::input_controller::InputControllerEvent;
        use crate::tab::svg_editor::toolbar::ToolContext;
        use crate::tab::svg_editor::tools::InputControllerTool;
        use crate::tab::svg_editor::tools::selection::SelectionEvent;

        let (ctx, mut editor) = editor();
        ctx.push_event(crate::Event::CanvasPaste {
            content: CanvasPasteContent::Svg(SVG.into()),
            position: pos2(200., 150.),
        });
        frame(&ctx, |ui| editor.handle_clip_input(ui));
        let id = editor.toolbar.selection.selected_elements[0].id;
        let original_bounds = editor.buffer.elements[&id].bounding_box();
        let preview = Transform::from_translate(4., 3.);
        editor.buffer.elements[&id].transform(preview);
        editor.toolbar.selection.selected_elements[0].transform = preview;
        frame(&ctx, |ui| {
            let selection = &mut editor.toolbar.selection;
            let event = selection.controller_event_to_tool_event(InputControllerEvent::ToolCancel);
            assert!(matches!(event, Some(SelectionEvent::Cancel)));
            selection.handle_tool_event(
                ui,
                event.unwrap(),
                &mut ToolContext {
                    painter: &mut editor.painter,
                    read_only: false,
                    buffer: &mut editor.buffer,
                    history: &mut editor.history,
                    settings: &mut editor.settings,
                    viewport_settings: &mut editor.viewport_settings,
                },
            );
        });
        assert_eq!(editor.buffer.elements[&id].bounding_box(), original_bounds);
        assert!(
            editor.toolbar.selection.selected_elements[0]
                .transform
                .is_identity()
        );
        editor.history.undo(&mut editor.buffer);
        assert!(matches!(&editor.buffer.elements[&id], Element::Path(p) if p.deleted));
    }
}
