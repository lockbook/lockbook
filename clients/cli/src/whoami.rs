use crate::utils::get_account_or_exit;

pub fn whoami() {
    println!("{}", get_account_or_exit().username)
}
