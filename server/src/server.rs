use crate::api::Api;
use crate::config::config;
use crate::files_db;
use crate::index_db;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request};
use std::convert::Infallible;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

pub trait Server {
    fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct ServerImpl<ApiImpl: Api> {
    api: PhantomData<ApiImpl>,
}

pub struct ServerState {
    pub index_db_client: postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
}

impl<ApiImpl: Api> Server for ServerImpl<ApiImpl> {
    #[tokio::main]
    async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = config();
        let index_db_client = match index_db::connect(&config.index_db_config) {
            Ok(client) => client,
            Err(err) => panic!("{:?}", err),
        };
        let files_db_client = match files_db::connect(&config.files_db_config) {
            Ok(x) => x,
            Err(err) => panic!("{:?}", err),
        };
        let server_state = Arc::new(Mutex::new(ServerState {
            index_db_client: index_db_client,
            files_db_client: files_db_client,
        }));
        let addr = "0.0.0.0:3000".parse()?;

        let make_service = make_service_fn(|_| {
            let server_state = server_state.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                    let server_state = server_state.clone();
                    async move { Ok::<_, Infallible>(ApiImpl::handle(server_state, req)) }
                }))
            }
        });

        hyper::Server::bind(&addr).serve(make_service).await?;
        Ok(())
    }
}
