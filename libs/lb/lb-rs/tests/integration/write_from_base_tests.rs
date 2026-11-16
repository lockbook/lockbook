//! Writes from a base the writer read, against documents others changed since.

use lb_rs::Uuid;
use lb_rs::model::errors::LbErrKind;
use test_utils::byte_source::ByteSource;
use test_utils::{Lb, another_client, prop, test_core_with_account};

/// Two writers edit one base on either side of a line neither touches, each inserting a line
/// of its own and maybe deleting one of the base's, on one device or two, syncing between the
/// writes or not. Neither inserted line is lost and the devices end up agreeing.
#[test]
fn writers_from_one_base_both_land() {
    prop::run(100, 16, |bytes| runtime().block_on(both_land(bytes)), |_| String::new());
}

async fn both_land(bytes: &[u8]) -> Result<(), String> {
    let mut src = ByteSource::new(bytes);
    let (above, below) = (1 + src.draw(3), 1 + src.draw(3));
    let base_lines: Vec<String> = (0..above + 1 + below)
        .map(|i| format!("line {i}"))
        .collect();
    let base = lines(&base_lines);
    // edits lines lo..hi, inserting its marker before one of them or after the last
    let mut edit = |marker: &str, lo: usize, hi: usize| {
        let deleted = (src.draw(2) == 1).then(|| lo + src.draw(hi - lo));
        let inserted = lo + src.draw(hi - lo + 1);
        let mut edited = vec![];
        for (i, line) in base_lines.iter().enumerate() {
            if i == inserted {
                edited.push(marker.to_string());
            }
            if Some(i) != deleted {
                edited.push(line.clone());
            }
        }
        if inserted == base_lines.len() {
            edited.push(marker.to_string());
        }
        lines(&edited)
    };
    let (mut ours_a, mut ours_b) =
        (edit("from a", 0, above), edit("from b", above + 1, base_lines.len()));
    if src.draw(2) == 1 {
        std::mem::swap(&mut ours_a, &mut ours_b);
    }
    let (same_device, a_pushes, b_pulls) = (src.draw(2) == 1, src.draw(2) == 1, src.draw(2) == 1);

    let a = test_core_with_account().await;
    let id = a.create_at_path("board.md").await.unwrap().id;
    a.write_document(id, &base).await.unwrap();
    a.sync().await.unwrap();
    let b = another_client(&a).await;
    b.sync().await.unwrap();

    a.write_from_base(id, &base, ours_a, None)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let writer_b = if same_device { &a } else { &b };
    if a_pushes {
        a.sync().await.unwrap();
    }
    if b_pulls {
        writer_b.sync().await.unwrap();
    }
    writer_b
        .write_from_base(id, &base, ours_b, None)
        .await
        .map_err(|e| format!("{e:?}"))?;
    for device in [writer_b, &a, &b] {
        device.sync().await.unwrap();
    }

    let (on_a, on_b) = (read(&a, id).await, read(&b, id).await);
    if on_a != on_b {
        return Err(format!("devices disagree:\n{on_a}\n---\n{on_b}"));
    }
    for marker in ["from a", "from b"] {
        if !on_a.lines().any(|line| line == marker) {
            return Err(format!("lost '{marker}':\n{on_a}"));
        }
    }
    Ok(())
}

#[tokio::test]
async fn moved_document_without_a_merge_is_left_untouched() {
    refuses("photo.png", b"base", b"theirs", b"ours").await;
}

#[tokio::test]
async fn json_that_does_not_parse_is_left_untouched() {
    refuses("settings.json", br#"{"a":1}"#, b"not json", br#"{"a":3}"#).await;
}

#[tokio::test]
async fn a_repeated_write_lands_once() {
    let lb = test_core_with_account().await;
    let id = lb.create_at_path("board.md").await.unwrap().id;
    lb.write_document(id, b"line 0\n").await.unwrap();
    let ours = b"line 0\nline 1\n".to_vec();
    for _ in 0..2 {
        lb.write_from_base(id, b"line 0\n", ours.clone(), None)
            .await
            .unwrap();
    }
    assert_eq!(lb.read_document(id, false).await.unwrap(), ours);
}

#[tokio::test]
async fn writing_back_what_was_read_keeps_their_version() {
    let lb = test_core_with_account().await;
    let id = lb.create_at_path("photo.png").await.unwrap().id;
    lb.write_document(id, b"theirs").await.unwrap();
    lb.write_from_base(id, b"base", b"base".to_vec(), None)
        .await
        .unwrap();
    assert_eq!(lb.read_document(id, false).await.unwrap(), b"theirs");
}

#[tokio::test]
async fn deleted_document_is_not_written() {
    let lb = test_core_with_account().await;
    let id = lb.create_at_path("board.md").await.unwrap().id;
    lb.delete(&id).await.unwrap();
    assert!(
        lb.write_from_base(id, b"", b"ours".to_vec(), None)
            .await
            .is_err()
    );
}

async fn refuses(name: &str, base: &[u8], theirs: &[u8], ours: &[u8]) {
    let lb = test_core_with_account().await;
    let id = lb.create_at_path(name).await.unwrap().id;
    lb.write_document(id, theirs).await.unwrap();
    let err = lb
        .write_from_base(id, base, ours.to_vec(), None)
        .await
        .unwrap_err();
    assert_eq!(err.kind, LbErrKind::ReReadRequired);
    assert_eq!(lb.read_document(id, false).await.unwrap(), theirs);
}

fn lines(lines: &[String]) -> Vec<u8> {
    lines
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>()
        .into_bytes()
}

async fn read(lb: &Lb, id: Uuid) -> String {
    String::from_utf8(lb.read_document(id, false).await.unwrap()).unwrap()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
