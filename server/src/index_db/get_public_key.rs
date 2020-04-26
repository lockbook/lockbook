use postgres::Client as PostgresClient;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
}

pub fn get_public_key(client: &mut PostgresClient, username: &String) -> Result<String, Error> {
    match client.query_one(
        "SELECT public_key FROM files WHERE username = $1;",
        &[&username],
    ) {
        Ok(row) => Ok(row.get("public_key")),
        Err(err) => Err(Error::Postgres(err)),
    }
}
