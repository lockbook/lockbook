use postgres::Client as PostgresClient;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::error::SqlState;

#[derive(Debug)]
pub enum Error {
    UsernameTaken,
    Uninterpreted(PostgresError),
}

impl From<PostgresError> for Error {
    fn from(e: PostgresError) -> Error {
        match e.code() {
            Some(x) if x == &SqlState::UNIQUE_VIOLATION => Error::UsernameTaken,
            _ => Error::Uninterpreted(e),
        }
    }
}

pub fn create_user(
    client: &mut PostgresClient,
    username: &String,
    pub_key_n: &String,
    pub_key_e: &String,
) -> Result<(), Error> {
    client.execute(
        "INSERT INTO users (username, pub_key_n, pub_key_e) VALUES ($1, $2, $3);",
        &[&username, &pub_key_n, &pub_key_e],
    )?;
    Ok(())
}
