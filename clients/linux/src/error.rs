#[macro_export]
macro_rules! uerr {
    ($base:literal $(, $args:tt )*) => {
        LbError::new_user_err(format!($base $(, $args )*))
    };
}

#[macro_export]
macro_rules! progerr {
    ($base:literal $(, $args:tt )*) => {
        LbError::new_program_err(format!($base $(, $args )*))
    };
}

pub type LbResult<T> = Result<T, LbError>;

#[derive(Debug)]
pub enum LbErrKind {
    Program,
    User,
}

#[derive(Debug)]
pub struct LbError {
    kind: LbErrKind,
    msg: String,
}

impl LbError {
    pub fn new(kind: LbErrKind, msg: String) -> Self {
        Self { kind, msg }
    }

    pub fn new_program_err(msg: String) -> Self {
        Self::new(LbErrKind::Program, msg)
    }

    pub fn new_user_err(msg: String) -> Self {
        Self::new(LbErrKind::User, msg)
    }

    pub fn msg(&self) -> &String {
        &self.msg
    }

    pub fn kind(&self) -> &LbErrKind {
        &self.kind
    }

    pub fn is_prog(&self) -> bool {
        matches!(self.kind, LbErrKind::Program)
    }

    pub fn fmt_program_err<T: std::fmt::Debug>(err: T) -> Self {
        progerr!("{:?}", err)
    }
}
