use crate::Subcommands::DeleteUser;
use structopt::StructOpt;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A utility for a lockbook server administrator.")]
enum Subcommands {
    /// Purge a user, and all their files from postgres & s3
    DeleteUser { user: String },
}

fn main() {
    match Subcommands::from_args() {
        DeleteUser { user } => println!("deleting: {}", user),
    }
}
