use std::env;

pub fn data_dir() -> Result<String, String> {
    match (env::var("LOCKBOOK_PATH"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => Ok(s),
        (Err(_), Ok(s), _) => Ok(format!("{s}/.lockbook")),
        (Err(_), Err(_), Ok(s)) => Ok(format!("{s}/.lockbook")),
        _ => Err("Unable to determine a Lockbook data directory. Please consider setting the LOCKBOOK_PATH environment variable.".to_string()),
    }
}
