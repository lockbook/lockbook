pub type LbPath = String;

#[derive(Debug)]
pub enum DriveEvent {
    Create(LbPath),
    Delete(LbPath),
    Rename(LbPath, String),
    Move(LbPath, LbPath),
    DocumentModified(LbPath, Vec<u8>),
}
