extern crate base64;
extern crate hyper;
extern crate lockbook_core;
extern crate tokio;

pub mod api;
pub mod config;
pub mod endpoint;
pub mod files_db;
pub mod index_db;
pub mod server;
pub mod services;

use crate::server::Server;

type DefaultLogger = lockbook_core::service::logging_service::ConditionalStdOut;
type DefaultChangeFileContentEndpoint = services::change_file_content::EndpointServiceImpl;
type DefaultCreateFileEndpoint = services::create_file::EndpointServiceImpl;
type DefaultDeleteFileEndpoint = services::delete_file::EndpointServiceImpl;
type DefaultGetUpdatesEndpoint = services::get_updates::EndpointServiceImpl;
type DefaultMoveFileEndpoint = services::move_file::EndpointServiceImpl;
type DefaultNewAccountEndpoint = services::new_account::EndpointServiceImpl;
type DefaultRenameFileEndpoint = services::rename_file::EndpointServiceImpl;
type DefaultApi = api::ApiImpl<
    DefaultLogger,
    DefaultChangeFileContentEndpoint,
    DefaultCreateFileEndpoint,
    DefaultDeleteFileEndpoint,
    DefaultGetUpdatesEndpoint,
    DefaultMoveFileEndpoint,
    DefaultNewAccountEndpoint,
    DefaultRenameFileEndpoint,
>;
type DefaultServer = server::ServerImpl<DefaultApi>;

fn main() {
    DefaultServer::run().unwrap();
}
