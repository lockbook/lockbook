use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::error::SqlState;
use tokio_postgres::Transaction;

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

pub async fn new_account(
    transaction: &Transaction<'_>,
    username: &String,
    public_key: &String,
) -> Result<(), Error> {
    transaction
        .execute(
            "INSERT INTO users (username, public_key) VALUES ($1, $2);",
            &[&username, &public_key],
        )
        .await?;
    Ok(())
}
