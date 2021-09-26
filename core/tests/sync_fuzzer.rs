mod integration_test;

#[cfg(test)]
mod sync_fuzzer {
    use crate::sync_fuzzer::Actions::{NewFolder, NewMarkdownFile, SyncAndCheck};
    use indicatif::{ProgressBar, ProgressStyle};
    use lockbook_core::model::client_conversion::ClientFileMetadata;
    use lockbook_core::model::state::Config;
    use lockbook_core::service::account_service::{create_account, export_account, import_account};
    use lockbook_core::service::sync_service::sync;
    use lockbook_core::service::test_utils::{assert_dbs_eq, random_username, test_config, url};
    use lockbook_core::{calculate_work, create_file, list_metadatas};
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use rand::distributions::{Alphanumeric, Distribution, Standard};
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use variant_count::VariantCount;

    static SEED: u64 = 0;
    static CLIENTS: u8 = 5;
    static ACTION_COUNT: u64 = 1000;
    static SHOW_PROGRESS: bool = true;
    static MAX_FILE_SIZE: u128 = 1024;

    /// If you add a variant here, make sure you add the corresponding entry for random selection
    /// See `impl Distribution<Actions> for Standard`
    #[derive(VariantCount, Debug)]
    enum Actions {
        NewFolder(u8),
        NewMarkdownFile { client_id: u8, file_size: u128 },
        SyncAndCheck,
    }

    #[test]
    fn stress_test_sync() {
        println!("seed: {}", SEED);
        println!("clients: {}", CLIENTS);

        let mut rng = StdRng::seed_from_u64(SEED);
        let clients = create_clients();

        let pb = setup_progress_bar();
        for _ in 0..ACTION_COUNT {
            let action = rng.gen::<Actions>();
            if SHOW_PROGRESS {
                pb.set_message(format!("{:?}", action));
                pb.inc(1)
            };
            action.execute(&clients, &mut rng);
        }
    }

    impl Actions {
        fn execute(&self, clients: &[Config], rng: &mut StdRng) {
            match &self {
                NewFolder(id) => {
                    let client = &clients[(*id as usize)];
                    let parent = Self::pick_random_parent(&client, rng);
                    let name = Self::random_filename(rng);
                    create_file(&client, &name, parent.id, Folder).unwrap();
                }
                NewMarkdownFile {
                    client_id,
                    file_size,
                } => {
                    let client = &clients[(*client_id as usize)];
                    let parent = Self::pick_random_parent(&client, rng);
                    let name = Self::random_filename(rng);
                    let file = create_file(client, &name, parent.id, Document).unwrap();
                }
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
                        assert!(calculate_work(row).unwrap().local_files.is_empty());
                        assert!(calculate_work(row).unwrap().server_files.is_empty());
                        assert_eq!(calculate_work(row).unwrap().server_unknown_name_count, 0);
                    }
                }
            }
        }

        fn random_filename(rng: &mut StdRng) -> String {
            rng.sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect()
        }

        fn random_utf8(rng: &mut StdRng, size) -> String {
            rand::thread_rng()
                .gen_iter::<char>()
                .take(len)
                .collect();
        }

        fn pick_random_parent(config: &Config, rng: &mut StdRng) -> ClientFileMetadata {
            let mut possible_parents = list_metadatas(&config).unwrap();
            possible_parents.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
            possible_parents.retain(|meta| meta.file_type == Folder);

            let parent_index = rng.gen_range(0, possible_parents.len());
            possible_parents[parent_index].clone()
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
            let client_id = rng.gen_range(0, CLIENTS);
            let file_size = rng.gen_range(0, MAX_FILE_SIZE);

            match rng.gen_range(0, Actions::VARIANT_COUNT) {
                0 => NewFolder(client_id),
                1 => NewMarkdownFile { client_id, file_size },
                2 => SyncAndCheck,
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
