pub fn encrypt_document(
    document: &[u8], metadata: &CoreFile,
) -> Result<EncryptedDocument, CoreError> {
    symkey::encrypt(&metadata.decrypted_access_key, &document.to_vec()).map_err(core_err_unexpected)
}

pub fn decrypt_document(
    document: &EncryptedDocument, metadata: &CoreFile,
) -> Result<DecryptedDocument, CoreError> {
    symkey::decrypt(&metadata.decrypted_access_key, document).map_err(core_err_unexpected)
}
