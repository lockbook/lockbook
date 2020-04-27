use crate::index_db::get_public_key::Error::Postgres;
use postgres::Client as PostgresClient;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
    SerializationError(serde_json::Error),
}

pub fn get_public_key(client: &mut PostgresClient, username: &String) -> Result<String, Error> {
    match client.query_one(
        "SELECT public_key FROM users WHERE username = $1;",
        &[&username],
    ) {
        Ok(row) => Ok(row.get("public_key")),
        Err(e) => Err(Error::Postgres(e)),
    }
}
