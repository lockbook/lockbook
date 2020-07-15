use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{get_account, DefaultSyncService, GetAccountError};

use crate::utils::{
    connect_to_db, exit_with, exit_with_no_account, get_config, print_last_successful_sync,
};
use crate::UNEXPECTED_ERROR;

pub fn status() {
    match get_account(&get_config()) {
        Ok(_) => {}
        Err(err) => match err {
            GetAccountError::NoAccount => exit_with_no_account(),
            GetAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }

    DefaultSyncService::calculate_work(&connect_to_db())
        .expect("Failed to calculate work required to sync")
        .work_units
        .into_iter()
        .for_each(|work| match work {
            WorkUnit::LocalChange { metadata } => println!("{} needs to be pushed", metadata.name),
            WorkUnit::ServerChange { metadata } => println!("{} needs to be pulled", metadata.name),
        });

    print_last_successful_sync(&connect_to_db());
}
