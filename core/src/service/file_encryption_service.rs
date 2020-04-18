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
    fn read_file(key: &Account, file: EncryptedFile) -> Result<DecryptedValue, UnableToReadFile>;
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
        file: EncryptedFile,
    ) -> Result<DecryptedValue, UnableToReadFile> {
        let encrypted_key = &file.access_keys.get(&account.username)?.access_key;
        let file_encryption_key = AesKey {
            key: PK::decrypt(&account.keys, encrypted_key)?.secret,
        };
        Ok(AES::decrypt(&file_encryption_key, &file.content)?)
    }
}
