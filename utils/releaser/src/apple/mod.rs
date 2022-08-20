mod cli;

use crate::Secrets;

pub fn release_apple(secret: Secrets) {
    cli::release(secret);
    // (todo)
    // release_ios();
    // release_macos();
}
