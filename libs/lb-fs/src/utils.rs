use nfs3_server::nfs3_types::nfs3::filename3;

pub fn get_string(f: &filename3) -> String {
    String::from_utf8(f.as_ref().to_vec()).expect("Invalid UTF-8")
}
