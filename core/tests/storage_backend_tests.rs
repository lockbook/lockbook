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

        let result = MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_and_read() {
        let cfg = &temp_config();
        let db = &connect_to_db(cfg).unwrap();
        let backend = &Backend::Sled(db);

        let data = "noice";

        MyBackend::write(backend, "files", "notes.txt", data).unwrap();

        let result = MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt")
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

        MyBackend::write(backend, "files", "a.txt", data).unwrap();
        MyBackend::write(backend, "files", "b.txt", data).unwrap();
        MyBackend::write(backend, "files", "c.txt", data).unwrap();

        assert_eq!(
            vec![
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec()
            ],
            MyBackend::<_, Vec<u8>>::dump(backend, "files").unwrap()
        )
    }

    #[test]
    fn write_read_delete() {
        let cfg = &temp_config();
        let db = &connect_to_db(cfg).unwrap();
        let backend = &Backend::Sled(db);

        let data = "noice";

        MyBackend::write(backend, "files", "notes.txt", data).unwrap();

        assert_eq!(
            data.as_bytes().to_vec(),
            MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt")
                .unwrap()
                .unwrap()
        );

        MyBackend::delete(backend, "files", "notes.txt").unwrap();

        assert_eq!(
            None,
            MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt").unwrap()
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

        let result = MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_and_read() {
        let cfg = &temp_config();
        let backend = &Backend::File(cfg);

        let data = "noice";

        MyBackend::write(backend, "files", "notes.txt", data).unwrap();

        let result = MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt")
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

        MyBackend::write(backend, "files", "a.txt", data).unwrap();
        MyBackend::write(backend, "files", "b.txt", data).unwrap();
        MyBackend::write(backend, "files", "c.txt", data).unwrap();

        assert_eq!(
            vec![
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec()
            ],
            MyBackend::<_, Vec<u8>>::dump(backend, "files").unwrap()
        )
    }

    #[test]
    fn write_read_delete() {
        let cfg = &temp_config();
        let backend = &Backend::File(cfg);

        let data = "noice";

        MyBackend::write(backend, "files", "notes.txt", data).unwrap();

        assert_eq!(
            data.as_bytes().to_vec(),
            MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt")
                .unwrap()
                .unwrap()
        );

        MyBackend::delete(backend, "files", "notes.txt").unwrap();

        assert_eq!(
            None,
            MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt").unwrap()
        );
    }
}
