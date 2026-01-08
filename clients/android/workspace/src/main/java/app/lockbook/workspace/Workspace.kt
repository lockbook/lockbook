package app.lockbook.workspace
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

    // widget response
    @SerialName("selected_file")
    val selectedFile: String,
    @SerialName("doc_created")
    val docCreated: String,

    @SerialName("status_updated")
    val statusUpdated: Boolean,
    @SerialName("refresh_files")
    val refreshFiles: Boolean,

    @SerialName("new_folder_btn_pressed")
    val newFolderBtnPressed: Boolean,
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

    external fun initWS(surface: Surface, core: Long, scaleFactor: Float, darkMode: Boolean, oldWGPU: Long): Long
    external fun enterFrame(rustObj: Long): String
    external fun resizeWS(rustObj: Long, surface: Surface, scaleFactor: Float)
    external fun setBottomInset(rustObj: Long, inset: Int)

    external fun unfocusTitle(rustObj: Long)
    external fun touchesBegin(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesMoved(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesEnded(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesCancelled(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun sendKeyEvent(rustObj: Long, keyCode: Int, content: String, pressed: Boolean, alt: Boolean, ctrl: Boolean, shift: Boolean): Int
    external fun openDoc(rustObj: Long, id: String, newFile: Boolean)
    external fun closeDoc(rustObj: Long, id: String)
    external fun requestSync(rustObj: Long)
    external fun showTabs(rustObj: Long, show: Boolean)

    external fun getTabs(rustObj: Long) : Array<String>

    external fun currentTab(rustObj: Long): Int

    external fun getStatus(rustObj: Long): String

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
    external fun getAllText(rustObj: Long): String


    external fun selectAll(rustObj: Long)
    external fun clipboardCut(rustObj: Long)
    external fun clipboardCopy(rustObj: Long)
    external fun clipboardPaste(rustObj: Long, content: String)

    external fun toggleEraserSVG(rustObj: Long, select: Boolean)

    external fun insertTextAtCursor(rustObj: Long, text: String)
}

@Serializable
data class WsStatus(val syncing: Boolean, val msg: String)
@Serializable
data class JTextRange(val none: Boolean, val start: Int, val end: Int) {
    fun isEmpty(): Boolean = none || end - start == 0
}
@Serializable
data class JTextPosition(val none: Boolean, val position: Int)

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

fun String.isNullUUID(): Boolean {
    return this == "00000000-0000-0000-0000-000000000000"
}