use crate::config::IndexDbConfig;
use openssl::error::ErrorStack as OpenSslError;
use openssl::ssl::{SslConnector, SslMethod};
use postgres_openssl::MakeTlsConnector;
use tokio_postgres;
use tokio_postgres::config::Config as PostgresConfig;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Client as PostgresClient;
use tokio_postgres::NoTls;

#[derive(Debug)]
pub enum Error {
    OpenSslFailed(OpenSslError),
    PostgresConnectionFailed(PostgresError),
}

pub async fn connect(config: &IndexDbConfig) -> Result<PostgresClient, Error> {
    let postgres_config = match config.port.parse() {
        Ok(port) => {
            let mut postgres_config = PostgresConfig::new();
            postgres_config
                .user(config.user)
                .host(config.host)
                .password(config.pass)
                .port(port)
                .dbname(config.db);
            postgres_config
        }
        Err(err) => return Err(Error::PostgresPortNotU16(err)),
    };

    match config.cert {
        "" => connect_no_tls(&postgres_config).await,
        cert => connect_with_tls(&postgres_config, &cert).await,
    }
}

async fn connect_no_tls(postgres_config: &PostgresConfig) -> Result<PostgresClient, Error> {
    match postgres_config.connect(NoTls).await {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    panic!("connection error: {}", e);
                }
            });
            Ok(client)
        }
        Err(err) => Err(Error::PostgresConnectionFailed(err)),
    }
}

async fn connect_with_tls(
    postgres_config: &PostgresConfig,
    cert: &str,
) -> Result<PostgresClient, Error> {
    let mut builder = match SslConnector::builder(SslMethod::tls()) {
        Ok(builder) => builder,
        Err(err) => return Err(Error::OpenSslFailed(err)),
    };
    builder
        .set_ca_file(cert)
        .map_err(|e| Error::OpenSslFailed(e))?;
    match postgres_config
        .connect(MakeTlsConnector::new(builder.build()))
        .await
    {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    panic!("connection error: {}", e);
                }
            });
            Ok(client)
        }
        Err(err) => Err(Error::PostgresConnectionFailed(err)),
    }
}
