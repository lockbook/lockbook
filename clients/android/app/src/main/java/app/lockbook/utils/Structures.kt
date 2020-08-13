package app.lockbook.utils

import java.util.LinkedHashMap

data class FileMetadata(
    val id: String,
    val file_type: FileType,
    val parent: String,
    val name: String,
    val owner: String,
    val signature: SignedValue,
    val metadata_version: Long,
    val content_version: Long,
    val deleted: Boolean,
    val user_access_keys: LinkedHashMap<String, UserAccessInfo>,
    val folder_access_keys: FolderAccessInfo
)

data class SignedValue(
    val content: String,
    val signature: String
)

data class FolderAccessInfo(
    val folder_id: String,
    val access_key: EncryptedValueWithNonce
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
    val public_key: String,
    val access_key: EncryptedValue
)

data class EncryptedValue(
    val garbage: String
)

data class DecryptedValue(
    val secret: String
)

data class Account(
    val username: String,
    val keys: String
)

data class WorkCalculated(
    val work_units: List<WorkUnit>,
    val most_recent_update_from_server: Int
)

data class WorkUnit(
    val LocalChange: LocalChange,
    val ServerChange: ServerChange
)

data class LocalChange(val metadata: FileMetadata)
data class ServerChange(val metadata: FileMetadata)

data class Config(val writeable_path: String)

data class EditableFile(
    val name: String,
    val id: String,
    val contents: String
)
