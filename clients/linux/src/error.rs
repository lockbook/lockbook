pub type LbResult<T> = Result<T, LbError>;

pub enum LbError {
    User(String),
    Program(String),
}

impl LbError {
    pub fn msg(&self) -> &String {
        match self {
            Self::User(msg) => msg,
            Self::Program(msg) => msg,
        }
    }

    pub fn is_prog(&self) -> bool {
        match self {
            Self::User(_) => false,
            Self::Program(_) => true,
        }
    }
}
