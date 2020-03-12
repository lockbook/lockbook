use crate::config::IndexDbConfig;
use openssl::error::ErrorStack as OpenSslError;
use openssl::ssl::{SslConnector, SslMethod};
use postgres::config::Config as PostgresConfig;
use postgres::Client as PostgresClient;
use postgres::NoTls;
use postgres_openssl::MakeTlsConnector;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;

#[derive(Debug)]
pub enum Error {
    OpenSslFailed(OpenSslError),
    PostgresConnectionFailed(PostgresError),
    PostgresPortNotU16(String),
}

pub fn connect(config: &IndexDbConfig) -> Result<PostgresClient, Error> {
    Ok(())
        .and_then(|_| match config.port.parse() {
            Ok(port) => {
                let mut postgres_config = PostgresConfig::new();
                postgres_config
                    .user(config.user)
                    .host(config.host)
                    .password(config.pass)
                    .port(port)
                    .dbname(config.db);
                Ok(postgres_config)
            }
            Err(err) => Err(Error::PostgresPortNotU16(err.to_string())),
        })
        .and_then(|postgres_config| match config.cert {
            "" => connect_no_tls(&postgres_config),
            cert => connect_with_tls(&postgres_config, &cert),
        })
}

fn connect_no_tls(postgres_config: &PostgresConfig) -> Result<PostgresClient, Error> {
    match postgres_config.connect(NoTls) {
        Ok(connection) => Ok(connection),
        Err(err) => Err(Error::PostgresConnectionFailed(err)),
    }
}

fn connect_with_tls(postgres_config: &PostgresConfig, cert: &str) -> Result<PostgresClient, Error> {
    Ok(())
        .and_then(|_| match SslConnector::builder(SslMethod::tls()) {
            Ok(builder) => Ok(builder),
            Err(err) => Err(Error::OpenSslFailed(err)),
        })
        .and_then(|mut builder| match builder.set_ca_file(cert) {
            Ok(()) => Ok(builder),
            Err(err) => Err(Error::OpenSslFailed(err)),
        })
        .and_then(
            |builder| match postgres_config.connect(MakeTlsConnector::new(builder.build())) {
                Ok(connection) => Ok(connection),
                Err(err) => Err(Error::PostgresConnectionFailed(err)),
            },
        )
}
