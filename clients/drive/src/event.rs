pub type LbPath = String;

pub enum DriveEvent {
    Create(LbPath),
    Delete(LbPath),
}
