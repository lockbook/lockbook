use std::collections::HashMap;
use std::option::NoneError;

use rsa::RSAPublicKey;
use serde::export::PhantomData;

use crate::error_enum;
use crate::model::account::Account;
use crate::service::crypto_service::{
    AesDecryptionFailed, AesEncryptionFailed, AesKey, DecryptedValue, DecryptionFailed,
    EncryptedValue, EncryptedValueWithNonce, PubKeyCryptoService, SignedValue,
    SymmetricCryptoService,
};

#[derive(Clone)]
pub struct AccessInfo {
    pub username: String,
    pub public_key: RSAPublicKey,
    pub access_key: EncryptedValue,
}

pub struct EncryptedFile {
    pub access_keys: HashMap<String, AccessInfo>,
    pub content: EncryptedValueWithNonce,
    pub last_edited: SignedValue,
}

error_enum! {
    enum FileCreationError {
        FailedToEncryptAccessKey(rsa::errors::Error),
        FailedToEncryptEmptyFile(AesEncryptionFailed)
    }
}

error_enum! {
    enum FileWriteError {
        NoAccessFoundForUser(NoneError),
        UnableToDecryptAccessKey(DecryptionFailed),
        UnableToEncryptContent(AesEncryptionFailed),
        SignatureCreationError(rsa::errors::Error)
    }
}

error_enum! {
    enum UnableToReadFile {
        NoAccessFoundForUser(NoneError),
        UnableToDecryptAccessKey(DecryptionFailed),
        UnableToEncryptContent(AesDecryptionFailed),

    }
}

trait FileEncryptionService {
    fn new_file(author: &Account) -> Result<EncryptedFile, FileCreationError>;
    fn write_to_file(
        author: &Account,
        file_before: &EncryptedFile,
        content: &DecryptedValue,
    ) -> Result<EncryptedFile, FileWriteError>;
    fn read_file(key: &Account, file: &EncryptedFile) -> Result<DecryptedValue, UnableToReadFile>;
}

pub struct FileEncryptionServiceImpl<PK: PubKeyCryptoService, AES: SymmetricCryptoService> {
    pk: PhantomData<PK>,
    aes: PhantomData<AES>,
}

impl<PK: PubKeyCryptoService, AES: SymmetricCryptoService> FileEncryptionService
    for FileEncryptionServiceImpl<PK, AES>
{
    fn new_file(author: &Account) -> Result<EncryptedFile, FileCreationError> {
        let file_encryption_key = AES::generate_key();
        let author_pk = author.keys.to_public_key();

        let encrypted_for_author =
            PK::encrypt(&author_pk, &file_encryption_key.to_decrypted_value())?;

        let author_access = AccessInfo {
            username: author.username.clone(),
            public_key: author_pk,
            access_key: encrypted_for_author,
        };

        let mut access_keys = HashMap::new();
        access_keys.insert(author.username.clone(), author_access);

        let content = AES::encrypt(
            &file_encryption_key,
            &DecryptedValue {
                secret: "".to_string(),
            },
        )?;

        // TODO re-use of error
        let last_edited = PK::sign(&author.keys, author.username.clone())?;

        Ok(EncryptedFile {
            access_keys,
            content,
            last_edited,
        })
    }

    fn write_to_file(
        author: &Account,
        file_before: &EncryptedFile,
        content: &DecryptedValue,
    ) -> Result<EncryptedFile, FileWriteError> {
        let encrypted_key = &file_before.access_keys.get(&author.username)?.access_key;
        let file_encryption_key = AesKey {
            key: PK::decrypt(&author.keys, encrypted_key)?.secret,
        };
        let new_content = AES::encrypt(&file_encryption_key, &content)?;
        let signature = PK::sign(&author.keys, author.username.clone())?;

        Ok(EncryptedFile {
            access_keys: file_before.access_keys.clone(),
            content: new_content,
            last_edited: signature,
        })
    }

    fn read_file(
        account: &Account,
        file: &EncryptedFile,
    ) -> Result<DecryptedValue, UnableToReadFile> {
        let encrypted_key = &file.access_keys.get(&account.username)?.access_key;
        let file_encryption_key = AesKey {
            key: PK::decrypt(&account.keys, encrypted_key)?.secret,
        };
        Ok(AES::decrypt(&file_encryption_key, &file.content)?)
    }
}

#[cfg(test)]
mod unit_test_symmetric {
    use crate::model::account::Account;
    use crate::service::crypto_service::{
        AesImpl, AesKey, DecryptedValue, PubKeyCryptoService, RsaImpl, SymmetricCryptoService,
    };
    use crate::service::file_encryption_service::{
        FileEncryptionService, FileEncryptionServiceImpl,
    };

    type File = FileEncryptionServiceImpl<RsaImpl, AesImpl>;

    #[test]
    fn test_file_generation() {
        let account = Account {
            username: "Parth".to_string(),
            keys: RsaImpl::generate_key().unwrap(),
        };

        let ef = File::new_file(&account).unwrap();

        assert_eq!(
            ef.access_keys.get(&account.username).unwrap().username,
            account.username
        );
        assert_eq!(
            ef.access_keys.get(&account.username).unwrap().public_key,
            account.keys.to_public_key()
        );

        let key = RsaImpl::decrypt(
            &account.keys,
            &ef.access_keys.get(&account.username).unwrap().access_key,
        )
        .unwrap()
        .secret;

        let aes = AesKey { key };

        assert_eq!(AesImpl::decrypt(&aes, &ef.content).unwrap().secret, "");

        RsaImpl::verify(&account.keys.to_public_key(), &ef.last_edited).unwrap();
    }

    #[test]
    fn test_file_editing() {
        let long_content = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Nunc congue nisi vitae suscipit tellus mauris a diam. Ipsum dolor sit amet consectetur adipiscing elit. Risus quis varius quam quisque id diam vel quam. Volutpat maecenas volutpat blandit aliquam etiam erat velit scelerisque. Risus quis varius quam quisque id diam vel quam elementum. Feugiat vivamus at augue eget arcu dictum varius duis. Habitant morbi tristique senectus et netus et malesuada fames ac. Fusce id velit ut tortor pretium viverra suspendisse potenti nullam. Aliquet nibh praesent tristique magna. Diam vel quam elementum pulvinar etiam non quam. Ipsum dolor sit amet consectetur adipiscing elit duis. Amet purus gravida quis blandit turpis cursus in hac habitasse. Sollicitudin aliquam ultrices sagittis orci a scelerisque purus. Dis parturient montes nascetur ridiculus mus mauris vitae ultricies. Nisl vel pretium lectus quam id leo in vitae. Aliquam ultrices sagittis orci a scelerisque. Nibh sed pulvinar proin gravida hendrerit lectus a. Viverra nibh cras pulvinar mattis nunc sed blandit libero volutpat. Risus feugiat in ante metus dictum. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin libero. Vulputate dignissim suspendisse in est ante in. Tortor id aliquet lectus proin nibh nisl condimentum id venenatis. Sit amet volutpat consequat mauris nunc congue nisi vitae suscipit. Sit amet risus nullam eget felis eget nunc. Maecenas volutpat blandit aliquam etiam erat velit scelerisque. Leo duis ut diam quam. Nulla at volutpat diam ut venenatis tellus in metus vulputate. Vitae turpis massa sed elementum tempus egestas sed sed. Aliquam vestibulum morbi blandit cursus. Feugiat pretium nibh ipsum consequat. Egestas sed sed risus pretium. Placerat orci nulla pellentesque dignissim enim sit. Dignissim sodales ut eu sem integer vitae. Elementum nibh tellus molestie nunc non blandit massa enim. Metus aliquam eleifend mi in nulla posuere sollicitudin aliquam ultrices. Enim ut sem viverra aliquet eget sit amet tellus. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin libero. Vulputate dignissim suspendisse in est ante in. Tortor id aliquet lectus proin nibh nisl condimentum id venenatis. Sit amet volutpat consequat mauris nunc congue nisi vitae suscipit. Sit amet risus nullam eget felis eget nunc. Maecenas volutpat blandit aliquam etiam erat velit scelerisque. Leo duis ut diam quam. Nulla at volutpat diam ut venenatis tellus in metus vulputate. Vitae turpis massa sed elementum tempus egestas sed sed. Aliquam vestibulum morbi blandit cursus. Feugiat pretium nibh ipsum consequat. Egestas sed sed risus pretium. Placerat orci nulla pellentesque dignissim enim sit. Dignissim sodales ut eu sem integer vitae. Elementum nibh tellus molestie nunc non blandit massa enim. Metus aliquam eleifend mi in nulla posuere sollicitudin aliquam ultrices. Enim ut sem viverra aliquet eget sit amet tellus.".to_string();

        let account = Account {
            username: "Parth".to_string(),
            keys: RsaImpl::generate_key().unwrap(),
        };

        let ef = File::new_file(&account).unwrap();

        let new_file = File::write_to_file(
            &account,
            &ef,
            &DecryptedValue {
                secret: long_content.clone(),
            },
        )
        .unwrap();

        let key = RsaImpl::decrypt(
            &account.keys,
            &new_file
                .access_keys
                .get(&account.username)
                .unwrap()
                .access_key,
        )
        .unwrap()
        .secret;

        assert_eq!(
            AesImpl::decrypt(&AesKey { key }, &new_file.content)
                .unwrap()
                .secret,
            long_content.to_string()
        );
    }

    #[test]
    fn test_read_file() {
        let long_content = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Nunc congue nisi vitae suscipit tellus mauris a diam. Ipsum dolor sit amet consectetur adipiscing elit. Risus quis varius quam quisque id diam vel quam. Volutpat maecenas volutpat blandit aliquam etiam erat velit scelerisque. Risus quis varius quam quisque id diam vel quam elementum. Feugiat vivamus at augue eget arcu dictum varius duis. Habitant morbi tristique senectus et netus et malesuada fames ac. Fusce id velit ut tortor pretium viverra suspendisse potenti nullam. Aliquet nibh praesent tristique magna. Diam vel quam elementum pulvinar etiam non quam. Ipsum dolor sit amet consectetur adipiscing elit duis. Amet purus gravida quis blandit turpis cursus in hac habitasse. Sollicitudin aliquam ultrices sagittis orci a scelerisque purus. Dis parturient montes nascetur ridiculus mus mauris vitae ultricies. Nisl vel pretium lectus quam id leo in vitae. Aliquam ultrices sagittis orci a scelerisque. Nibh sed pulvinar proin gravida hendrerit lectus a. Viverra nibh cras pulvinar mattis nunc sed blandit libero volutpat. Risus feugiat in ante metus dictum. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin libero. Vulputate dignissim suspendisse in est ante in. Tortor id aliquet lectus proin nibh nisl condimentum id venenatis. Sit amet volutpat consequat mauris nunc congue nisi vitae suscipit. Sit amet risus nullam eget felis eget nunc. Maecenas volutpat blandit aliquam etiam erat velit scelerisque. Leo duis ut diam quam. Nulla at volutpat diam ut venenatis tellus in metus vulputate. Vitae turpis massa sed elementum tempus egestas sed sed. Aliquam vestibulum morbi blandit cursus. Feugiat pretium nibh ipsum consequat. Egestas sed sed risus pretium. Placerat orci nulla pellentesque dignissim enim sit. Dignissim sodales ut eu sem integer vitae. Elementum nibh tellus molestie nunc non blandit massa enim. Metus aliquam eleifend mi in nulla posuere sollicitudin aliquam ultrices. Enim ut sem viverra aliquet eget sit amet tellus. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin libero. Vulputate dignissim suspendisse in est ante in. Tortor id aliquet lectus proin nibh nisl condimentum id venenatis. Sit amet volutpat consequat mauris nunc congue nisi vitae suscipit. Sit amet risus nullam eget felis eget nunc. Maecenas volutpat blandit aliquam etiam erat velit scelerisque. Leo duis ut diam quam. Nulla at volutpat diam ut venenatis tellus in metus vulputate. Vitae turpis massa sed elementum tempus egestas sed sed. Aliquam vestibulum morbi blandit cursus. Feugiat pretium nibh ipsum consequat. Egestas sed sed risus pretium. Placerat orci nulla pellentesque dignissim enim sit. Dignissim sodales ut eu sem integer vitae. Elementum nibh tellus molestie nunc non blandit massa enim. Metus aliquam eleifend mi in nulla posuere sollicitudin aliquam ultrices. Enim ut sem viverra aliquet eget sit amet tellus.".to_string();

        let account = Account {
            username: "Parth".to_string(),
            keys: RsaImpl::generate_key().unwrap(),
        };

        let ef = File::new_file(&account).unwrap();
        let new_file = File::write_to_file(
            &account,
            &ef,
            &DecryptedValue {
                secret: long_content.clone(),
            },
        )
        .unwrap();

        let content = File::read_file(&account, &new_file).unwrap().secret;

        assert_eq!(long_content.to_string(), content);
    }
}
