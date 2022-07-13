use rand::distributions::Alphanumeric;
use rand::rngs::OsRng;
use rand::Rng;

use lockbook_core::Core;
use lockbook_models::file_metadata::CoreFile;

pub fn find_by_name(core: &Core, name: &str) -> CoreFile {
    let mut possible_matches = core.list_metadatas().unwrap();
    if name == "root" {
        possible_matches.retain(|meta| meta.parent == meta.id);
    } else {
        possible_matches.retain(|meta| meta.decrypted_name == name);
    }
    if possible_matches.len() > 1 {
        eprintln!("Multiple matches for a file name found: {}", name);
    } else if possible_matches.is_empty() {
        panic!("No file matched name: {}", name);
    }

    possible_matches[0].clone()
}

pub fn random_utf8() -> String {
    OsRng
        .sample_iter(&Alphanumeric)
        .take(1024)
        .map(char::from)
        .collect()
}

pub fn random_filename() -> String {
    OsRng
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect()
}
