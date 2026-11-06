//! Image integration at the canvas boundary: common cache, editable metadata,
//! and destinations generated from the receiving canvas's current folder.
use std::sync::{Arc, RwLock};

use egui::{Color32, ColorImage, ImageData, RawInput, Rect, TextureId, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::model::svg::element::{Element, WeakImage};

use super::{SVGEditor, element::PromoteBufferWeakImages};
use crate::test_utils::files::{at, cache, tree};
use crate::widgets::image_cache::{ImageCache, ImageKey};
use crate::workspace::WsPersistentStore;

fn fixture() -> (egui::Context, SVGEditor, Uuid, Uuid) {
    let ctx = crate::tab::markdown_editor::test_egui_ctx();
    use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
    ctx.set_lb_theme(Theme::default(Mode::Light));
    let files = cache(tree("alice", &["a/canvas.svg", "b/canvas.svg", "a/imports/sun 🌻.png"]));
    let source = at(&files, "/a/canvas.svg");
    let destination = at(&files, "/b/canvas.svg");
    let target = at(&files, "/a/imports/sun 🌻.png");
    let cfg = WsPersistentStore::new(false, format!("/tmp/{}", Uuid::new_v4()).into(), true);
    let lb = crate::tab::markdown_editor::tests::harness::build_lb();
    let images = ImageCache::new(
        ctx.clone(),
        Default::default(),
        lb.clone(),
        Arc::new(RwLock::new(files)),
        cfg.clone(),
    );
    let svg = SVGEditor::new(
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        &ctx,
        lb,
        source,
        None,
        &cfg,
        false,
        images,
    );
    (ctx, svg, target, destination)
}

fn weak(target: Uuid, z_index: usize) -> WeakImage {
    WeakImage {
        href: target,
        transform: Default::default(),
        opacity: 1.0,
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        z_index,
    }
}

fn add_image(svg: &mut SVGEditor, target: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    svg.buffer.weak_images.insert(id, weak(target, 0));
    svg.buffer.promote_weak_images(Default::default());
    id
}

fn paint(ctx: &egui::Context, svg: &mut SVGEditor) -> Vec<egui::epaint::ClippedShape> {
    svg.images.begin_frame();
    let mut out = ctx.run_pass(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
            ..Default::default()
        },
        |ctx| {
            ctx.root_ui(|ui| {
                let mut painter = ui.painter().clone();
                svg.renderer.render_svg(
                    ui,
                    &mut svg.buffer,
                    &mut painter,
                    Default::default(),
                    Default::default(),
                    &svg.document_images,
                );
            });
        },
    );
    svg.images.end_frame();
    out.textures_delta.clear();
    out.shapes
}

fn image_meshes(shapes: &[egui::epaint::ClippedShape], texture: TextureId) -> usize {
    shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::Rect(rect) if rect.brush.as_ref().is_some_and(|b| b.fill_texture_id == texture))).count()
}

#[test]
fn canvas_requests_static_images_each_frame_and_releases_offscreen_textures() {
    let (ctx, mut svg, target, _) = fixture();
    add_image(&mut svg, target);
    let key = ImageKey::File(target);
    svg.images.complete_load(&key, Err("damaged image".into()));
    let failed = paint(&ctx, &mut svg);
    assert!(!failed.is_empty(), "a failed image still paints a placeholder");
    assert_eq!(svg.buffer.elements.len(), 1);

    let texture = ctx.tex_manager().write().alloc(
        "canvas_test".into(),
        ImageData::Color(Arc::new(ColorImage::filled([8, 6], Color32::RED))),
        Default::default(),
    );
    svg.images.complete_load(&key, Ok(texture));
    for _ in 0..3 {
        svg.buffer.elements.values_mut().for_each(|e| {
            if let Element::Image(i) = e {
                i.diff_state = Default::default();
            }
        });
        assert_eq!(
            image_meshes(&paint(&ctx, &mut svg), texture),
            1,
            "completion must paint even without a geometry edit"
        );
        assert!(ctx.tex_manager().read().meta(texture).is_some());
    }
    // A viewport pan puts the image outside the clip; the common cache frees it.
    svg.buffer
        .elements
        .values_mut()
        .for_each(|e| e.transform(resvg::usvg::Transform::from_translate(2000.0, 0.0)));
    paint(&ctx, &mut svg);
    assert!(!svg.images.has_state(&key));
    assert!(ctx.tex_manager().read().meta(texture).is_none());
}

#[test]
fn broken_images_and_invalid_geometry_survive_saving() {
    let (_, mut svg, target, _) = fixture();
    let good = add_image(&mut svg, target);
    let missing = Uuid::new_v4();
    add_image(&mut svg, missing);
    let invalid = Uuid::new_v4();
    svg.buffer
        .weak_images
        .insert(invalid, WeakImage { width: 0.0, ..weak(target, 2) });
    svg.buffer.promote_weak_images(Default::default());
    assert!(svg.buffer.elements.contains_key(&good));
    assert!(svg.buffer.weak_images.contains_key(&invalid));
    let saved = svg.save_buffer().serialize();
    let reopened = lb_rs::model::svg::buffer::Buffer::new(&saved);
    assert_eq!(reopened.weak_images.len(), 3);
    assert!(reopened.weak_images.values().any(|i| i.href == missing));
    assert_eq!(reopened.weak_images[&invalid].width, 0.0);
}

#[test]
fn canvas_clipboard_keeps_identity_and_rebases_export_paths() {
    let (ctx, mut source, target, destination) = fixture();
    add_image(&mut source, target);
    let exported = source.save_buffer();
    let source_href = &exported.image_hrefs[&target];
    assert_eq!(source_href, "imports/sun%20🌻.png");
    let mut pasted = lb_rs::model::svg::buffer::Buffer::new(&exported.serialize());
    pasted.promote_weak_images(Default::default());
    let mut receiver = SVGEditor::new(
        b"<svg/>",
        &ctx,
        source.lb.clone(),
        destination,
        None,
        &source.cfg,
        false,
        source.images.clone(),
    );
    super::clip::duplicate_elements(
        pasted.elements,
        pasted.weak_path_pressures,
        &mut receiver.buffer,
        &mut receiver.history,
        Some(Default::default()),
    );
    let received = receiver.save_buffer();
    assert_eq!(received.image_hrefs[&target], "../a/imports/sun%20🌻.png");
    assert!(
        matches!(received.elements.values().next(), Some(Element::Image(i)) if i.href == target)
    );
}

#[test]
fn invalid_paste_is_rejected_before_account_or_files_are_needed() {
    let (_, svg, _, _) = fixture();
    let error =
        crate::image_import::import(&svg.images, &svg.lb, svg.open_file, None, b"not an image")
            .err()
            .expect("invalid image must fail");
    assert!(!error.contains("Account"), "decode must happen before file creation: {error}");
}

#[test]
fn exported_image_is_visible_to_a_standard_svg_renderer() {
    let (_, mut svg, target, _) = fixture();
    add_image(&mut svg, target);
    let bytes =
        crate::widgets::image_cache::encode_png(&ColorImage::filled([8, 6], Color32::RED)).unwrap();
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(move |href, _| {
        assert_eq!(href, "imports/sun%20🌻.png");
        Some(resvg::usvg::ImageKind::PNG(Arc::new(bytes.clone())))
    });
    let saved = svg.save_buffer().serialize();
    let tree = resvg::usvg::Tree::from_str(&saved, &options).unwrap();
    let size = tree.size().to_int_size();
    let mut pixels = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(&tree, Default::default(), &mut pixels.as_mut());
    let center = pixels.pixel(size.width() / 2, size.height() / 2).unwrap();
    assert_eq!(center.red(), 255);
    assert_eq!(center.alpha(), 255);
}

#[test]
fn clipboard_paste_scales_once_and_preserves_image_stacking() {
    let (ctx, mut svg, target, _) = fixture();
    let back = add_image(&mut svg, target);
    let front = add_image(&mut svg, target);
    // Distinguish images by opacity without changing their shared file identity.
    if let Element::Image(i) = svg.buffer.elements.get_mut(&front).unwrap() {
        i.opacity = 0.25;
    }
    if let Element::Image(i) = svg.buffer.elements.get_mut(&back).unwrap() {
        i.opacity = 0.75;
    }
    let clipboard = svg.save_buffer().serialize();
    svg.buffer = Default::default();
    svg.viewport_settings.master_transform = resvg::usvg::Transform::from_scale(2.0, 2.0);
    let mut output = ctx.run_pass(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
            events: vec![egui::Event::Paste(clipboard)],
            ..Default::default()
        },
        |ctx| {
            ctx.root_ui(|ui| svg.handle_clip_input(ui));
        },
    );
    output.textures_delta.clear();
    let images: Vec<_> = svg
        .buffer
        .elements
        .values()
        .filter_map(|e| if let Element::Image(i) = e { Some(i) } else { None })
        .collect();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].opacity, 0.25);
    assert_eq!(images[1].opacity, 0.75);
    for image in images {
        assert_eq!(image.view_box.width(), 160.0);
        assert_eq!(image.view_box.height(), 120.0);
    }
}

#[test]
fn saving_after_a_target_move_refreshes_its_relative_export_path() {
    let (_, mut svg, target, _) = fixture();
    add_image(&mut svg, target);
    let moved = cache(tree("alice", &["a/canvas.svg", "b/canvas.svg", "b/imports/new 🌻.png"]));
    assert_eq!(at(&moved, "/b/imports/new 🌻.png"), target);
    svg.images.files().write().unwrap().replace(moved);
    let saved = svg.save_buffer();
    assert_eq!(saved.image_hrefs[&target], "../b/imports/new%20🌻.png");
}
