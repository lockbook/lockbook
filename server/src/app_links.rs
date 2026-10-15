use tracing::{Level, info, span};
use uuid::Uuid;
use warp::Filter;

const OPEN_HTML: &str = include_str!("../static/open.html");

pub fn app_link_routes(
    api_url: &str,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let api_url = api_url.trim_end_matches('/').to_owned();
    let open = warp::path!("open" / Uuid).map(move |uuid: Uuid| {
        let span = span!(Level::INFO, "matched_request", method = "GET", route = "/open");
        let _enter = span.enter();
        info!(%uuid, "external link routed");
        warp::reply::html(
            OPEN_HTML
                .replace("{{OPEN_URL}}", &format!("{api_url}/open/{uuid}"))
                .replace("{{HANDOFF}}", &format!("lb://{uuid}")),
        )
    });

    let apple = warp::path!(".well-known" / "apple-app-site-association").map(|| {
        warp::reply::with_header(
            &include_bytes!("../static/apple-app-site-association")[..],
            "Content-Type",
            "application/json",
        )
    });
    let android = warp::path!(".well-known" / "assetlinks.json").map(|| {
        warp::reply::with_header(
            &include_bytes!("../static/assetlinks.json")[..],
            "Content-Type",
            "application/json",
        )
    });

    open.or(apple).or(android)
}
