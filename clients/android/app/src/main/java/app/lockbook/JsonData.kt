package app.lockbook

import com.beust.klaxon.Json
import java.security.interfaces.RSAPublicKey
import java.util.*
import kotlin.collections.HashMap

data class ClientFileMetadata(
    @Json(name = "file_type")
    val fileType: FileType,
    val id: String,
    val name: String,
    @Json(name = "parent_id")
    val parentId: UUID,
    @Json(name = "content_version")
    val contentVersion: Int,
    @Json(name = "metadata_version")
    val metadataVersion: Int,
    @Json(name = "user_access_keys", ignored = true)
    val userAccessKeys: HashMap<String, UserAccessInfo>,
    @Json(name = "folder_access_keys", ignored = true)
    val folderAccessKeys: FolderAccessInfo,
    val new: Boolean,
    @Json(name = "document_edited")
    val documentEdited: Boolean,
    @Json(name = "metadata_changed")
    val metadataChanged: Boolean,
    val deleted: Boolean
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