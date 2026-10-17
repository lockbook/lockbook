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
