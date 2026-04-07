use tracing::{Level, info, span};
use warp::{Filter, http::Response, hyper::Body};

const APPLE_APP_SITE_ASSOCIATION: &str = include_str!("../static/apple-app-site-association");

pub fn static_routes() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    open_route().or(well_known_route())
}

fn open_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("open")
        .and(warp::path::param::<String>())
        .map(|uuid: String| {
            let span = span!(Level::INFO, "matched_request", method = "GET", route = "/open");
            let _enter = span.enter();

            info!("external link routed");

            let redirect_html = get_files_preview_html(&uuid);
            warp::reply::html(redirect_html)
        })
}

fn well_known_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".well-known")
        .and(warp::path("apple-app-site-association"))
        .map(|| {
            Response::builder()
                .header("Content-Type", "application/json")
                .body(Body::from(APPLE_APP_SITE_ASSOCIATION))
                .unwrap()
        })
}

pub fn get_files_preview_html(uuid: &str) -> String {
    format!(
        r#"
<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>Open in Lockbook</title>
        <script>
            window.onload = function() {{
                const url = "lb://{uuid}";

                // Try to open the app
                window.location.href = url;
            }};
        </script>
    </head>
    <body>
        <h1>Opening Lockbook...</h1>
        <p>If nothing happens, use the link below:</p>
        <div>
            <a href="lb://{uuid}">Open in App</a>
        </div>
    </body>
</html>
"#,
        uuid = uuid
    )
}
