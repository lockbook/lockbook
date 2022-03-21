package app.lockbook.util

import android.content.res.Resources
import com.beust.klaxon.Json
import kotlinx.serialization.Serializable

import kotlinx.serialization.*
import kotlinx.serialization.json.*

@Serializable
data class DecryptedFileMetadata(
    val id: String = "",
    @SerialName("file_type")
    val fileType: FileType = FileType.Document,
    val parent: String = "",
    @SerialName("decrypted_name")
    val decryptedName: String = "",
    val owner: String = "",
    @SerialName("metadata_version")
    val metadataVersion: Long = 0,
    @SerialName("content_version")
    val contentVersion: Long = 0,
    val deleted: Boolean = false,
    @SerialName("decrypted_access_key")
    val decryptedAccessKey: List<Int> = listOf()
)

enum class FileType {
    Document, Folder
}

@Serializable
class Account(
    val username: String,
    @SerialName("api_url")
    val apiUrl: String,
    @SerialName("private_key")
    val privateKey: Array<Int>
)

@Serializable
data class WorkCalculated(
    @SerialName("work_units")
    val workUnits: List<WorkUnit>,
    @SerialName("most_recent_update_from_server")
    val mostRecentUpdateFromServer: Long,
)

@Serializable
data class WorkUnit(val content: DecryptedFileMetadata, val tag: String)

@Serializable
data class Config(val writeable_path: String)

@Serializable
enum class State {
    ReadyToUse,
    Empty,
    MigrationRequired,
    StateRequiresClearing
}

@Serializable
data class UsageMetrics(
    val usages: List<FileUsage>,
    @SerialName("server_usage")
    val serverUsage: UsageItemMetric,
    @SerialName("data_cap")
    val dataCap: UsageItemMetric,
)

@Serializable
data class UsageItemMetric(
    val exact: Int,
    val readable: String,
)

@Serializable
data class FileUsage(
    @SerialName("file_id")
    val fileId: String,
    @SerialName("size_bytes")
    val sizeBytes: Int,
)

inline fun <reified T : Enum<T>> String.asEnumOrDefault(defaultValue: T? = null): T? =
    enumValues<T>().firstOrNull { it.name.equals(this, ignoreCase = true) } ?: defaultValue
