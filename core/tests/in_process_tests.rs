#[cfg(feature = "no-network")]
#[cfg(test)]
mod ip_tests {
    use lockbook_core::model::errors::Error::UiError;
    use lockbook_core::model::errors::*;
    use lockbook_core::service::api_service::no_network::{CoreIP, InProcess};
    use std::default::Default;
    use test_utils::test_config;
    use test_utils::*;

    #[test]
    fn with_init_username_taken() {
        let server = InProcess::init(test_config(), Default::default());
        let core1 = CoreIP::init_in_process(&test_config(), server.clone());
        let core2 = CoreIP::init_in_process(&test_config(), server);
        let name = random_name();
        core1.create_account(&name, "not used").unwrap();
        assert_matches!(
            core2.create_account(&name, "not used"),
            Err(UiError(CreateAccountError::UsernameTaken))
        );
    }

    #[test]
    fn create_sync_compare() {
        let server = InProcess::init(test_config(), Default::default());
        let core1 = CoreIP::init_in_process(&test_config(), server.clone());
        let core2 = CoreIP::init_in_process(&test_config(), server);
        core1.create_account(&random_name(), "unused af").unwrap();
        core2
            .import_account(&core1.export_account().unwrap())
            .unwrap();
        core2.sync(None).unwrap();

        let doc = core2.create_at_path("test.md").unwrap();
        core2.write_document(doc.id, b"test").unwrap();

        core1.sync(None).unwrap();
        core2.sync(None).unwrap();
        core1.sync(None).unwrap();
        core2.sync(None).unwrap();

        assert!(dbs_equal(&core1, &core2));
    }

    #[test]
    fn sync_and_check() {
        loop {
            let server = InProcess::init(test_config(), Default::default());
            let core1 = CoreIP::init_in_process(&test_config(), server.clone());
            let core2 = CoreIP::init_in_process(&test_config(), server.clone());

            core1.create_account(&random_name(), "unused af").unwrap();
            core2
                .import_account(&core1.export_account().unwrap())
                .unwrap();
            core2.sync(None).unwrap();

            let doc = core1.create_at_path("PVJpYfU.md").unwrap();
            core1.write_document(doc.id, b"qaFUI3VI8MUYfxOnykdVmA0sthZQPtMPwUVbrBMYqPfGiWqNdaTqerEB6Tz4o93Yvml5uWBE58WyqER5KUhllqBgAowD3QzKuxuWMnmpIvWC973XAyr5GWNVzyBq7PC85yUxCkcjylP9OqeRyUzadqkg0bHyXGpYRSWZkQFk5pGRQFOo5D986KDmPwf4VXHayBqvKYuPkmwZCO4YJBpmQds4cu5Um9hPU79YO8YZCsPiEprkto1SBX06oXxhPa7NkNVREIngUkSgPvvBMzhom8ISbBhbd8kIcqn1UpNJyRlAsXSQtAjuNXYk4caxXZDvYddMxVxtWo3qIzkhzAY5w6iLD4RBBFA5bUa6BrXGyqKMf1wFF2XKgsfpEbIORugv6pDau9GqiIsGUlBjHZEZPBYdwRmpzjtfwcXPcl2W4SGoZ3bXle970SJYNlHsKlNEmihFdkHVlFt6Gp24zSofqa0SxMHATARJKdphsniYx4kpsjiGowqDsXeN8FpmHv408qGZAvS73KRWpA3jSmIaeSeJXieE3FXsnsbfUEMAU2ZQyKz3S4XKY90z3tKxuzeOh6FeY7FuXZoiuhIs5XPZHSjtbAcesRVvogMhrF0kXp3CXNEEk2QccA6PAJkygj2mSNx7bN9pYCi5FA1R1TPKYZXAkr1qCYZ670v6tG9yt4G8GhimfQtulQqrm8rsneiLLFS67CJHDptQxpNMEgQYIQGcHIbmPE8lvuDywPXWG7qaVFeilM5PAVOLnhuO0jAQB5RSHMFs7kfqfql0uFxrq2m1HjcdgrXyFlnBJzfsWzBH7hLu7fdbmKOXjjyaaqwvGBdfPzzbv6q8NDFueg2ysSKpRawE5fZjMixlfO3Zbab9Uhe8yV1fIWFjzwJP5lDkdtd8e84UXwwsHnQb0rZc3X3x7NG4HsJMCoeukAjfg7yvXFSJYR1fgCMuiYzyNRV9NyN1p9ielKxZkLgGqaw3DdYWngZ8cvFtOELK17AgdnjHATo8dM2aetkdwm9Y65FH").unwrap();
            core1.create_at_path("VZHLvcr.md").unwrap();

            core1.sync(None).unwrap();
            core2.sync(None).unwrap();
        }
    }
}
