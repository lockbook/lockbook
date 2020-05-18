use crate::model::state::Config;

pub enum Error {
    General,
}

pub trait Store {
    fn update(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Error>;
    fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, Error>;
}

pub struct FsStore {
    pub config: Config,
}

impl Store for FsStore {
    fn update(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Error> {
        let path = std::path::Path::new(crate::JUNK);
        match std::fs::write(path, value) {
            Ok(_) => {
                debug!("Wrote some new shit to junk");
                Ok(())
            }
            Err(err) => {
                error!("Failed to write to junk! {:?}", err);
                Err(Error::General)
            }
        }
    }

    fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, Error> {
        let path = std::path::Path::new(crate::JUNK);
        match std::fs::read(path) {
            Ok(val) => Ok(Some(val)),
            Err(err) => {
                error!("Failed to read junk! {:?}", err);
                Err(Error::General)
            }
        }
    }
}
