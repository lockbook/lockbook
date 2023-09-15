package app.lockbook.workspace
import android.annotation.SuppressLint
import android.view.Surface
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.jsonPrimitive
import java.math.BigInteger

// Examine performance improvements with borsh.io

object BigIntegerSerializer: KSerializer<BigInteger> {
    override fun deserialize(decoder: Decoder): BigInteger {
        return if (decoder is JsonDecoder) {
            BigInteger(decoder.decodeJsonElement().jsonPrimitive.content)
        } else {
            BigInteger(decoder.decodeString())
        }
    }

    override fun serialize(encoder: Encoder, value: BigInteger) {
        encoder.encodeString(value.toString())
    }

    override val descriptor: SerialDescriptor
        get() = PrimitiveSerialDescriptor("java.math.BigInteger", PrimitiveKind.LONG)
}

@SuppressLint("UnsafeOptInUsageError")
@Serializable
public data class AndroidResponse(
    // platform response
    @SerialName("redraw_in")
    val redrawIn: ULong,
    @SerialName("copied_text")
    val copiedText: String,
    @SerialName("has_url_opened")
    val hasURLOpened: Boolean,
    @SerialName("url_opened")
    val urlOpened: String,

    @SerialName("virtual_keyboard_shown")
    val virtualKeyboardShown: Boolean?,

    // widget response
    @SerialName("selected_file")
    val selectedFile: String,
    @SerialName("doc_created")
    val docCreated: String,

    @SerialName("tab_title_clicked")
    val tabTitleClicked: Boolean,
    @SerialName("tabs_changed")
    val tabsChanged: Boolean,
    
    @SerialName("has_edit_menu")
    val hasEditMenu: Boolean,
    @SerialName("edit_menu_x")
    val editMenuX: Float,
    @SerialName("edit_menu_y")
    val editMenuY: Float,

    @SerialName("selection_updated")
    val selectionUpdated: Boolean,
    @SerialName("text_updated")
    val textUpdated: Boolean
)

object Workspace {

    init {
        System.loadLibrary("workspace")
    }

    // dummy init to load workspace and lb-java lib
    fun init() {
        print("do nothing")
    }

    external fun initWS(surface: Surface, core: Long, darkMode: Boolean): Long
    external fun dropWS(ptr: Long)

    external fun enterFrame(rustObj: Long): String
    external fun resizeWS(rustObj: Long, surface: Surface, scaleFactor: Float)
    external fun setBottomInset(rustObj: Long, inset: Int)

    external fun unfocusTitle(rustObj: Long)
    external fun touchesBegin(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesMoved(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesPredicted(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)

    external fun mouseMoved(rustObj: Long, x: Float, y: Float)

    external fun touchesEnded(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesCancelled(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun multiTouch(rustObj: Long, x: Float, y: Float, factor: Float, focusX: Float, focusY: Float, startX: FloatArray, startY: FloatArray)

    external fun sendKeyEvent(rustObj: Long, keyCode: Int, content: String, pressed: Boolean, alt: Boolean, ctrl: Boolean, shift: Boolean): Int
    external fun openDoc(rustObj: Long, id: String, newFile: Boolean) : Int

    external fun createDocAt(rustObj: Long, isDrawing: Boolean, parent: String)

    external fun closeDoc(rustObj: Long, id: String)
    external fun closeAllTabs(rustObj: Long)
    external fun showTabs(rustObj: Long, show: Boolean)
    external fun back(rustObj: Long): Boolean
    external fun forward(rustObj: Long): Boolean

    external fun getTabs(rustObj: Long) : Array<String>

    external fun currentTab(rustObj: Long): Int

    external fun fileRenamed(rustObj: Long, id: String, name: String): Int

    // text input
    external fun setSelection(rustObj: Long, start: Int, end: Int)
    external fun getSelection(rustObj: Long): String
    external fun getTextLength(rustObj: Long): Int
    external fun clear(rustObj: Long)
    external fun replace(rustObj: Long, start: Int, end: Int, text: String)
    external fun insert(rustObj: Long, index: Int, text: String)
    external fun append(rustObj: Long, text: String)
    external fun getTextInRange(rustObj: Long, start: Int, end: Int): String

    external fun getBuffer(rustObj: Long): String

    external fun getAllText(rustObj: Long): String


    external fun selectAll(rustObj: Long)
    external fun clipboardCut(rustObj: Long)
    external fun clipboardCopy(rustObj: Long)
    external fun clipboardPaste(rustObj: Long, content: String)
    external fun isPenOnlyDraw(rustObj: Long) : Boolean
    external fun insertTextAtCursor(rustObj: Long, text: String)
}
@SuppressLint("UnsafeOptInUsageError")
@Serializable
public data class LbStatus(
    /// some recent server interaction failed due to network conditions
    @SerialName("offline")
    val offline: Boolean = false,

    /// a sync is in progress
    @SerialName("syncing")
    val syncing: Boolean = false,

    /// at-least one document cannot be pushed due to a data cap
    @SerialName("out_of_space")
    val outOfSpace: Boolean = false,

    /// there are pending shares
    @SerialName("pending_shares")
    val pendingShares: Boolean = false,

    /// you must update to be able to sync, see update_available below
    @SerialName("update_required")
    val updateRequired: Boolean = false,

    /// metadata or content for this id is being sent to the server
    @SerialName("pushing_files")
    val pushingFiles: List<String> = emptyList(),

    /// following files need to be pushed
    @SerialName("dirty_locally")
    val dirtyLocally: List<String> = emptyList(),

    /// metadata or content for this id is being pulled from the server
    /// callers should be prepared to handle ids they don't know about yet
    @SerialName("pulling_files")
    val pullingFiles: List<String> = emptyList(),

    @SerialName("space_used")
    val spaceUsed: SpaceUsed? = null,

    /// if there is no pending work this will have a human readable
    /// description of when we last synced successfully
    @SerialName("sync_status")
    val syncStatus: String? = null,

    @SerialName("unexpected_sync_problem")
    val unexpectedSyncProblem: String? = null,
)

@SuppressLint("UnsafeOptInUsageError")
@Serializable
data class SpaceUsed(
    @SerialName("usages")
    val usages: List<FileUsage> = emptyList(),

    @SerialName("server_usage")
    val serverUsage : UsageItemMetric? = null,

    @SerialName("data_cap")
    val dataCap : UsageItemMetric? = null,
)

@SuppressLint("UnsafeOptInUsageError")
@Serializable
data class UsageItemMetric (
    @SerialName("exact")
    var exact: Long? = null,

    @SerialName("readable")
    var readable: String? = null
)

@SuppressLint("UnsafeOptInUsageError")
@Serializable
data class FileUsage (
    @SerialName("file_id")
    var fileId: String? = null,

    @SerialName("size_bytes")
    var sizeBytes: Long? = null
)

@SuppressLint("UnsafeOptInUsageError")
@Serializable
data class JTextRange(val none: Boolean, val start: Int, val end: Int) {
    fun isEmpty(): Boolean = none || end - start == 0
}
@SuppressLint("UnsafeOptInUsageError")
@Serializable
data class JTextPosition(val none: Boolean, val position: Int)

@SuppressLint("UnsafeOptInUsageError")
@Serializable
data class JRect(
    @SerialName("min_x")
    val minX: Float,
    @SerialName("min_y")
    val minY: Float,
    @SerialName("max_x")
    val maxX: Float,
    @SerialName("max_y")
    val maxY: Float
)

const val NULL_UUID = "00000000-0000-0000-0000-000000000000"

fun String.isNullUUID(): Boolean {
    return this == NULL_UUID
}
