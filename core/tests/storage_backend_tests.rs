mod integration_test;

#[cfg(test)]
mod unit_tests_sled {
    use lockbook_core::model::state::temp_config;
    use lockbook_core::storage::db_provider::{Backend, SledBackend};

    #[test]
    fn read() {
        let cfg = &temp_config();
        let db = SledBackend::connect_to_db(cfg).unwrap();

        let result = SledBackend::read::<_, _, Vec<u8>>(&db, "files", "notes.txt").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_and_read() {
        let cfg = &temp_config();
        let db = SledBackend::connect_to_db(cfg).unwrap();

        let data = "noice";

        SledBackend::write(&db, "files", "notes.txt", data).unwrap();

        let result = SledBackend::read::<_, _, Vec<u8>>(&db, "files", "notes.txt")
            .unwrap()
            .unwrap();

        assert_eq!(result, data.as_bytes());
    }

    #[test]
    fn write_and_dump() {
        let cfg = &temp_config();
        let db = SledBackend::connect_to_db(cfg).unwrap();

        let data = "noice";

        SledBackend::write(&db, "files", "a.txt", data).unwrap();
        SledBackend::write(&db, "files", "b.txt", data).unwrap();
        SledBackend::write(&db, "files", "c.txt", data).unwrap();

        assert_eq!(
            vec![
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec(),
                data.as_bytes().to_vec()
            ],
            SledBackend::dump::<_, Vec<u8>>(&db, "files").unwrap()
        )
    }

    #[test]
    fn write_read_delete() {
        let cfg = &temp_config();
        let db = SledBackend::connect_to_db(cfg).unwrap();

        let data = "noice";

        SledBackend::write(&db, "files", "notes.txt", data).unwrap();

        assert_eq!(
            data.as_bytes().to_vec(),
            SledBackend::read::<_, _, Vec<u8>>(&db, "files", "notes.txt")
                .unwrap()
                .unwrap()
        );

        SledBackend::delete(&db, "files", "notes.txt").unwrap();

        assert_eq!(
            None,
            SledBackend::read::<_, _, Vec<u8>>(&db, "files", "notes.txt").unwrap()
        );
    }
}

/// We should figure out a way to not just copy-paste these tests!
#[cfg(test)]
mod unit_tests_file {
    use lockbook_core::model::state::temp_config;
    use lockbook_core::storage::db_provider::{Backend, FileBackend};

    type MyBackend = FileBackend;

    #[test]
    fn read() {
        let cfg = &temp_config();
        let backend = &MyBackend::connect_to_db(cfg).unwrap();

        let result = MyBackend::read::<_, _, Vec<u8>>(backend, "files", "notes.txt").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_and_read() {
        let cfg = &temp_config();
        let backend = &MyBackend::connect_to_db(cfg).unwrap();

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
        let backend = &MyBackend::connect_to_db(cfg).unwrap();

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
            MyBackend::dump::<_, Vec<u8>>(backend, "files").unwrap()
        )
    }

    #[test]
    fn write_read_delete() {
        let cfg = &temp_config();
        let backend = &MyBackend::connect_to_db(cfg).unwrap();

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
