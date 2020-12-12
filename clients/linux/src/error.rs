pub type LbResult<T> = Result<T, LbError>;

pub enum LbError {
    User(String),
    Program(String),
}
