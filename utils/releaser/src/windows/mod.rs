mod cli;

use crate::Github;

pub fn release(gh: &Github) {
    cli::release(gh);
}
