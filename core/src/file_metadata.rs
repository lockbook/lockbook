#[derive(PartialEq, Debug)]
pub struct FileMetadata {
    pub id: String,
    pub name: String,
    pub path: String,
    pub updated_at: i64,
    pub status: String,
}
