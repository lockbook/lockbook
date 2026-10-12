use std::path::Path;
use tracing::{Level, info, span};
use uuid::Uuid;
use warp::Filter;

const OPEN_HTML: &str = include_str!("../static/open.html");

pub fn static_routes(
    api_url: &str, static_dir: &Path,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let server_origin = canonical_api_origin(api_url).unwrap_or_else(|| {
        panic!("API_URL must be an HTTP(S) origin without credentials, path, query, or fragment")
    });
    assert!(static_dir.is_dir(), "static asset directory does not exist: {}", static_dir.display());
    open_route(server_origin)
        .or(well_known_route(static_dir))
        .or(logo_route(static_dir))
        .or(warp::fs::dir(static_dir.to_path_buf()).map(cached_file))
}

fn open_route(
    server_origin: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("open")
        .and(warp::path::param::<Uuid>())
        .and(warp::path::end())
        .map(move |uuid: Uuid| {
            let span = span!(Level::INFO, "matched_request", method = "GET", route = "/open");
            let _enter = span.enter();
            info!(%uuid, "external link routed");
            warp::reply::html(get_files_preview_html(&server_origin, uuid))
        })
}

fn well_known_route(
    static_dir: &Path,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".well-known")
        .and(
            warp::path("apple-app-site-association")
                .and(warp::fs::file(static_dir.join("apple-app-site-association")))
                .or(warp::path("assetlinks.json")
                    .and(warp::fs::file(static_dir.join("assetlinks.json"))))
                .unify(),
        )
        .and(warp::path::end())
        .map(|file| warp::reply::with_header(file, "Content-Type", "application/json"))
}

fn logo_route(
    static_dir: &Path,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("lockbook-logo.png")
        .and(warp::path::end())
        .and(warp::fs::file(static_dir.join("favicon/web-app-manifest-512x512.png")))
        .map(cached_file)
}

fn cached_file(file: warp::fs::File) -> impl warp::Reply {
    warp::reply::with_header(
        warp::reply::with_header(file, "Cache-Control", "public, max-age=86400"),
        "X-Content-Type-Options",
        "nosniff",
    )
}

pub fn get_files_preview_html(server_origin: &str, uuid: Uuid) -> String {
    let id = uuid.to_string();
    OPEN_HTML
        .replace("{{OPEN_URL}}", &format!("{server_origin}/open/{id}"))
        .replace("{{PREVIEW_IMAGE}}", &format!("{server_origin}/open-preview.png"))
        .replace("{{HANDOFF}}", &format!("lb://{id}"))
}

fn canonical_api_origin(value: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(value).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
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

    fn routes(
        api_url: &str,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        static_routes(api_url, Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/static")))
    }

    #[tokio::test]
    async fn valid_open_route_has_explicit_escaped_handoff_and_metadata() {
        let response = warp::test::request()
            .path(&format!("/open/{ID}"))
            .reply(&routes("https://Notes.Example.com:443/"))
            .await;
        assert_eq!(response.status(), 200);
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(body.contains("<title>"));
        assert!(body.contains("<meta property=\"og:title\""));
        assert!(body.contains("<meta name=\"twitter:title\""));
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
        assert!(body.contains("href=\"https://lockbook.net/download/\""));
        assert!(body.contains(&format!("href=\"lb://{ID}\"")));
        assert!(!body.contains("window.location"));
        assert!(!body.contains("{{"));
    }

    #[tokio::test]
    async fn ipv6_handoff_and_preview_keep_valid_authorities() {
        let response = warp::test::request()
            .path(&format!("/open/{ID}"))
            .reply(&routes("https://[::1]:8443/"))
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
                .reply(&routes("https://example.com"))
                .await;
            assert_eq!(response.status(), 404);
        }
    }

    #[tokio::test]
    async fn association_files_are_json_without_redirects() {
        for path in ["/.well-known/apple-app-site-association", "/.well-known/assetlinks.json"] {
            let response = warp::test::request()
                .path(path)
                .reply(&routes("https://example.com"))
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
                .reply(&routes("https://example.com"))
                .await;
            assert_eq!(response.status(), 200, "{path}");
            assert_eq!(response.headers()["content-type"], content_type, "{path}");
            if content_type == "image/png" {
                assert!(response.body().starts_with(b"\x89PNG\r\n\x1a\n"), "{path}");
            }
        }
        let unknown = warp::test::request()
            .path("/favicon/unknown.png")
            .reply(&routes("https://example.com"))
            .await;
        assert_eq!(unknown.status(), 404);
    }

    #[test]
    fn configured_api_origin_must_be_safe() {
        assert_eq!(canonical_api_origin("https://[::1]:8443/"), Some("https://[::1]:8443".into()));
        assert_eq!(canonical_api_origin("https://[::1]:443/"), Some("https://[::1]".into()));
        assert_eq!(
            canonical_api_origin("https://EXAMPLE.com:443/"),
            Some("https://example.com".into())
        );
        assert_eq!(
            canonical_api_origin("http://localhost:8000"),
            Some("http://localhost:8000".into())
        );
        for invalid in [
            "ftp://example.com",
            "https://user@example.com",
            "https://example.com/path",
            "https://example.com/?query=true",
        ] {
            assert_eq!(canonical_api_origin(invalid), None);
        }
    }
}
