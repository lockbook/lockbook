// TODO this is going to in large part replace router_service in a future PR
use crate::{file_service, RequestContext, ServerError};
use async_trait::async_trait;
use lockbook_shared::api::{Request, UpsertRequest};

#[async_trait]
pub trait HandledRequest: Request + Sized {
    async fn handle(
        req_cxt: RequestContext<'async_trait, Self>,
    ) -> Result<Self::Response, ServerError<Self::Error>>;
}

#[async_trait]
impl HandledRequest for UpsertRequest {
    async fn handle(
        req_cxt: RequestContext<'async_trait, Self>,
    ) -> Result<Self::Response, ServerError<Self::Error>> {
        file_service::upsert_file_metadata(req_cxt).await
    }
}
