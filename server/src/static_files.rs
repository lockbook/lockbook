use tracing::{Level, info, span};
use uuid::Uuid;
use warp::{Filter, http::Response, hyper::Body};

const APPLE_APP_SITE_ASSOCIATION: &str = include_str!("../static/apple-app-site-association");
const ANDROID_ASSET_LINKS: &str = include_str!("../static/assetlinks.json");
const LOCKBOOK_LOGO: &[u8] =
    include_bytes!("../../public-site/static/favicon/web-app-manifest-512x512.png");

pub fn static_routes(
    public_url: &str,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let public_origin = canonical_https_origin(public_url).unwrap_or_else(|| {
        panic!("PUBLIC_URL must be an HTTPS origin without credentials, path, query, or fragment")
    });
    open_route(public_origin)
        .or(well_known_route())
        .or(logo_route())
}

fn open_route(
    public_origin: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("open")
        .and(warp::path::param::<Uuid>())
        .and(warp::path::end())
        .map(move |uuid: Uuid| {
            let span = span!(Level::INFO, "matched_request", method = "GET", route = "/open");
            let _enter = span.enter();
            info!(%uuid, "external link routed");
            warp::reply::html(get_files_preview_html(&public_origin, uuid))
        })
}

fn well_known_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".well-known")
        .and(
            warp::path("apple-app-site-association")
                .map(|| json_response(APPLE_APP_SITE_ASSOCIATION))
                .or(warp::path("assetlinks.json").map(|| json_response(ANDROID_ASSET_LINKS)))
                .unify(),
        )
        .and(warp::path::end())
}

fn logo_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("lockbook-logo.png")
        .and(warp::path::end())
        .map(|| {
            Response::builder()
                .header("Content-Type", "image/png")
                .header("Cache-Control", "public, max-age=86400")
                .body(Body::from(LOCKBOOK_LOGO))
                .unwrap()
        })
}

fn json_response(body: &'static str) -> Response<Body> {
    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

pub fn get_files_preview_html(public_origin: &str, uuid: Uuid) -> String {
    let uuid = uuid.to_string();
    let handoff = format!("lb://{uuid}");
    let logo = format!("{public_origin}/lockbook-logo.png");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Open shared note in Lockbook</title>
    <meta name="description" content="Someone shared a Lockbook note with you.">
    <meta property="og:title" content="Open shared note in Lockbook">
    <meta property="og:description" content="Someone shared a Lockbook note with you.">
    <meta property="og:type" content="website">
    <meta property="og:image" content="{logo}">
    <meta name="twitter:card" content="summary">
    <meta name="twitter:title" content="Open shared note in Lockbook">
    <meta name="twitter:description" content="Someone shared a Lockbook note with you.">
    <meta name="twitter:image" content="{logo}">
    <style>
        @font-face {{ font-family: Martian; src: url("https://lockbook.github.io/martian.woff2") format("woff2"); font-display: swap; }}
        :root {{ color-scheme: dark; font-family: Martian, ui-monospace, monospace; background: #101010; color: #f2f2f2; }}
        body {{ min-height: 100vh; margin: 0; display: grid; place-items: center; background: #101010; color: #f2f2f2; }}
        main {{ width: min(34rem, calc(100% - 3rem)); padding: 3rem 2.5rem; text-align: center; border: 1px solid #303030; background: #1a1a1a; box-shadow: 0 1.5rem 4rem #00000066; }}
        img {{ width: 5rem; height: 5rem; margin-bottom: 1rem; }}
        h1 {{ margin: 0 0 .75rem; font-size: clamp(1.5rem, 4vw, 2.25rem); letter-spacing: -.04em; }}
        p {{ line-height: 1.6; color: #bfbfbf; }}
        a {{ display: inline-block; margin-top: 1rem; padding: .85rem 1.35rem; border-radius: .15rem; background: #67e4b6; color: #101010; font-weight: 700; text-decoration: none; }}
        a:hover {{ background: #8af0ca; }}
        a:focus-visible {{ outline: 3px solid #67e4b6; outline-offset: 3px; }}
    </style>
</head>
<body>
    <main>
        <img src="{logo}" alt="Lockbook logo" width="80" height="80">
        <h1>A Lockbook note was shared with you</h1>
        <p>Open Lockbook to sync your account and view this access-controlled note.</p>
        <a href="{handoff}">Open in Lockbook</a>
    </main>
</body>
</html>"#,
    )
}

fn canonical_https_origin(value: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(value).ok()?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
    {
        return None;
    }
    Some(parsed.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "a6743b18-c7ef-4960-9825-8022e2fa5672";

    #[tokio::test]
    async fn valid_open_route_has_explicit_escaped_handoff_and_metadata() {
        let response = warp::test::request()
            .path(&format!("/open/{ID}"))
            .reply(&static_routes("https://Notes.Example.com:443/"))
            .await;
        assert_eq!(response.status(), 200);
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(body.contains("<title>Open shared note in Lockbook</title>"));
        assert!(body.contains("Someone shared a Lockbook note with you."));
        assert!(body.contains("https://notes.example.com/lockbook-logo.png"));
        assert!(body.contains(&format!("lb://{ID}")));
        assert!(!body.contains("window.location"));
    }

    #[tokio::test]
    async fn ipv6_handoff_and_preview_keep_valid_authorities() {
        let response = warp::test::request()
            .path(&format!("/open/{ID}"))
            .reply(&static_routes("https://[::1]:8443/"))
            .await;
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(body.contains(&format!("lb://{ID}")));
        assert!(body.contains("https://[::1]:8443/lockbook-logo.png"));
        assert!(!body.contains("[["));
    }

    #[tokio::test]
    async fn invalid_open_routes_are_not_found() {
        for path in [format!("/open/not-a-uuid"), format!("/open/{ID}/extra")] {
            let response = warp::test::request()
                .path(&path)
                .reply(&static_routes("https://example.com"))
                .await;
            assert_eq!(response.status(), 404);
        }
    }

    #[tokio::test]
    async fn association_files_are_json_without_redirects() {
        for path in ["/.well-known/apple-app-site-association", "/.well-known/assetlinks.json"] {
            let response = warp::test::request()
                .path(path)
                .reply(&static_routes("https://example.com"))
                .await;
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["content-type"], "application/json");
        }

        let logo = warp::test::request()
            .path("/lockbook-logo.png")
            .reply(&static_routes("https://example.com"))
            .await;
        assert_eq!(logo.status(), 200);
        assert_eq!(logo.headers()["content-type"], "image/png");
    }

    #[test]
    fn configured_origin_must_be_safe_https() {
        assert_eq!(
            canonical_https_origin("https://[::1]:8443/"),
            Some("https://[::1]:8443".into())
        );
        assert_eq!(canonical_https_origin("https://[::1]:443/"), Some("https://[::1]".into()));
        assert_eq!(
            canonical_https_origin("https://EXAMPLE.com:443/"),
            Some("https://example.com".into())
        );
        for invalid in [
            "http://example.com",
            "https://user@example.com",
            "https://example.com/path",
            "https://example.com/?query=true",
        ] {
            assert_eq!(canonical_https_origin(invalid), None);
        }
    }
}
