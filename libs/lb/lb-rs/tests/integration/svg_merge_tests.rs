use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::element::{Color, Element};
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
    Buffer::reload(&mut local, &base, &remote);
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

fn reload(local: &mut Buffer, base: &str, remote: &str) {
    Buffer::reload(local, &Buffer::new(base), &Buffer::new(remote));
}

#[test]
fn readable_ids_merge_once() {
    let doc = r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="db" d="M 0 0 L 0 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/></svg>"#;
    let mut local = Buffer::new(doc);
    reload(&mut local, doc, doc);
    assert_eq!(local.elements.len(), 1);
    assert!(local.serialize().contains("id='db'"));
}

/// Another device recolors a stroke red; after syncing, this device has it red too.
#[test]
fn remote_recolor_survives_merge() {
    let black = svg(&[1]);
    let red = black.replace("rgb(0,0,0)", "rgb(255,0,0)");
    let mut local = Buffer::new(&black);
    reload(&mut local, &black, &red);
    let Element::Path(stroke) = &local.elements[&id(1)] else { unreachable!() };
    assert_eq!(stroke.stroke.unwrap().color.light, Color::new_rgb(255, 0, 0));
}

/// Another device brings a stroke to the front; this device's new stroke stays
/// in front of the one it was drawn over.
#[test]
fn remote_restack_survives_merge() {
    let mut local = Buffer::new(&svg(&[1, 2, 3]));
    local.elements.move_index(2, 1); // drawn here, in front of 2: [1, 9, 2]
    let local_only = Uuid::new_v4();
    let el = local.elements[&id(3)].clone();
    local.elements.shift_remove(&id(3));
    local.elements.insert_before(1, local_only, el);
    reload(&mut local, &svg(&[1, 2]), &svg(&[2, 1]));
    assert_eq!(local.elements.keys().copied().collect::<Vec<_>>(), [id(2), id(1), local_only]);
}
