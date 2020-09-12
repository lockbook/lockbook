use crate::utils::prepare_db_and_get_account_or_exit;

pub fn whoami() {
    println!("{}", prepare_db_and_get_account_or_exit().username)
}
