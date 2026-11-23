use tracing::{Level, info, span};
use warp::Filter;

const OPEN_HTML: &str = include_str!("../static/open.html");

pub fn app_link_routes() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    // the page builds its `lb://` twin from its own address, so it serves
    // every kind of place and the fragment never reaches here
    let open = warp::path("open")
        .and(warp::path::tail())
        .map(|place: warp::path::Tail| {
            let span = span!(Level::INFO, "matched_request", method = "GET", route = "/open");
            let _enter = span.enter();
            info!(place = place.as_str(), "external link routed");
            warp::reply::html(OPEN_HTML)
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

#[cfg(test)]
mod tests {
    use super::app_link_routes;

    /// Any kind of place gets the page, and nothing is left to fill in.
    #[tokio::test]
    async fn every_place_gets_the_page() {
        for path in ["/open/8d3b2a64-1f5e-4b8e-9c4a-2f0e6b7d9a10", "/open/search"] {
            let res = warp::test::request()
                .path(path)
                .reply(&app_link_routes())
                .await;
            assert_eq!(res.status(), 200, "{path}");
            assert!(!String::from_utf8_lossy(res.body()).contains("{{"), "{path}");
        }
    }
}
