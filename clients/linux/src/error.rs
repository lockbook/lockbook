#[macro_export]
macro_rules! uerr {
    ($base:literal $(, $args:tt )* | $target:expr) => {
        LbError::new_user_err(format!($base $(, $args )*), $target)
    };
}

#[macro_export]
macro_rules! uerr_status_panel {
    ($base:literal $(, $args:tt )*) => {
        uerr!($base $(, $args )* | LbErrTarget::StatusPanel)
    };
}

#[macro_export]
macro_rules! uerr_dialog {
    ($base:literal $(, $args:tt )*) => {
        uerr!($base $(, $args )* | LbErrTarget::Dialog)
    };
}

#[macro_export]
macro_rules! progerr {
    ($base:literal $(, $args:tt )*) => {
        LbError::new_program_err(format!($base $(, $args )*))
    };
}

pub type LbResult<T> = Result<T, LbError>;

pub enum LbErrKind {
    Program,
    User,
}

pub enum LbErrTarget {
    Dialog,
    StatusPanel,
}

pub struct LbError {
    kind: LbErrKind,
    msg: String,
    target: LbErrTarget,
}

impl LbError {
    pub fn new(kind: LbErrKind, msg: String, target: LbErrTarget) -> Self {
        Self { kind, msg, target }
    }

    pub fn new_program_err(msg: String) -> Self {
        Self::new(LbErrKind::Program, msg, LbErrTarget::Dialog)
    }

    pub fn new_user_err(msg: String, target: LbErrTarget) -> Self {
        Self::new(LbErrKind::User, msg, target)
    }

    pub fn msg(&self) -> &String {
        &self.msg
    }

    pub fn kind(&self) -> &LbErrKind {
        &self.kind
    }

    pub fn target(&self) -> &LbErrTarget {
        &self.target
    }

    pub fn is_prog(&self) -> bool {
        matches!(self.kind, LbErrKind::Program)
    }

    pub fn fmt_program_err<T: std::fmt::Debug>(err: T) -> Self {
        progerr!("{:?}", err)
    }
}
