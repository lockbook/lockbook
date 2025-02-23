use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::FileType;
use lb_rs::service::search::{SearchConfig, SearchResult};
use std::collections::HashSet;
use test_utils::*;
use tokio::time;
use web_time::Duration;

const FILE_PATHS: [&str; 6] =
    ["/abc.md", "/abcd.md", "/abcde.md", "/dir/doc1", "/dir/doc2", "/dir/doc3"];

const MATCHED_PATHS_1: (&str, [&str; 3]) = ("a", ["/abc.md", "/abcd.md", "/abcde.md"]);

const MATCHED_PATHS_2: (&str, [&str; 4]) =
    ("dir", ["/dir/", "/dir/doc1", "/dir/doc2", "/dir/doc3"]);

const MATCHED_PATHS_3: (&str, [&str; 1]) = ("bbbb", ["/bbbbbbb.md"]);

const CONTENT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus \
lorem purus, malesuada a dui a, auctor lobortis dolor. Proin ut placerat lectus. Vestibulum massa \
orci, fermentum id nunc sit amet, scelerisque tempus enim. Duis tristique imperdiet ex. Curabitur \
sagittis augue vel orci eleifend, sed cursus ante porta. Phasellus pellentesque vulputate ante id \
fringilla. Suspendisse eu volutpat augue. Mauris massa nisl, venenatis eget viverra non, ultrices \
vel enim.";

const MATCHED_CONTENT_1: (&str, &str) = (
    "consectetur adipiscing elit. Vivamus lorem purus, malesuada",
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus lorem purus, \
malesuada a dui a, auctor lobortis dolor. Proin ut placerat lectus. Vestibulum m...",
);

const MATCHED_CONTENT_2: (&str, &str) = (
    "Mauris massa nisl, venenatis eget viverra",
    ".... Phasellus pellentesque vulputate ante id fringilla. Suspendisse eu volutpat augue. \
    Mauris massa nisl, venenatis eget viverra non, ultrices vel enim.",
);

const MATCHED_CONTENT_3: (&str, &str) = (
    "scelerisque tempus",
    "...t amet, scelerisque tempus enim. Duis tristique imperdiet ex. Curabitur sagittis augue \
    vel orci eleifend, sed cursus ante porta. Phasellus pellente...",
);

#[tokio::test]
async fn search_paths_successfully() {
    let core = test_core_with_account().await;

    for file_path in FILE_PATHS {
        core.create_at_path(file_path).await.unwrap();
    }

    core.build_index().await.unwrap();

    let search1 = core.search("", SearchConfig::Paths).await.unwrap();
    assert_eq!(search1.len(), 0);

    let matched_paths_1: HashSet<_> = MATCHED_PATHS_1.1.iter().collect();
    let search2 = core
        .search(MATCHED_PATHS_1.0, SearchConfig::Paths)
        .await
        .unwrap();
    assert_eq!(search2.len(), MATCHED_PATHS_1.1.len());

    for result in search2 {
        if let SearchResult::PathMatch { path, .. } = result {
            assert!(
                matched_paths_1.contains(&path.as_str()),
                "A path from the first set didn't match."
            );
        } else {
            panic!("Non-path search result.")
        }
    }

    let matched_paths_2: HashSet<_> = MATCHED_PATHS_2.1.iter().collect();
    let search3 = core
        .search(MATCHED_PATHS_2.0, SearchConfig::Paths)
        .await
        .unwrap();
    assert_eq!(search3.len(), MATCHED_PATHS_2.1.len());

    for result in search3 {
        if let SearchResult::PathMatch { path, .. } = result {
            assert!(
                matched_paths_2.contains(&path.as_str()),
                "A path from the second set didn't match: {}",
                path
            );
        } else {
            panic!("Non-path search result.")
        }
    }
}

#[tokio::test]
async fn search_content_successfully() {
    let core = test_core_with_account().await;

    let file = core.create_at_path("/aaaaaaaaaa.md").await.unwrap();
    core.write_document(file.id, CONTENT.as_bytes())
        .await
        .unwrap();

    time::sleep(Duration::from_millis(10)).await;
    core.build_index().await.unwrap();

    let search1 = core
        .search("", SearchConfig::PathsAndDocuments)
        .await
        .unwrap();

    assert_eq!(search1.len(), 1);

    let results1 = core
        .search(MATCHED_CONTENT_1.0, SearchConfig::PathsAndDocuments)
        .await
        .unwrap();
    assert_eq!(results1.len(), 1);
    if let SearchResult::DocumentMatch { content_matches, .. } = &results1[0] {
        assert!(content_matches[0].paragraph == MATCHED_CONTENT_1.1)
    } else {
        panic!("Search result was not a document match.")
    }

    let results2 = core
        .search(MATCHED_CONTENT_2.0, SearchConfig::PathsAndDocuments)
        .await
        .unwrap();
    assert_eq!(results2.len(), 1);
    if let SearchResult::DocumentMatch { content_matches, .. } = &results2[0] {
        assert!(content_matches[0].paragraph == MATCHED_CONTENT_2.1)
    } else {
        panic!("Search result was not a document match.")
    }

    let results3 = core
        .search(MATCHED_CONTENT_3.0, SearchConfig::PathsAndDocuments)
        .await
        .unwrap();
    assert_eq!(results3.len(), 1);
    if let SearchResult::DocumentMatch { content_matches, .. } = &results3[0] {
        assert!(content_matches[0].paragraph == MATCHED_CONTENT_3.1)
    } else {
        panic!("Search result was not a document match.")
    }
}

#[tokio::test]
async fn search_exclude_pending_share() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_with_account().await;

    let file1 = core1.create_at_path("/aaaaaaa.md").await.unwrap();
    core1
        .write_document(file1.id, CONTENT.as_bytes())
        .await
        .unwrap();
    core1
        .share_file(file1.id, &core2.get_account().unwrap().username, ShareMode::Read)
        .await
        .unwrap();

    core1.sync(None).await.unwrap();
    core2.sync(None).await.unwrap();

    core2.create_at_path("/bbbbbbb.md").await.unwrap();
    core2
        .create_file(
            &file1.name,
            &core2.root().await.unwrap().id,
            FileType::Link { target: file1.id },
        )
        .await
        .unwrap();

    core2.build_index().await.unwrap();

    let search1 = core2
        .search("", SearchConfig::PathsAndDocuments)
        .await
        .unwrap();
    assert_eq!(search1.len(), 0);

    let search2 = core2
        .search(MATCHED_PATHS_3.0, SearchConfig::PathsAndDocuments)
        .await
        .unwrap();
    assert_eq!(search2.len(), 1);
    if let SearchResult::PathMatch { path, .. } = &search2[0] {
        assert!(path == MATCHED_PATHS_3.1[0])
    } else {
        panic!("Search result was not a path match.")
    }
}
