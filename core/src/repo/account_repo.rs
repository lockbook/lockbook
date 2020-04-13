use std::ops::Try;
use std::option::NoneError;

use rusqlite::{params, Connection};

use crate::crypto::{KeyPair, PrivateKey, PublicKey};
use crate::error_enum;
use crate::model::account::Account;

error_enum! {
    enum Error {
        DbError(rusqlite::Error),
        AccountRowMissing(NoneError),
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
            "INSERT INTO user_info (id, username, public_n, public_e, private_d, private_p, private_q, private_dmp1, private_dmq1, private_iqmp) VALUES (0, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &account.username,
                &account.keys.public_key.n,
                &account.keys.public_key.e,
                &account.keys.private_key.d,
                &account.keys.private_key.p,
                &account.keys.private_key.q,
                &account.keys.private_key.dmp1,
                &account.keys.private_key.dmq1,
                &account.keys.private_key.iqmp,
            ],
        )?;
        Ok(())
    }

    fn get_account(db: &Connection) -> Result<Account, Error> {
        let mut stmt = db.prepare(
            "SELECT username, public_n, public_e, private_d, private_p, private_q, private_dmp1, private_dmq1, private_iqmp FROM user_info WHERE id = 0",
        )?;

        let mut user_iter = stmt.query_map(params![], |row| {
            Ok(Account {
                username: row.get(0)?,
                keys: KeyPair {
                    public_key: PublicKey {
                        n: row.get(1)?,
                        e: row.get(2)?,
                    },
                    private_key: PrivateKey {
                        d: row.get(3)?,
                        p: row.get(4)?,
                        q: row.get(5)?,
                        dmp1: row.get(6)?,
                        dmq1: row.get(7)?,
                        iqmp: row.get(8)?,
                    },
                },
            })
        })?;

        let maybe_row = user_iter.next().into_result()?;

        // TODO attempt to check key for validity?
        Ok(maybe_row?)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::crypto::{KeyPair, PrivateKey, PublicKey};
    use crate::model::account::Account;
    use crate::model::state::Config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::db_provider::{DbProvider, RamBackedDB};
    use crate::repo::schema::SchemaCreatorImpl;

    type DefaultSchema = SchemaCreatorImpl;
    type DefaultDbProvider = RamBackedDB<DefaultSchema>;
    type DefaultAcountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            keys: KeyPair {
                public_key: PublicKey {
                    n: "vec![1]".to_string(),
                    e: "vec![2]".to_string(),
                },
                private_key: PrivateKey {
                    d: "vec![3]".to_string(),
                    p: "vec![4]".to_string(),
                    q: "vec![5]".to_string(),
                    dmp1: "vec![6]".to_string(),
                    dmq1: "vec![7]".to_string(),
                    iqmp: "vec![8]".to_string(),
                },
            },
        };

        let config = &Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(config).unwrap();
        DefaultAcountRepo::insert_account(&db, &test_account).unwrap();

        let db_account = DefaultAcountRepo::get_account(&db).unwrap();
        assert_eq!(test_account, db_account);
    }
}
