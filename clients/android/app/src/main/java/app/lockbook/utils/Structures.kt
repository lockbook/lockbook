package app.lockbook.utils

import java.util.*

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
    val public_key: RSAPublicKey,
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
    val keys: RSAPrivateKey
)

data class RSAPrivateKey(
    val n: List<Int>,
    val e: List<Int>,
    val d: List<Int>,
    val primes: List<String>
)

data class RSAPublicKey(
    val n: List<Int>,
    val e: List<Int>
)

data class WorkCalculated(
    val work_units: List<WorkUnit>,
    val most_recent_update_from_server: Long
)

data class WorkUnit(
    val tag: String,
    val content: WorkUnitMetadata
)

data class WorkUnitMetadata(val metadata: FileMetadata)

data class Config(val writeable_path: String)

data class EditableFile(
    val name: String,
    val id: String,
)

data class SyncingStatus(
    var isSyncing: Boolean = false,
    var maxProgress: Int = 0
)

data class DialogStatus(
    var isDialogOpen: Boolean = false,
    var alertDialogFileName: String = ""
)

data class Drawing(
    val page: Page = Page(),
    val events: MutableList<Event> = mutableListOf()
)

data class Event(
    val stroke: Stroke? = null
)

data class Stroke(
    val color: Int,
    val points: MutableList<PressurePoint> = mutableListOf()
)

data class Page(
    var transformation: Transformation = Transformation(),
)

data class Transformation(
    var translation: Point = Point(0f, 0f),
    var scale: Float = 2f,
    var scaleFocus: Point = Point(0f, 0f),
    var rotation: Float = 0f
)

data class PressurePoint(
    val x: Float,
    val y: Float,
    val pressure: Float
)

data class Point(
    var x: Float,
    var y: Float
)
