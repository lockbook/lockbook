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
}

pub fn connect(config: &IndexDbConfig) -> Result<PostgresClient, Error> {
    let mut postgres_config = PostgresConfig::new();
    postgres_config
        .user(config.user)
        .host(config.host)
        .password(config.pass)
        .port(config.port)
        .dbname(config.db);

    match config.cert {
        "" => connect_no_tls(&postgres_config),
        cert => connect_with_tls(&postgres_config, &cert),
    }
}

fn connect_no_tls(postgres_config: &PostgresConfig) -> Result<PostgresClient, Error> {
    match postgres_config.connect(NoTls) {
        Ok(connection) => Ok(connection),
        Err(err) => Err(Error::PostgresConnectionFailed(err)),
    }
}

fn connect_with_tls(postgres_config: &PostgresConfig, cert: &str) -> Result<PostgresClient, Error> {
    let mut builder = match SslConnector::builder(SslMethod::tls()) {
        Ok(builder) => builder,
        Err(err) => return Err(Error::OpenSslFailed(err)),
    };
    match builder.set_ca_file(cert) {
        Ok(()) => {},
        Err(err) => return Err(Error::OpenSslFailed(err)),
    };
    match postgres_config.connect(MakeTlsConnector::new(builder.build())) {
        Ok(connection) => Ok(connection),
        Err(err) => Err(Error::PostgresConnectionFailed(err)),
    }
}
