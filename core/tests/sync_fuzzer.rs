mod integration_test;

#[cfg(test)]
mod sync_fuzzer {
    use std::cmp::Ordering;

    use indicatif::{ProgressBar, ProgressStyle};
    use rand::distributions::{Alphanumeric, Distribution, Standard};
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use variant_count::VariantCount;

    use lockbook_core::model::client_conversion::ClientFileMetadata;
    use lockbook_core::model::state::Config;
    use lockbook_core::service::account_service::{create_account, export_account, import_account};
    use lockbook_core::service::integrity_service::test_repo_integrity;
    use lockbook_core::service::sync_service::sync;
    use lockbook_core::service::test_utils::{assert_dbs_eq, random_username, test_config, url};
    use lockbook_core::Error::UiError;
    use lockbook_core::{
        calculate_work, create_file, delete_file, get_path_by_id, list_metadatas, move_file,
        rename_file, write_document, MoveFileError,
    };
    use lockbook_models::file_metadata::FileType::{Document, Folder};

    use crate::sync_fuzzer::Actions::{
        AttemptFolderMove, DeleteFile, MoveDocument, NewFolder, NewMarkdownDocument, RenameFile,
        SyncAndCheck, UpdateDocument,
    };

    /// Starting parameters that matter
    static SEED: u64 = 0;
    static CLIENTS: u8 = 2;
    static ACTION_COUNT: u64 = 250;
    static MAX_FILE_SIZE: usize = 1024;
    ///
    static SHOW_PROGRESS: bool = false;

    /// If you add a variant here, make sure you add the corresponding entry for random selection
    /// See `impl Distribution<Actions> for Standard`
    #[derive(VariantCount, Debug)]
    enum Actions {
        SyncAndCheck,
        NewFolder,
        NewMarkdownDocument,
        UpdateDocument,
        MoveDocument,
        AttemptFolderMove,
        RenameFile,
        DeleteFile,
    }

    #[test]
    #[ignore]
    /// Run with: cargo test --release stress_test_sync -- --nocapture --ignored
    fn stress_test_sync() {
        println!("seed: {}", SEED);
        println!("clients: {}", CLIENTS);

        let mut rng = StdRng::seed_from_u64(SEED);
        let clients = create_clients();

        let pb = setup_progress_bar();
        for event_id in 0..ACTION_COUNT {
            let action = rng.gen::<Actions>();
            if SHOW_PROGRESS {
                pb.set_message(format!("{}: {:?}", event_id, action));
                pb.inc(1)
            } else {
                print!("\n{}: {:?}\t", event_id, action);
                match action {
                    NewFolder | RenameFile | DeleteFile => print!("\t"),
                    SyncAndCheck => println!(),
                    _ => {}
                };
            }
            action.execute(&clients, &mut rng);
        }
    }

    impl Actions {
        fn execute(&self, clients: &[Config], rng: &mut StdRng) {
            match &self {
                SyncAndCheck => {
                    for _ in 0..2 {
                        for client in clients {
                            sync(client, None).unwrap()
                        }
                    }

                    for row in clients {
                        for col in clients {
                            assert_dbs_eq(row, col);
                        }
                        test_repo_integrity(row).unwrap();
                        assert!(calculate_work(row).unwrap().local_files.is_empty());
                        assert!(calculate_work(row).unwrap().server_files.is_empty());
                        assert_eq!(calculate_work(row).unwrap().server_unknown_name_count, 0);
                    }
                }
                NewFolder => {
                    let client = Self::random_client(clients, rng);
                    let parent = Self::pick_random_parent(&client, rng);
                    let name = Self::random_filename(rng);
                    let file = create_file(&client, &name, parent.id, Folder).unwrap();
                    print!(
                        "[{:?}]\t{:?}",
                        file.id,
                        get_path_by_id(&client, file.id).unwrap()
                    );
                }
                NewMarkdownDocument => {
                    let client = Self::random_client(clients, rng);
                    let parent = Self::pick_random_parent(&client, rng);
                    let name = Self::random_filename(rng) + ".md"; // TODO pick a random extension (or no extension)
                    let file = create_file(&client, &name, parent.id, Document).unwrap();
                    print!(
                        "[{:?}]\t{:?}",
                        file.id,
                        get_path_by_id(&client, file.id).unwrap()
                    );
                }
                UpdateDocument => {
                    let client = Self::random_client(clients, rng);
                    if let Some(file) = Self::pick_random_document(&client, rng) {
                        let new_content = Self::random_utf8(rng);
                        write_document(&client, file.id, &new_content.as_bytes()).unwrap();
                        print!(
                            "[{:?}]\t{:?}",
                            file.id,
                            get_path_by_id(&client, file.id).unwrap()
                        );
                    }
                }
                MoveDocument => {
                    let client = Self::random_client(clients, rng);
                    if let Some(file) = Self::pick_random_document(&client, rng) {
                        let new_parent = Self::pick_random_parent(&client, rng);
                        if file.parent != new_parent.id && file.id != new_parent.id {
                            let initial_path = get_path_by_id(&client, file.id).unwrap();
                            move_file(&client, file.id, new_parent.id).unwrap();
                            print!(
                                "[{:?}]\t{:?} to {:?}",
                                file.id,
                                initial_path,
                                get_path_by_id(&client, file.id).unwrap()
                            );
                        }
                    }
                }
                AttemptFolderMove => {
                    let client = Self::random_client(clients, rng);
                    if let Some(file) = Self::pick_random_file(&client, rng) {
                        let new_parent = Self::pick_random_parent(&client, rng);
                        if file.parent != new_parent.id && file.id != new_parent.id {
                            let initial_path = get_path_by_id(&client, file.id).unwrap();
                            let move_file_result = move_file(&client, file.id, new_parent.id);
                            match move_file_result {
                                Ok(()) | Err(UiError(MoveFileError::FolderMovedIntoItself)) => {}
                                _ => panic!(
                                    "Unexpected error while moving file: {:#?}",
                                    move_file_result
                                ),
                            }
                            print!(
                                "[{:?}]\t{:?} to {:?}",
                                file.id,
                                initial_path,
                                get_path_by_id(&client, file.id).unwrap()
                            );
                        }
                    }
                }
                RenameFile => {
                    let client = Self::random_client(clients, rng);
                    if let Some(file) = Self::pick_random_file(&client, rng) {
                        let initial_path = get_path_by_id(&client, file.id).unwrap();
                        let new_name = Self::random_filename(rng) + ".md";
                        rename_file(&client, file.id, &new_name).unwrap();
                        print!(
                            "[{:?}]\t{:?} to {:?}",
                            file.id,
                            initial_path,
                            get_path_by_id(&client, file.id).unwrap()
                        );
                    }
                }
                DeleteFile => {
                    let client = Self::random_client(clients, rng);
                    if let Some(file) = Self::pick_random_file(&client, rng) {
                        print!(
                            "[{:?}]\t{:?}",
                            file.id,
                            get_path_by_id(&client, file.id).unwrap()
                        );
                        delete_file(&client, file.id).unwrap();
                    }
                }
            }
        }

        fn random_client(clients: &[Config], rng: &mut StdRng) -> Config {
            let client_index = rng.gen_range(0..CLIENTS) as usize;
            print!("client index = {:?}\t", client_index);
            clients[client_index].clone()
        }

        fn random_filename(rng: &mut StdRng) -> String {
            rng.sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect()
        }

        fn random_utf8(rng: &mut StdRng) -> String {
            rng.sample_iter(&Alphanumeric)
                .take(MAX_FILE_SIZE)
                .map(char::from)
                .collect()
        }

        fn pick_random_file(config: &Config, rng: &mut StdRng) -> Option<ClientFileMetadata> {
            let mut possible_files = list_metadatas(&config).unwrap();
            possible_files.retain(|meta| meta.parent != meta.id);
            possible_files.sort_by(Self::deterministic_sort());

            if !possible_files.is_empty() {
                let parent_index = rng.gen_range(0..possible_files.len());
                Some(possible_files[parent_index].clone())
            } else {
                None
            }
        }

        fn deterministic_sort() -> fn(&ClientFileMetadata, &ClientFileMetadata) -> Ordering {
            |lhs, rhs| {
                if lhs.parent == lhs.id {
                    Ordering::Less
                } else if rhs.id == rhs.parent {
                    Ordering::Greater
                } else {
                    lhs.name.cmp(&rhs.name)
                }
            }
        }

        fn pick_random_parent(config: &Config, rng: &mut StdRng) -> ClientFileMetadata {
            let mut possible_parents = list_metadatas(&config).unwrap();
            possible_parents.retain(|meta| meta.file_type == Folder);
            possible_parents.sort_by(Self::deterministic_sort());

            let parent_index = rng.gen_range(0..possible_parents.len());
            possible_parents[parent_index].clone()
        }

        fn pick_random_document(config: &Config, rng: &mut StdRng) -> Option<ClientFileMetadata> {
            let mut possible_documents = list_metadatas(&config).unwrap();
            possible_documents.retain(|meta| meta.file_type == Document);
            possible_documents.sort_by(Self::deterministic_sort());

            if !possible_documents.is_empty() {
                let document_index = rng.gen_range(0..possible_documents.len());
                Some(possible_documents[document_index].clone())
            } else {
                None
            }
        }
    }

    fn create_clients() -> Vec<Config> {
        let mut configs = vec![];

        for _ in 0..CLIENTS {
            configs.push(test_config());
        }

        create_account(&configs[0], &random_username(), &url()).unwrap();
        let account_string = export_account(&configs[0]).unwrap();

        for client in &configs[1..] {
            import_account(&client, &account_string).unwrap();
            sync(&client, None).unwrap();
        }
        configs
    }

    impl Distribution<Actions> for Standard {
        fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Actions {
            match rng.gen_range(0..Actions::VARIANT_COUNT) {
                0 => SyncAndCheck,
                1 => NewFolder,
                2 => NewMarkdownDocument,
                3 => UpdateDocument,
                4 => MoveDocument,
                5 => AttemptFolderMove,
                6 => RenameFile,
                7 => DeleteFile,
                _ => panic!("An enum was added to Actions, but does not have a corresponding random selection")
            }
        }
    }

    fn setup_progress_bar() -> ProgressBar {
        let pb = ProgressBar::new(ACTION_COUNT);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {msg}")
                .with_key("eta", |state| format!("{:.1}s", state.eta().as_secs_f64()))
                .progress_chars("#>-"),
        );
        pb
    }
}
