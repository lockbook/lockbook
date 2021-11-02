mod integration_test;

#[cfg(test)]
mod sync_fuzzer2 {
    use crate::sync_fuzzer2::Action::NewDocument;
    use crate::sync_fuzzer2::Status::{Failed, Ready, Running, Succeeded};
    use lockbook_core::model::client_conversion::ClientFileMetadata;
    use lockbook_core::model::state::Config;
    use lockbook_core::service::test_utils::{random_username, test_config, url};
    use lockbook_core::{create_account, export_account, import_account, sync_all};
    use lockbook_core::{create_file, list_metadatas};
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use rand::distributions::Alphanumeric;
    use rand::rngs::OsRng;
    use rand::Rng;
    use std::future::Pending;
    use variant_count::VariantCount;

    struct Experiment {
        pub pending: Vec<Trial>,
        pub running: Vec<Trial>,
        pub concluded: Vec<Trial>,
    }

    #[derive(VariantCount, Debug, Clone)]
    enum Action {
        NewDocument {
            client: usize,
            parent: String,
            name: String,
        },
    }

    #[derive(Clone, Debug, PartialEq)]
    enum Status {
        Ready,
        Running,
        Succeeded,
        Failed,
    }

    #[derive(Clone, Debug)]
    struct Trial {
        pub clients: Vec<Config>,
        pub target_clients: usize,
        pub target_steps: usize,
        pub steps: Vec<Action>,
        pub completed_steps: usize,
        pub status: Status,
    }

    fn find_by_name(config: &Config, name: &str) -> ClientFileMetadata {
        let mut possible_matches = list_metadatas(config).unwrap();
        possible_matches.retain(|meta| meta.name == name);
        if possible_matches.len() > 1 {
            eprintln!("Multiple matches for a file name found: {}", name);
        } else if possible_matches.is_empty() {
            panic!("No file matched name: {}", name);
        }

        possible_matches[0].clone()
    }

    fn random_filename() -> String {
        OsRng
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect()
    }

    impl Trial {
        fn create_clients(&mut self) {
            for _ in 0..self.target_clients {
                self.clients.push(test_config());
            }

            create_account(&self.clients[0], &random_username(), &url()).unwrap();
            let account_string = export_account(&self.clients[0]).unwrap();

            for client in &self.clients[1..] {
                import_account(&client, &account_string).unwrap();
                sync_all(&client, None).unwrap();
            }
        }

        fn perform_all_known_actions(&mut self) {
            let mut additional_completed_steps = 0;
            for index in self.completed_steps..self.steps.len() {
                let step = self.steps[index].clone();
                additional_completed_steps += 1;
                match step {
                    Action::NewDocument {
                        client,
                        parent,
                        name,
                    } => {
                        let db = self.clients[client].clone();
                        let parent = find_by_name(&db, &parent).id;
                        if let Err(err) = create_file(&db, &name, parent, Document) {
                            eprintln!("Create file error: {:#?}", err);
                            self.status = Failed;
                            break;
                        }
                    }
                }
            }
            self.completed_steps += additional_completed_steps;
            if self.status == Running && self.completed_steps == self.target_steps {
                self.status = Succeeded;
            }
        }

        fn generate_mutations(&self) -> Vec<Trial> {
            let mut mutants: Vec<Trial> = vec![];

            if self.status != Running {
                return mutants;
            }

            for client_index in 0..self.clients.len() {
                let client = self.clients[client_index].clone();
                for parent in list_metadatas(&client).unwrap() {
                    if parent.file_type == Folder {
                        mutants.push(self.create_mutation(NewDocument {
                            parent: parent.name,
                            client: client_index,
                            name: random_filename(),
                        }))
                    }
                }
            }

            mutants
        }

        fn execute(&mut self) -> Vec<Trial> {
            self.status = Running;
            self.create_clients();

            let mut all_mutations = vec![];

            while self.status == Running {
                self.perform_all_known_actions();
                let mut mutations = self.generate_mutations();
                if let Some(next_action) = mutations.pop() {
                    self.steps.push(next_action.steps.last().unwrap().clone());
                    all_mutations.extend(mutations);
                }
            }

            all_mutations
        }

        fn create_mutation(&self, new_action: Action) -> Trial {
            let mut clone = self.clone();
            clone.steps.push(new_action);
            clone.status = Ready;
            clone
        }
    }

    #[test]
    /// Run with: cargo test --release exhaustive_test_sync -- --nocapture --ignored
    fn exhaustive_test_sync() {
        let actions = 3;
        let clients = 2;

        let mut trial = Trial {
            clients: vec![],
            target_clients: 2,
            target_steps: 2,
            steps: vec![],
            completed_steps: 0,
            status: Status::Ready,
        };

        let mutations = trial.execute();

        println!("{:#?}", trial);

        println!("{:#?}", mutations);
    }
}
