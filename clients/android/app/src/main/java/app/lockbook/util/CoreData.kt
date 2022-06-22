package app.lockbook.util

import android.content.res.Resources
import app.lockbook.R
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

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
) {
    fun isRoot() = parent == id
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
data class WorkCalculated(
    @SerialName("work_units")
    val workUnits: List<WorkUnit>,
    @SerialName("most_recent_update_from_server")
    val mostRecentUpdateFromServer: Long,
)

@Serializable
data class WorkUnit(val content: WorkUnitMetadata, val tag: String)

@Serializable
data class WorkUnitMetadata(val metadata: DecryptedFileMetadata)

@Serializable
data class Config(
    val logs: Boolean,
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
