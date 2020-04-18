use std::collections::HashMap;

use rsa::RSAPublicKey;
use serde::export::PhantomData;

use crate::error_enum;
use crate::model::account::Account;
use crate::service::crypto_service::{
    AesEncryptionFailed, DecryptedValue, EncryptedValue, EncryptedValueWithNonce,
    PubKeyCryptoService, SignedValue, SymmetricCryptoService,
};

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

trait FileEncryptionService {
    fn new_file(author: &Account) -> Result<EncryptedFile, FileCreationError>;
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
}
