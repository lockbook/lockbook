extern crate base64;

use std::marker::PhantomData;
use std::ops::Try;
use std::option::NoneError;

use base64::{decode, DecodeError, encode};
use rusqlite::{Connection, params, Row};

use crate::account::{Account, PrivateKey, PublicKey};
use crate::error_enum;
use crate::state::Config;

error_enum! {
    enum Error {
        DbError(rusqlite::Error),
        DecodingError(base64::DecodeError),
        RowMissing(NoneError),
    }
}

pub trait AccountRepo {
    fn insert_account(db: &Connection, account: &Account) -> Result<(), Error>;
    fn get_account(db: &Connection) -> Result<Account, Error>;
}

pub struct AccountRepoImpl;


impl AccountRepo for AccountRepoImpl {
    fn insert_account(db: &Connection, account: &Account) -> Result<(), Error> {
        db.execute(
            "insert into user_info
            (id, username, public_n, public_e, private_d, private_p, private_q, private_dmp1, private_dmq1, private_iqmp)
            values (0, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
            &account.username,
            encode(&account.public_key.n),
            encode(&account.public_key.e),
            encode(&account.private_key.d),
            encode(&account.private_key.p),
            encode(&account.private_key.q),
            encode(&account.private_key.dmp1),
            encode(&account.private_key.dmq1),
            encode(&account.private_key.iqmp),
            ]).unwrap();

        Ok(())
    }

    fn get_account(db: &Connection) -> Result<Account, Error> {
        let mut stmt =
            db.prepare(
                "select
                        username,
                        public_n,
                        public_e,
                        private_d,
                        private_p,
                        private_q,
                        private_dmp1,
                        private_dmq1,
                        private_iqmp
                    from user_info where id = 0"
            )?;

        let mut user_iter = stmt.query_map(params![], |row| {
            Ok(
                AccountRow { // TODO this step should not be needed, why can't we clone the row and return it here?
                    username: row.get(0)?,
                    n: row.get(1)?,
                    e: row.get(2)?,
                    d: row.get(3)?,
                    p: row.get(4)?,
                    q: row.get(5)?,
                    dmp1: row.get(6)?,
                    dmq1: row.get(7)?,
                    iqmp: row.get(8)?,
                }
            )
        })?;

        let maybe_row = user_iter.next().into_result()?;
        let row = maybe_row?;

        Ok(Account {
            username: row.username,
            public_key: PublicKey {
                n: decode(row.n.as_str())?,
                e: decode(row.e.as_str())?,
            },
            private_key: PrivateKey {
                d: decode(row.d.as_str())?,
                p: decode(row.p.as_str())?,
                q: decode(row.q.as_str())?,
                dmp1: decode(row.dmp1.as_str())?,
                dmq1: decode(row.dmq1.as_str())?,
                iqmp: decode(row.iqmp.as_str())?,
            },
        })
    }
}


#[cfg(test)]
mod tests {
    use rusqlite::params;

    use crate::account::{Account, PrivateKey, PublicKey};
    use crate::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::db_provider::{DbProvider, RamBackedDB};
    use crate::schema::SchemaCreatorImpl;
    use crate::state::Config;

    type DefaultSchema = SchemaCreatorImpl;
    type DefaultDbProvider = RamBackedDB<DefaultSchema>;
    type DefaultAcountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            public_key: PublicKey { n: vec![1], e: vec![2] },
            private_key: PrivateKey {
                d: vec![3],
                p: vec![4],
                q: vec![5],
                dmp1: vec![6],
                dmq1: vec![7],
                iqmp: vec![8],
            },
        };

        let config = Config { writeable_path: "ignored".to_string() };
        let db = DefaultDbProvider::connect_to_db(config).unwrap();
        DefaultAcountRepo::insert_account(&db, &test_account).unwrap();

        let db_account = DefaultAcountRepo::get_account(&db).unwrap();
        assert_eq!(test_account, db_account);
    }
}

struct AccountRow {
    username: String,
    n: String,
    e: String,
    d: String,
    p: String,
    q: String,
    dmp1: String,
    dmq1: String,
    iqmp: String,
}
