use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PostgresClient;

#[derive(Debug)]
pub enum Error {
    UsernameTaken,
    InvalidUsername,
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

pub async fn new_account(
    client: &mut PostgresClient,
    username: &String,
    public_key: &String,
) -> Result<(), Error> {

    if !username.chars().all(char::is_alphanumeric) {
        return Err(Error::InvalidUsername);
    }

    client
        .execute(
            "INSERT INTO users (username, public_key) VALUES ($1, $2);",
            &[&username.to_lowercase(), &public_key],
        )
        .await?;
    Ok(())
}
