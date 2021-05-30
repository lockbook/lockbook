mod integration_test;

/// We should figure out a way to not just copy-paste these tests!
#[cfg(test)]
mod unit_tests_file {
    use lockbook_core::model::state::temp_config;
    use lockbook_core::storage::db_provider::FileBackend;

    #[test]
    fn read() {
        let config = &temp_config();

        let result = FileBackend::read::<_, _, Vec<u8>>(config, "files", "notes.txt").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_and_read() {
        let config = &temp_config();

        let data = "noice";

        FileBackend::write(config, "files", "notes.txt", data).unwrap();

        let result = FileBackend::read::<_, _, Vec<u8>>(config, "files", "notes.txt")
            .unwrap()
            .unwrap();

        assert_eq!(result, data.as_bytes());
    }

    #[test]
    fn write_and_dump() {
        let config = &temp_config();

        println!("{:?}", config);

        let data = "noice";

        FileBackend::write(config, "files", "a.txt", data).unwrap();
        FileBackend::write(config, "files", "b.txt", data).unwrap();
        FileBackend::write(config, "files", "c.txt", data).unwrap();

        assert_eq!(
            vec![
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec()
            ],
            FileBackend::dump::<_, Vec<u8>>(config, "files").unwrap()
        )
    }

    #[test]
    fn write_read_delete() {
        let config = &temp_config();

        let data = "noice";

        FileBackend::write(config, "files", "notes.txt", data).unwrap();

        assert_eq!(
            data.as_bytes().to_vec(),
            FileBackend::read::<_, _, Vec<u8>>(config, "files", "notes.txt")
                .unwrap()
                .unwrap()
        );

        FileBackend::delete(config, "files", "notes.txt").unwrap();

        assert_eq!(
            None,
            FileBackend::read::<_, _, Vec<u8>>(config, "files", "notes.txt").unwrap()
        );
    }
}
