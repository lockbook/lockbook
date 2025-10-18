use std::time::{SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use lb_rs::{
    Uuid,
    blocking::Lb,
    model::{file::File, file_metadata::FileType},
};

pub mod show;

pub fn import_transcription(lb: &Lb, file_id: Uuid, data: &[u8]) -> File {
    let file = lb
        .get_file_by_id(file_id)
        .expect("get lockbook file for transcription");
    let siblings = lb
        .get_children(&file.parent)
        .expect("get lockbook siblings for transcription");

    let file_name = file.name;

    let imports_folder = {
        let mut imports_folder = None;
        for sibling in siblings {
            if sibling.name == "imports" {
                imports_folder = Some(sibling);
                break;
            }
        }
        imports_folder.unwrap_or_else(|| {
            lb.create_file("imports", &file.parent, FileType::Folder)
                .expect("create lockbook folder for transcription")
        })
    };

    // get local time in a human readable datetime format
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let human_readable_time = DateTime::from_timestamp(time.as_secs() as _, 0)
        .expect("invalid system time")
        .format("%Y-%m-%d_%H-%M-%S")
        .to_string();

    let file_extension = ".txt";

    let file = lb
        .create_file(
            &format!("{file_name} {human_readable_time}.{file_extension}"),
            &imports_folder.id,
            FileType::Document,
        )
        .expect("create lockbook file for transcription");
    lb.write_document(file.id, data)
        .expect("write lockbook file for transcription");

    file
}
