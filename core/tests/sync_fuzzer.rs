mod integration_test;

#[cfg(test)]
mod sync_fuzzer {
    use lockbook_core::model::state::Config;
    use lockbook_core::service::test_utils::{test_config, random_username, url};
    use rand::distributions::{Distribution, Standard};
    use rand::{Rng, SeedableRng};
    use variant_count::VariantCount;
    use crate::sync_fuzzer::Actions::{NewFolder, NewMarkdownFile, SyncAndCheck};
    use rand::rngs::StdRng;
    use lockbook_core::service::account_service::{create_account, import_account, export_account};
    use lockbook_core::service::sync_service::sync;

    static CLIENTS: u8 = 2;
    static ACTION_COUNT: u8 = 10;

    /// If you add a variant here, make sure you add the corresponding entry for random selection
    /// See `impl Distribution<Actions> for Standard`
    #[derive(VariantCount)]
    enum Actions {
        NewFolder(u8),
        NewMarkdownFile(u8),
        SyncAndCheck,
    }

    #[test]
    fn stress_test_sync() {
        let mut rng = StdRng::seed_from_u64(0);
        let clients = create_clients();

        for _ in 0..ACTION_COUNT {
            let action: Actions = rng.gen();
            action.execute(&clients);
        }
    }

    impl Actions {
        fn execute(&self, clients: &[Config])  {
            match &self {
                NewFolder(id) => {
                    println!("New Folder: {}", id)
                }
                NewMarkdownFile(id) => {
                    println!("Markdown: {}", id)
                }
                SyncAndCheck => {
                    println!("Sync")
                }
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

        for client in configs[1..] {
            import_account(&client, &account_string).unwrap();
            sync(&client, None).unwrap();
        }
        configs
    }

    impl Distribution<Actions> for Standard {
        fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Actions {
            let client_id = rng.gen_range(0, CLIENTS);

            match rng.gen_range(0, Actions::VARIANT_COUNT) {
                0 => NewFolder(client_id),
                1 => NewMarkdownFile(client_id),
                2 => SyncAndCheck,
                _ => panic!("An enum was added to Actions, but does not have a corresponding random selection")
            }
        }
    }
}