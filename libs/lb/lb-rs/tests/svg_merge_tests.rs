use lb_rs::model::svg::buffer::Buffer;
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn stroke(n: u8) -> String {
    format!(
        r#"<path id="{}" d="M {n} 0 L {n} 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/>"#,
        id(n)
    )
}

fn svg(strokes: &[u8]) -> String {
    let body: String = strokes.iter().map(|&n| stroke(n)).collect();
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{body}</svg>"#)
}

/// The syncer's drawing merge: local edits, then remote edits replayed on top.
fn merge(base: &[u8], local: &[u8], remote: &[u8]) -> Vec<Uuid> {
    let base = Buffer::new(&svg(base));
    let remote = Buffer::new(&svg(remote));
    let mut local = Buffer::new(&svg(local));
    Buffer::reload(
        &mut local.elements,
        &mut local.weak_images,
        &mut local.weak_path_pressures,
        &mut local.weak_viewport_settings,
        &base,
        &remote,
    );
    local.elements.keys().copied().collect()
}

#[test]
fn local_erase_with_remote_draw() {
    assert_eq!(merge(&[1, 2, 3], &[2, 3], &[1, 2, 3, 9]), [id(2), id(3), id(9)]);
}

#[test]
fn remote_draw_keeps_its_position() {
    assert_eq!(merge(&[1, 2, 3], &[1, 2, 3], &[1, 9, 2, 3]), [id(1), id(9), id(2), id(3)]);
}

#[test]
fn remote_draw_after_locally_erased_neighbor() {
    assert_eq!(merge(&[1, 2, 3], &[1, 3], &[1, 2, 9, 3]), [id(1), id(9), id(3)]);
}

fn weak_image(target: Uuid) -> lb_rs::model::svg::element::WeakImage {
    lb_rs::model::svg::element::WeakImage {
        href: target,
        transform: Default::default(),
        opacity: 0.75,
        x: -40.0,
        y: 300.0,
        width: 500.0,
        height: 240.0,
        z_index: 0,
    }
}

#[test]
fn svg_images_keep_legacy_metadata_and_export_relative_tags() {
    let mut buffer = Buffer::default();
    buffer.weak_images.insert(id(1), weak_image(id(9)));
    buffer
        .image_hrefs
        .insert(id(9), "imports/a%20%F0%9F%8C%BB&\".png".into());
    let saved = buffer.serialize();
    let xml = usvg::roxmltree::Document::parse(&saved).unwrap();
    let tag = xml.descendants().find(|n| n.has_tag_name("image")).unwrap();
    assert_eq!(tag.attribute("href"), Some("imports/a%20%F0%9F%8C%BB&\".png"));
    assert_eq!(tag.attribute("x"), Some("-40"));
    assert_eq!(tag.attribute("y"), Some("300"));
    assert_eq!(tag.attribute("opacity"), Some("0.75"));
    assert_eq!(xml.root_element().attribute("viewBox"), Some("-40 300 500 240"));
    let reopened = Buffer::new(&saved);
    assert!(reopened.elements.is_empty(), "the export tag must not duplicate its metadata image");
    assert_eq!(reopened.weak_images, buffer.weak_images);
    assert_eq!(reopened.image_hrefs, buffer.image_hrefs);
    assert_eq!(Buffer::new(&reopened.serialize()).weak_images, buffer.weak_images);

    // Old clients stored exactly this metadata, with no SVG image tag.
    let mut legacy = buffer.clone();
    legacy.image_hrefs.clear();
    let legacy = legacy
        .serialize()
        .replace(" data-lb-paint-order=\"back-to-front\"", "");
    assert_eq!(Buffer::new(&legacy).weak_images, buffer.weak_images);
}

#[test]
fn export_viewport_and_paint_order_do_not_change_canvas_paths_on_reopen() {
    let buffer = Buffer::new(&svg(&[1, 2, 3]));
    let saved = buffer.serialize();
    let reopened = Buffer::new(&saved);
    assert_eq!(
        reopened.elements.keys().collect::<Vec<_>>(),
        buffer.elements.keys().collect::<Vec<_>>()
    );
    for (id, before) in &buffer.elements {
        match (before, &reopened.elements[id]) {
            (
                lb_rs::model::svg::element::Element::Path(a),
                lb_rs::model::svg::element::Element::Path(b),
            ) => {
                assert_eq!(a.data.bounding_box(), b.data.bounding_box());
                assert_eq!(a.transform, b.transform);
            }
            _ => panic!("expected paths"),
        }
    }
}

#[test]
fn invalid_image_metadata_does_not_crash_or_discard_paths() {
    for metadata in ["%%%", "AA=="] {
        let drawing = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">{}<g id="lb_images"><g id="{metadata}"/></g></svg>"#,
            stroke(1)
        );
        let buffer = Buffer::new(&drawing);
        assert!(buffer.elements.contains_key(&id(1)));
        assert!(buffer.weak_images.is_empty());
    }
}

#[test]
fn image_resize_and_delete_merge_without_dropping_unreadable_targets() {
    let mut base = Buffer::default();
    base.weak_images.insert(id(1), weak_image(id(9)));
    base.weak_images.insert(id(2), weak_image(id(8)));
    let mut local = base.clone();
    let mut remote = base.clone();
    remote.weak_images.get_mut(&id(1)).unwrap().width = 800.0;
    remote.weak_images.remove(&id(2));
    Buffer::reload(
        &mut local.elements,
        &mut local.weak_images,
        &mut local.weak_path_pressures,
        &mut local.weak_viewport_settings,
        &base,
        &remote,
    );
    let reopened = Buffer::new(&local.serialize());
    assert_eq!(reopened.weak_images[&id(1)].width, 800.0);
    assert_eq!(reopened.weak_images[&id(1)].href, id(9));
    assert!(!reopened.weak_images.contains_key(&id(2)));
}

#[test]
fn sync_serialization_interleaves_images_with_strokes_in_paint_order() {
    let mut buffer = Buffer::new(&svg(&[1, 2]));
    buffer
        .weak_images
        .insert(id(3), lb_rs::model::svg::element::WeakImage { z_index: 1, ..weak_image(id(9)) });
    buffer.image_hrefs.insert(id(9), "imports/photo.png".into());
    let saved = buffer.serialize();
    let xml = usvg::roxmltree::Document::parse(&saved).unwrap();
    let ids: Vec<_> = xml
        .descendants()
        .filter(|n| n.has_tag_name("path") || n.has_tag_name("image"))
        .map(|n| n.attribute("id").unwrap().parse::<Uuid>().unwrap())
        .collect();
    assert_eq!(ids, [id(2), id(3), id(1)]);
    let reopened = Buffer::new(&saved);
    assert_eq!(reopened.weak_images[&id(3)].z_index, 1);
}
