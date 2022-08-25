use crate::secrets::PlayStore;
use crate::utils::{core_version, lb_repo, CommandRunner};
use crate::Github;
use gh_release::ReleaseClient;
use google_androidpublisher3::api::AppEdit;
use google_androidpublisher3::{hyper, hyper_rustls, oauth2, AndroidPublisher};
use std::fs;
use std::fs::File;
use std::process::Command;

mod core;

const OUTPUTS: &str = "clients/android/app/build/outputs";
const PACKAGE: &str = "app.lockbook";

pub fn release_android(gh: &Github) {
    core::build_libs();
    build_android();
    release_gh(gh);
    release_play_store();
}

fn build_android() {
    Command::new("gradlew")
        .args(["assembleRelease"])
        .current_dir("clients/android")
        .assert_success();

    Command::new("gradlew")
        .args(["bundleRelease"])
        .current_dir("clients/android")
        .assert_success();

    fs::rename(
        format!("{OUTPUTS}/apk/release/app-release.apk"),
        format!("{OUTPUTS}/apk/release/lockbook-android.apk"),
    )
    .unwrap();
}

fn release_gh(gh: &Github) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();
    let file = File::open(format!("{OUTPUTS}/apk/release/lockbook-android.apk")).unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id as u64,
            "lockbook-android.apk",
            "application/vnd.android.package-archive",
            file,
            None,
        )
        .unwrap();
}

fn release_play_store() {
    let service_account_key: oauth2::ServiceAccountKey =
        oauth2::parse_service_account_key(&PlayStore::env().0).unwrap();

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

    let edit = publisher
        .edits()
        .insert(AppEdit::default(), PACKAGE)
        .doit()
        .await
        .unwrap();

    publisher.edits().bundles_upload(PACKAGE, edit)
}
