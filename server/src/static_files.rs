use tracing::{Level, info, span};
use uuid::Uuid;
use warp::{Filter, http::Response, hyper::Body};

const APPLE_APP_SITE_ASSOCIATION: &str = include_str!("../static/apple-app-site-association");
const ANDROID_ASSET_LINKS: &str = include_str!("../static/assetlinks.json");
const OPEN_HTML: &str = include_str!("../static/open.html");
const LOCKBOOK_LOGO: &[u8] = include_bytes!("../static/favicon/web-app-manifest-512x512.png");

pub fn static_routes(
    public_url: &str,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let public_origin = canonical_https_origin(public_url).unwrap_or_else(|| {
        panic!("PUBLIC_URL must be an HTTPS origin without credentials, path, query, or fragment")
    });
    open_route(public_origin)
        .or(well_known_route())
        .or(logo_route())
        .or(mark_route())
        .or(preview_image_route())
        .or(favicon_route())
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
        .map(|| asset_response("image/png", LOCKBOOK_LOGO))
}

fn preview_image_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    warp::path("open-preview.png")
        .and(warp::path::end())
        .map(|| asset_response("image/png", include_bytes!("../static/open-preview.png")))
}

fn mark_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("lockbook-mark.svg")
        .and(warp::path::end())
        .map(|| asset_response("image/svg+xml", include_bytes!("../static/lockbook-mark.svg")))
}

fn favicon_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("favicon")
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and_then(|name: String| async move {
            let (content_type, bytes): (&str, &'static [u8]) = match name.as_str() {
                "favicon.svg" => ("image/svg+xml", include_bytes!("../static/favicon/favicon.svg")),
                "favicon.ico" => ("image/x-icon", include_bytes!("../static/favicon/favicon.ico")),
                "favicon-96x96.png" => {
                    ("image/png", include_bytes!("../static/favicon/favicon-96x96.png"))
                }
                "apple-touch-icon.png" => {
                    ("image/png", include_bytes!("../static/favicon/apple-touch-icon.png"))
                }
                "web-app-manifest-192x192.png" => {
                    ("image/png", include_bytes!("../static/favicon/web-app-manifest-192x192.png"))
                }
                "web-app-manifest-512x512.png" => ("image/png", LOCKBOOK_LOGO),
                "site.webmanifest" => (
                    "application/manifest+json",
                    include_bytes!("../static/favicon/site.webmanifest"),
                ),
                _ => return Err(warp::reject::not_found()),
            };
            Ok::<_, warp::Rejection>(asset_response(content_type, bytes))
        })
}

fn asset_response(content_type: &str, bytes: &'static [u8]) -> Response<Body> {
    Response::builder()
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=86400")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes))
        .unwrap()
}

fn json_response(body: &'static str) -> Response<Body> {
    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

pub fn get_files_preview_html(public_origin: &str, uuid: Uuid) -> String {
    let id = uuid.to_string();
    OPEN_HTML
        .replace("{{OPEN_URL}}", &format!("{public_origin}/open/{id}"))
        .replace("{{PREVIEW_IMAGE}}", &format!("{public_origin}/open-preview.png"))
        .replace("{{HANDOFF}}", &format!("lb://{id}"))
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
        assert!(body.contains("<title>Someone shared a note with you | Lockbook</title>"));
        assert!(body.contains("<meta property=\"og:title\" content=\"Someone shared a note with you\""));
        assert!(body.contains("<meta name=\"twitter:title\" content=\"Someone shared a note with you\""));
        assert!(!body.contains("class=\"eyebrow\""));
        assert!(body.contains("<main class=\"wrap hero\">"));
        assert!(!body.contains("class=\"card\""));
        assert!(body.contains("Someone shared a note with you."));
        assert!(!body.contains("Someone shared a Lockbook note with you"));
        assert!(body.contains("Open Lockbook to view it. Only people with access to the note can see its contents."));
        assert!(body.contains("End-to-end encrypted. This page does not reveal the note's contents."));
        assert!(!body.contains("sync and view"));
        assert!(body.contains(&format!(
            "<link rel=\"canonical\" href=\"https://notes.example.com/open/{ID}\""
        )));
        assert!(body.contains(&format!(
            "<meta property=\"og:url\" content=\"https://notes.example.com/open/{ID}\""
        )));
        assert!(body.contains(
            "<meta property=\"og:image\" content=\"https://notes.example.com/open-preview.png\""
        ));
        assert!(body.contains("<meta name=\"twitter:card\" content=\"summary_large_image\""));
        assert!(body.contains("<meta property=\"og:image:width\" content=\"1200\""));
        assert!(body.contains("<meta property=\"og:image:height\" content=\"630\""));
        assert!(body.contains("href=\"/favicon/site.webmanifest\""));
        assert!(body.contains("src=\"/lockbook-mark.svg\""));
        assert_eq!(body.matches(">Get Lockbook</a>").count(), 1);
        assert!(body.contains("href=\"https://lockbook.net/download/\""));
        assert!(!body.contains("<footer>"));
        assert!(body.contains("--lb-open-accent: #207fdf"));
        assert!(body.contains("--lb-open-accent: #66b2ff"));
        assert!(body.contains("--lb-open-bg: #fff"));
        assert!(!body.contains("var(--bg)"));
        assert!(!body.contains("var(--fg)"));
        assert!(!body.contains("id=\"theme-toggle\""));
        assert!(!body.contains("localStorage"));
        assert!(body.contains(&format!("lb://{ID}")));
        assert!(!body.contains("window.location"));
        assert!(!body.contains("{{"));
    }

    #[tokio::test]
    async fn ipv6_handoff_and_preview_keep_valid_authorities() {
        let response = warp::test::request()
            .path(&format!("/open/{ID}"))
            .reply(&static_routes("https://[::1]:8443/"))
            .await;
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(body.contains(&format!("lb://{ID}")));
        assert!(body.contains("https://[::1]:8443/open-preview.png"));
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

        for (path, content_type) in [
            ("/lockbook-logo.png", "image/png"),
            ("/lockbook-mark.svg", "image/svg+xml"),
            ("/open-preview.png", "image/png"),
            ("/favicon/favicon.svg", "image/svg+xml"),
            ("/favicon/favicon.ico", "image/x-icon"),
            ("/favicon/favicon-96x96.png", "image/png"),
            ("/favicon/apple-touch-icon.png", "image/png"),
            ("/favicon/web-app-manifest-192x192.png", "image/png"),
            ("/favicon/web-app-manifest-512x512.png", "image/png"),
            ("/favicon/site.webmanifest", "application/manifest+json"),
        ] {
            let response = warp::test::request()
                .path(path)
                .reply(&static_routes("https://example.com"))
                .await;
            assert_eq!(response.status(), 200, "{path}");
            assert_eq!(response.headers()["content-type"], content_type, "{path}");
            if content_type == "image/png" {
                assert!(response.body().starts_with(b"\x89PNG\r\n\x1a\n"), "{path}");
            }
        }
        let unknown = warp::test::request()
            .path("/favicon/unknown.png")
            .reply(&static_routes("https://example.com"))
            .await;
        assert_eq!(unknown.status(), 404);
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
