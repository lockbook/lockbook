use rsa::RSAPublicKey;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
    SerializationError(serde_json::Error),
}

pub async fn get_public_key(
    transaction: &Transaction<'_>,
    username: &String,
) -> Result<RSAPublicKey, Error> {
    match transaction
        .query_one(
            "SELECT public_key FROM users WHERE username = $1;",
            &[&username],
        )
        .await
    {
        Ok(row) => {
            Ok(serde_json::from_str(row.get("public_key")).map_err(Error::SerializationError)?)
        }
        Err(e) => Err(Error::Postgres(e)),
    }
}
