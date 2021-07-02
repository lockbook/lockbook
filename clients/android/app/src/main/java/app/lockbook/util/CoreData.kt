package app.lockbook.util

import com.beust.klaxon.Json

data class ClientFileMetadata(
    val id: String = "",
    @Json(name = "file_type")
    val fileType: FileType = FileType.Document,
    val parent: String = "",
    val name: String = "",
    val owner: String = "",
    @Json(name = "metadata_version")
    val metadataVersion: Long = 0,
    @Json(name = "content_version")
    val contentVersion: Long = 0,
)

enum class FileType {
    Document, Folder
}

data class Account(
    val username: String,
    @Json(name = "api_url")
    val apiUrl: String,
    @Json(name = "private_key")
    val privateKey: List<Byte>,
)

data class WorkCalculated(
    @Json(name = "local_files")
    val localFiles: List<ClientFileMetadata>,
    @Json(name = "server_files")
    val serverFiles: List<ClientFileMetadata>,
    @Json(name = "server_unknown_name_count")
    val serverUnknownNameCount: Int,
    @Json(name = "most_recent_update_from_server")
    val mostRecentUpdateFromServer: Long
)

data class Config(val writeable_path: String)

enum class State {
    ReadyToUse,
    Empty,
    MigrationRequired,
    StateRequiresClearing
}

data class LocalAndServerUsages(
    @Json(name = "server_usage")
    val serverUsage: String,
    @Json(name = "uncompressed_usage")
    val uncompressedUsage: String,
    @Json(name = "data_cap")
    val dataCap: String,
)
