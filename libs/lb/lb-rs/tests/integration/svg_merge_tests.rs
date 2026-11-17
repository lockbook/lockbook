use lb_rs::model::filename::DocumentType;
use lb_rs::model::svg::WeakTransform;
use lb_rs::model::svg::buffer::{Buffer, WeakViewportSettings};
use lb_rs::model::svg::element::EditableElement;
use usvg::Transform;
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
    let merged = DocumentType::Drawing
        .merge(svg(base).as_bytes(), svg(local).as_bytes(), svg(remote).as_bytes())
        .unwrap();
    Buffer::new(std::str::from_utf8(&merged).unwrap())
        .elements
        .keys()
        .copied()
        .collect()
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

fn legacy_svg(strokes: &[u8], scale: f32) -> String {
    let viewport = WeakViewportSettings {
        master_transform: WeakTransform::from(
            Transform::from_scale(scale, scale).post_translate(30., 40.),
        ),
        ..Default::default()
    };
    let encoded = base64::encode(bincode::serialize(&viewport).unwrap());
    svg(strokes).replace(
        "</svg>",
        &format!("<g id=\"lb_viewport_settings\"><g id=\"{encoded}\"></g></g></svg>"),
    )
}

fn assert_drawing(expected: &str, actual: &[u8]) {
    let drawing = |bytes: &str| {
        Buffer::new(bytes)
            .elements
            .into_iter()
            .filter_map(|(id, element)| {
                let EditableElement::Path(path) = element else { return None };
                let anchors = path
                    .data
                    .manipulator_groups()
                    .iter()
                    .map(|g| (g.anchor, g.in_handle, g.out_handle))
                    .collect::<Vec<_>>();
                let stroke = path.stroke.unwrap();
                Some((id, (anchors, stroke.width * path.stroke_scale(), stroke.opacity)))
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(drawing(expected), drawing(std::str::from_utf8(actual).unwrap()));
    assert!(
        !std::str::from_utf8(actual)
            .unwrap()
            .contains("lb_viewport_settings")
    );
}

#[test]
fn drawing_merge_ignores_legacy_viewports() {
    let base = legacy_svg(&[1], 2.);
    let local = legacy_svg(&[1, 2], 3.);
    let remote = legacy_svg(&[1, 3], 4.);
    // Exercise real decoded compatibility metadata, rather than an identity view.
    assert_eq!(Transform::from(Buffer::new(&local).weak_viewport_settings.master_transform).sx, 3.);
    let merged = DocumentType::Drawing
        .merge(base.as_bytes(), local.as_bytes(), remote.as_bytes())
        .unwrap();
    assert_drawing(&svg(&[1, 2, 3]), &merged);
}

#[tokio::test]
async fn drawing_write_from_base_preserves_document_coordinates() {
    let core = test_utils::test_core_with_account().await;
    let id = core.create_at_path("drawing.svg").await.unwrap().id;
    let base = legacy_svg(&[1], 2.);
    let local = legacy_svg(&[1, 2], 3.);
    let remote = legacy_svg(&[1, 3], 4.);
    core.write_document(id, remote.as_bytes()).await.unwrap();
    let (_, merged) = core
        .write_from_base(id, base.as_bytes(), local.into_bytes(), None)
        .await
        .unwrap();
    assert!(merged);
    assert_drawing(&svg(&[1, 2, 3]), &core.read_document(id, false).await.unwrap());
}

#[tokio::test]
async fn drawing_sync_preserves_document_coordinates() {
    let a = test_utils::test_core_with_account().await;
    let id = a.create_at_path("drawing.svg").await.unwrap().id;
    a.write_document(id, legacy_svg(&[1], 2.).as_bytes())
        .await
        .unwrap();
    a.sync().await.unwrap();
    let b = test_utils::another_client(&a).await;
    b.sync().await.unwrap();
    a.write_document(id, legacy_svg(&[1, 2], 3.).as_bytes())
        .await
        .unwrap();
    b.write_document(id, legacy_svg(&[1, 3], 4.).as_bytes())
        .await
        .unwrap();
    b.sync().await.unwrap();
    a.sync().await.unwrap();
    b.sync().await.unwrap();
    for core in [&a, &b] {
        assert_drawing(&svg(&[1, 2, 3]), &core.read_document(id, false).await.unwrap());
    }
}
