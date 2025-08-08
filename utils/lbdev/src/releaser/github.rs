use super::secrets::Github;
use super::utils;
use super::utils::{lb_repo, lb_version};
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use gh_release::release::{CreateReleaseInfo, TagInfo};

pub fn create_release() -> CliResult<()> {
    let gh = Github::env();

    let client = ReleaseClient::new(gh.0).unwrap();

    let tag_info = TagInfo {
        tag: lb_version(),
        message: "".to_string(),
        object: utils::commit_hash(),
        type_tagged: "commit".to_string(),
    };

    client.create_a_tag(&lb_repo(), &tag_info).unwrap();

    let release_info = CreateReleaseInfo {
        tag_name: lb_version(),
        target_commitish: None,
        name: Some(lb_version()),
        body: None,
        draft: None,
        prerelease: None,
        discussion_category_name: None,
        generate_release_notes: Some(true),
        make_latest: Some("true".to_string()),
    };

    client.create_a_release(&lb_repo(), &release_info).unwrap();
    Ok(())
}
