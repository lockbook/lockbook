pub trait DbProvider {
    fn connect_to_db();
}

pub struct DbProviderImpl;

impl DBProvider for DbProviderImpl {

}