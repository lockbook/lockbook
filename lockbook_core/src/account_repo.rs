extern crate base64;

use std::marker::PhantomData;

use base64::encode;
use rusqlite::{Connection, params};

use crate::account::Account;
use crate::db_provider;
use crate::db_provider::DbProvider;
use crate::error_enum;
use crate::state::Config;

error_enum! {
    enum Error {
        ConnectionError(db_provider::Error)
    }
}

pub trait AccountRepo {
    fn insert_account(config: Config, account: &Account) -> Result<&Account, Error>;
}

pub struct AccountRepoImpl<DB: DbProvider> {
    db: PhantomData<DB>,
}

impl<DB: DbProvider> AccountRepo for AccountRepoImpl<DB> {
    fn insert_account(config: Config, account: &Account) -> Result<&Account, Error> {
        let db = DB::connect_to_db(config)?;
        
        db.execute(
            "insert into user_info
            (username, public_n, public_e, private_d, private_p, private_q, private_dmp1, private_dmq1, private_iqmp)
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
            encode(&account.username),
            encode(&account.public_key.n),
            encode(&account.public_key.e),
            encode(&account.private_key.d),
            encode(&account.private_key.p),
            encode(&account.private_key.dmp1),
            encode(&account.private_key.dmq1),
            encode(&account.private_key.iqmp),
            ]).unwrap();
        
        Ok(account)
    }
}
