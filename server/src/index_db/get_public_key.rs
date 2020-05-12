use postgres::Client as PostgresClient;
use rsa::RSAPublicKey;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
    SerializationError(serde_json::Error),
}

pub fn get_public_key(
    client: &mut PostgresClient,
    username: &String,
) -> Result<RSAPublicKey, Error> {
    match client.query_one(
        "SELECT public_key FROM users WHERE username = $1;",
        &[&username],
    ) {
        Ok(row) => Ok(serde_json::from_str(row.get("public_key"))
            .map_err(|err| Error::SerializationError(err))?),
        Err(e) => Err(Error::Postgres(e)),
    }
}
