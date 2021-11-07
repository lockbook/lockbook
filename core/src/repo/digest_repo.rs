use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;
use uuid::Uuid;

const NAMESPACE_LOCAL: &str = "changed_local_document_digests";
const NAMESPACE_BASE: &str = "all_base_document_digests";

fn namespace(source: RepoSource) -> &'static str {
    match source {
        RepoSource::Local => NAMESPACE_LOCAL,
        RepoSource::Base => NAMESPACE_BASE,
    }
}

pub fn insert(
    config: &Config,
    source: RepoSource,
    id: Uuid,
    digest: &[u8],
) -> Result<(), CoreError> {
    local_storage::write(config, namespace(source), id.to_string().as_str(), digest)
}

pub fn get(config: &Config, source: RepoSource, id: Uuid) -> Result<Vec<u8>, CoreError> {
    maybe_get(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<Vec<u8>>, CoreError> {
    local_storage::read(config, namespace(source), id.to_string().as_str())
}

pub fn get_all(config: &Config, source: RepoSource) -> Result<Vec<Vec<u8>>, CoreError> {
    Ok(
        local_storage::dump::<_, Vec<u8>>(config, namespace(source))?
            .into_iter()
            .collect(),
    )
}

pub fn delete(config: &Config, source: RepoSource, id: Uuid) -> Result<(), CoreError> {
    local_storage::delete(config, namespace(source), id.to_string().as_str())
}

pub fn delete_all(config: &Config, source: RepoSource) -> Result<(), CoreError> {
    local_storage::delete_all(config, namespace(source))
}

#[cfg(test)]
mod unit_tests {
    use crate::model::repo::RepoSource;
    use crate::model::state::temp_config;
    use crate::repo::digest_repo;
    use uuid::Uuid;

    #[test]
    fn get() {
        let config = &temp_config();

        let id = Uuid::new_v4();
        let result = digest_repo::get(config, RepoSource::Local, id);

        assert!(result.is_err());
    }

    #[test]
    fn maybe_get() {
        let config = &temp_config();

        let id = Uuid::new_v4();
        let result = digest_repo::maybe_get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_get() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        let result = digest_repo::get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, digest);
    }

    #[test]
    fn insert_get_different_source() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        let result = digest_repo::maybe_get(config, RepoSource::Base, id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_get_overwrite_different_source() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        let (id_2, digest_2) = (Uuid::new_v4(), "digest_2".as_bytes());
        digest_repo::insert(config, RepoSource::Base, id_2, digest_2).unwrap();
        let result = digest_repo::get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, digest);
    }

    #[test]
    fn insert_get_all() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        let (id_2, digest_2) = (Uuid::new_v4(), "digest_2".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id_2, digest_2).unwrap();
        let (id_3, digest_3) = (Uuid::new_v4(), "digest_3".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id_3, digest_3).unwrap();
        let (id_4, digest_4) = (Uuid::new_v4(), "digest_4".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id_4, digest_4).unwrap();
        let (id_5, digest_5) = (Uuid::new_v4(), "digest_5".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id_5, digest_5).unwrap();
        let result = digest_repo::get_all(config, RepoSource::Local).unwrap();

        let mut expectation = vec![
            (id, digest),
            (id_2, digest_2),
            (id_3, digest_3),
            (id_4, digest_4),
            (id_5, digest_5),
        ];
        expectation.sort_by(|(a, _), (b, _)| a.cmp(&b));
        let expectation = expectation
            .into_iter()
            .map(|(_, d)| d.to_vec())
            .collect::<Vec<Vec<u8>>>();
        assert_eq!(result, expectation);
    }

    #[test]
    fn insert_get_all_different_source() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        let result = digest_repo::get_all(config, RepoSource::Base).unwrap();

        assert_eq!(result, Vec::<Vec<u8>>::new());
    }

    #[test]
    fn insert_delete() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        digest_repo::delete(config, RepoSource::Local, id).unwrap();
        let result = digest_repo::maybe_get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_delete_all() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        digest_repo::delete_all(config, RepoSource::Local).unwrap();
        let result = digest_repo::get_all(config, RepoSource::Local).unwrap();

        assert_eq!(result, Vec::<Vec<u8>>::new());
    }

    #[test]
    fn insert_delete_all_different_source() {
        let config = &temp_config();

        let (id, digest) = (Uuid::new_v4(), "digest".as_bytes());
        digest_repo::insert(config, RepoSource::Local, id, digest).unwrap();
        digest_repo::delete_all(config, RepoSource::Base).unwrap();
        let result = digest_repo::get_all(config, RepoSource::Local).unwrap();

        assert_eq!(result, vec![digest]);
    }
}
