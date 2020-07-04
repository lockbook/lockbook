package app.lockbook

import com.beust.klaxon.Json

data class ClientFileMetadata(
    @Json(name = "file_type", ignored = true)
    val fileType: FileType,
    @Json(ignored = true)
    val id: String, // THIS IS REALLY TYPE Uuid
    val name: String,
    @Json(name = "parent_id", ignored = true)
    val parentId: String, // THIS IS REALLY TYPE Uuid
    @Json(name = "content_version")
    val contentVersion: Int,
    @Json(name = "metadata_version")
    val metadataVersion: Int,
    @Json(name = "user_access_keys", ignored = true)
    val userAccessKeys: HashMap<String, String>, // THIS IS REALLY TYPE HashMap<String, UserAccessInfo>
    @Json(name = "folder_access_keys", ignored = true)
    val folderAccessKeys: String, // THIS IS REALLY TYPE FolderAccessInfo
    val new: Boolean,
    @Json(name = "document_edited")
    val documentEdited: Boolean,
    @Json(name = "metadata_changed")
    val metadataChanged: Boolean,
    val deleted: Boolean
)

enum class FileType {
    Document, Folder
}

