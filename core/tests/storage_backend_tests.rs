mod integration_test;

#[cfg(test)]
mod unit_tests_sled {
    use lockbook_core::connect_to_db;
    use lockbook_core::model::state::temp_config;
    use lockbook_core::storage::db_provider::Backend;

    #[test]
    fn read() {
        let cfg = &temp_config();
        let db = &connect_to_db(cfg).unwrap();
        let backend = &Backend::Sled(db);

        let result = backend.read::<_, _, Vec<u8>>("files", "notes.txt").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_and_read() {
        let cfg = &temp_config();
        let db = &connect_to_db(cfg).unwrap();
        let backend = &Backend::Sled(db);

        let data = "noice";

        backend.write("files", "notes.txt", data).unwrap();

        let result = backend
            .read::<_, _, Vec<u8>>("files", "notes.txt")
            .unwrap()
            .unwrap();

        assert_eq!(result, data.as_bytes());
    }

    #[test]
    fn write_and_dump() {
        let cfg = &temp_config();
        let db = &connect_to_db(cfg).unwrap();
        let backend = &Backend::Sled(db);

        let data = "noice";

        backend.write("files", "a.txt", data).unwrap();
        backend.write("files", "b.txt", data).unwrap();
        backend.write("files", "c.txt", data).unwrap();

        assert_eq!(
            vec![
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec()
            ],
            backend.dump::<_, Vec<u8>>("files").unwrap()
        )
    }

    #[test]
    fn write_read_delete() {
        let cfg = &temp_config();
        let db = &connect_to_db(cfg).unwrap();
        let backend = &Backend::Sled(db);

        let data = "noice";

        backend.write("files", "notes.txt", data).unwrap();

        assert_eq!(
            data.as_bytes().to_vec(),
            backend
                .read::<_, _, Vec<u8>>("files", "notes.txt")
                .unwrap()
                .unwrap()
        );

        backend.delete("files", "notes.txt").unwrap();

        assert_eq!(
            None,
            backend.read::<_, _, Vec<u8>>("files", "notes.txt").unwrap()
        );
    }
}

/// We should figure out a way to not just copy-paste these tests!
#[cfg(test)]
mod unit_tests_file {
    use lockbook_core::model::state::temp_config;
    use lockbook_core::storage::db_provider::Backend;

    #[test]
    fn read() {
        let cfg = &temp_config();
        let backend = &Backend::File(cfg);

        let result = backend.read::<_, _, Vec<u8>>("files", "notes.txt").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_and_read() {
        let cfg = &temp_config();
        let backend = &Backend::File(cfg);

        let data = "noice";

        backend.write("files", "notes.txt", data).unwrap();

        let result = backend
            .read::<_, _, Vec<u8>>("files", "notes.txt")
            .unwrap()
            .unwrap();

        assert_eq!(result, data.as_bytes());
    }

    #[test]
    fn write_and_dump() {
        let cfg = &temp_config();
        let backend = &Backend::File(cfg);

        println!("{:?}", cfg);

        let data = "noice";

        backend.write("files", "a.txt", data).unwrap();
        backend.write("files", "b.txt", data).unwrap();
        backend.write("files", "c.txt", data).unwrap();

        assert_eq!(
            vec![
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec()
            ],
            backend.dump::<_, Vec<u8>>("files").unwrap()
        )
    }

    #[test]
    fn write_read_delete() {
        let cfg = &temp_config();
        let backend = &Backend::File(cfg);

        let data = "noice";

        backend.write("files", "notes.txt", data).unwrap();

        assert_eq!(
            data.as_bytes().to_vec(),
            backend
                .read::<_, _, Vec<u8>>("files", "notes.txt")
                .unwrap()
                .unwrap()
        );

        backend.delete("files", "notes.txt").unwrap();

        assert_eq!(
            None,
            backend.read::<_, _, Vec<u8>>("files", "notes.txt").unwrap()
        );
    }
}
