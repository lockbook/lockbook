use crate::secrets::*;
use crate::utils::{CommandRunner, android_version_code, lb_repo, lb_version};
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use google_androidpublisher3::api::{AppEdit, LocalizedText, Track, TrackRelease};
use google_androidpublisher3::{AndroidPublisher, hyper, hyper_rustls, oauth2};
use std::fs::File;
use std::process::Command;
use tokio::runtime::Runtime;

mod ws;

const OUTPUTS: &str = "clients/android/app/build/outputs";
const PACKAGE: &str = "app.lockbook";
const RELEASE: &str = "release/app-release";

const RELEASES: &str = "https://github.com/lockbook/lockbook/releases/tag";

const TRACK: &str = "production";
const STATUS: &str = "completed";
const DEFAULT_LOC: &str = "en-US";
const MIME: &str = "application/octet-stream";

pub fn release() -> CliResult<()> {
    // core::build_libs();
    ws::build();
    build_android();
    release_gh();
    release_play_store();
    Ok(())
}

fn build_android() {
    Command::new("./gradlew")
        .args(["assembleRelease"])
        .current_dir("clients/android")
        .assert_success();

    Command::new("./gradlew")
        .args(["bundleRelease"])
        .current_dir("clients/android")
        .assert_success();
}

fn release_gh() {
    let gh = Github::env();
    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
        .unwrap();
    let file = File::open(format!("{OUTPUTS}/apk/{RELEASE}.apk")).unwrap();

    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-android.apk",
            "application/vnd.android.package-archive",
            file,
            None,
        )
        .unwrap();
}

fn release_play_store() {
    let ps = PlayStore::env();
    let service_account_key: oauth2::ServiceAccountKey =
        oauth2::parse_service_account_key(ps.service_account_key).unwrap();

    let runtime = Runtime::new().unwrap();

    runtime.block_on(async move {
        let auth = oauth2::ServiceAccountAuthenticator::builder(service_account_key)
            .build()
            .await
            .unwrap();

        let client = hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::with_native_roots(Default::default())
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build(),
        );

        let publisher = AndroidPublisher::new(client, auth);

        let app_edit = publisher
            .edits()
            .insert(AppEdit::default(), PACKAGE)
            .doit()
            .await
            .unwrap()
            .1;

        let id = app_edit.id.unwrap();

        publisher
            .edits()
            .bundles_upload(PACKAGE, &id)
            .upload(
                File::open(format!("{OUTPUTS}/bundle/{RELEASE}.aab")).unwrap(),
                MIME.parse().unwrap(),
            )
            .await
            .unwrap();

        publisher
            .edits()
            .tracks_update(
                Track {
                    releases: Some(vec![TrackRelease {
                        country_targeting: None,
                        in_app_update_priority: None,
                        name: None,
                        release_notes: Some(vec![LocalizedText {
                            language: Some(DEFAULT_LOC.to_string()),
                            text: Some(format!("Release notes on {}/{}", RELEASES, lb_version())),
                        }]),
                        status: Some(STATUS.to_string()),
                        user_fraction: None,
                        version_codes: Some(vec![android_version_code()]),
                    }]),
                    track: Some(TRACK.to_string()),
                },
                PACKAGE,
                &id,
                TRACK,
            )
            .doit()
            .await
            .unwrap();

        publisher.edits().commit(PACKAGE, &id).doit().await.unwrap();
    });
}
