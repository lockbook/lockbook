use crate::utils::{connect_to_db, get_account, print_last_successful_sync};
use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::DefaultSyncService;

pub fn status() {
    let db = connect_to_db();
    get_account(&db);

    DefaultSyncService::calculate_work(&db)
        .expect("Failed to calculate work required to sync")
        .work_units
        .into_iter()
        .for_each(|work| match work {
            WorkUnit::PushNewFile(client) => {
                println!("{} has local changes that need to be pushed", client.name)
            }
            WorkUnit::UpdateLocalMetadata(server) => println!(
                "{} has been moved or renamed on the server",
                server.name
            ),
            WorkUnit::PullFileContent(server) => {
                println!("{} has new content available", server.name)
            }
            WorkUnit::DeleteLocally(client) => {
                println!("{} needs to be deleted locally", client.name)
            }
            WorkUnit::PushMetadata(client) => println!("{} has been moved locally", client.name),
            WorkUnit::PushFileContent(client) => {
                println!("{} has local changes that need to be pushed", client.name)
            }
            WorkUnit::PushDelete(client) => println!("{} has been deleted locally", client.name),
            WorkUnit::PullMergePush(server) => {
                println!("{} has changes locally and on the server", server.name)
            }
            WorkUnit::MergeMetadataAndPushMetadata(server) => println!(
                "{} has been moved or renamed locally and on the server",
                server.name
            ),
        });

    print_last_successful_sync(&db);
}
