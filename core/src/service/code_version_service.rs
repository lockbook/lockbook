pub trait CodeVersion {
    fn get_code_version() -> &'static str;
}

pub struct CodeVersionImpl;

impl CodeVersion for CodeVersionImpl {
    fn get_code_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}
