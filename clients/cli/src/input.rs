use std::{
    fmt::{Debug, Display},
    io::{self, Write},
    str::FromStr,
};

use cli_rs::cli_error::CliError;

pub fn std_in<T>(prompt: impl Display) -> Result<T, CliError>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .expect("failed to read from stdin");
    answer.retain(|c| c != '\n' && c != '\r');

    Ok(answer.parse::<T>().unwrap())
}
