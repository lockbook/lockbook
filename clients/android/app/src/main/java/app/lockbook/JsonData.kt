package app.lockbook

import com.beust.klaxon.Json
import java.security.interfaces.RSAPublicKey
import java.util.*
import kotlin.collections.HashMap

data class FileMetadata(

    val id: UUID,
    @Json(name = "file_type")
    val fileType: FileType,
    val parent: UUID,
    val name: String,
    val owner: String,
    val signature: SignedValue,
    @Json(name = "metadata_version")
    val metadataVersion: Int,
    @Json(name = "content_version")
    val contentVersion: Int,
    val deleted: Boolean,
    @Json(name = "user_access_keys")
    val userAccessKeys: HashMap<String, UserAccessInfo>,
    @Json(name = "folder_access_keys")
    val folderAccessKeys: FolderAccessInfo
)

data class SignedValue(
    val content: String,
    val signature: String
)

data class FolderAccessInfo(
    @Json(name = "folder_id")
    val folderId: UUID,
    @Json(name = "access_key")
    val accessKey: EncryptedValueWithNonce
)

data class EncryptedValueWithNonce(
    val garbage: String,
    val nonce: String
)

enum class FileType {
    Document, Folder
}

data class UserAccessInfo(
    val username: String,
    @Json(name = "public_key")
    val publicKey: RSAPublicKey,
    @Json(name = "access_key")
    val accessKey: EncryptedValue
)

data class EncryptedValue(
    val garbage: String
)

data class Document(
    val content: EncryptedValueWithNonce
)