package app.lockbook.util

import android.content.res.Resources
import android.os.Parcelable
import app.lockbook.R
import kotlinx.parcelize.Parcelize
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

@Parcelize
@Serializable
data class File(
    val id: String = "",
    val parent: String = "",
    val name: String = "",
    @SerialName("file_type")
    val fileType: FileType = FileType.Document,
    @SerialName("last_modified")
    val lastModified: Long = 0,
    @SerialName("last_modified_by")
    val lastModifiedBy: String = "",
    val shares: List<Share> = listOf()
) : Parcelable {
    fun isRoot() = parent == id
    fun isFolder() = fileType == FileType.Folder
}

@Parcelize
@Serializable
data class Share(
    val mode: ShareMode,
    @SerialName("shared_by")
    val sharedBy: String,
    @SerialName("shared_with")
    val sharedWith: String,
) : Parcelable

@Parcelize
enum class ShareMode : Parcelable {
    Write,
    Read,
}

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
data class SyncStatus(
    @SerialName("work_units")
    val workUnits: List<WorkUnit>,
    @SerialName("latest_server_ts")
    val latestServerTS: Long,
)

@Serializable
data class WorkUnit(val content: WorkUnitMetadata, val tag: WorkUnitTag)

enum class WorkUnitTag {
    LocalChange,
    ServerChange
}

@Serializable
data class WorkUnitMetadata(val metadata: File)

@Serializable
data class Config(
    val logs: Boolean,
    val colored_logs: Boolean,
    val writeable_path: String
)

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
    val exact: Long,
    val readable: String,
)

@Serializable
data class FileUsage(
    @SerialName("file_id")
    val fileId: String,
    @SerialName("size_bytes")
    val sizeBytes: Int,
)

@Serializable
data class SubscriptionInfo(
    @SerialName("payment_platform")
    val paymentPlatform: PaymentPlatform,
    @SerialName("period_end")
    val periodEnd: Long
)

@Serializable
@JsonClassDiscriminator("tag")
sealed class PaymentPlatform {
    @Serializable
    @SerialName("GooglePlay")
    data class GooglePlay(
        @SerialName("account_state")
        val accountState: GooglePlayAccountState
    ) : PaymentPlatform()

    @Serializable
    @SerialName("Stripe")
    data class Stripe(
        @SerialName("card_last_4_digits")
        val cardLast4Digits: String
    ) : PaymentPlatform()

    fun toReadableString(resources: Resources): String = when (this) {
        is GooglePlay -> resources.getString(R.string.google_play)
        is Stripe -> resources.getString(R.string.stripe)
    }
}

enum class GooglePlayAccountState {
    Ok,
    Canceled,
    GracePeriod,
    OnHold
}

@Serializable
data class ContentMatch(
    val paragraph: String,
    @SerialName("matched_indices")
    val matchedIndices: List<Int>,
    val score: Int
)
