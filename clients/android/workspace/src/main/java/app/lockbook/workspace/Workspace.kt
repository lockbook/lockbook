package app.lockbook.workspace
import android.text.Editable
import android.text.InputFilter
import android.view.Surface
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialInfo
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.jsonPrimitive
import java.math.BigInteger
import java.util.UUID

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
public data class IntegrationOutput(
    @SerialName("workspace_resp")
    val workspaceResp: FfiWorkspaceResp,
    @Serializable(with = BigIntegerSerializer::class)
    @SerialName("redraw_in")
    val redrawIn: BigInteger,
    @SerialName("has_copied_text")
    val hasCopiedText: Boolean,
    @SerialName("copied_text")
    val copiedText: String,
    @SerialName("url_opened")
    val urlOpened: String
)

@Serializable
public data class FfiWorkspaceResp(
    @SerialName("selected_file")
    val selectedFile: String,
    @SerialName("doc_created")
    val docCreated: String,
    val msg: String,
    val syncing: Boolean,
    @SerialName("refresh_files")
    val refreshFiles: Boolean,
    @SerialName("new_folder_btn_pressed")
    val newFolderBtnPressed: Boolean,
    @SerialName("tab_title_clicked")
    val tabTitleClicked: Boolean,

    @SerialName("show_edit_menu")
    val showEditMenu: Boolean,
    @SerialName("edit_menu_x")
    val editMenuX: Float,
    @SerialName("edit_menu_y")
    val editMenuY: Float,

    @SerialName("selection_updated")
    val selectionUpdated: Boolean
)

class Workspace private constructor() {

    init {
        System.loadLibrary("workspace")
    }

    companion object {
        private var workspace: Workspace? = null

        fun getInstance(): Workspace {
            if(workspace == null) {
                workspace = Workspace()
            }

            return workspace!!
        }
    }

    external fun initWS(surface: Surface, core: Long, scaleFactor: Float, darkMode: Boolean, oldWGPU: Long): Long
    external fun enterFrame(rustObj: Long): String
    external fun resizeEditor(rustObj: Long, surface: Surface, scaleFactor: Float)

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
    external fun textOffsetForPosition(rustObj: Long, x: Float, y: Float): String

    external fun selectAll(rustObj: Long)
    external fun clipboardCut(rustObj: Long)
    external fun clipboardCopy(rustObj: Long)
    external fun clipboardPaste(rustObj: Long, content: String)

    external fun getComposing(rustObj: Long): String
    external fun setComposing(rustObj: Long, none: Boolean, start: Int, end: Int, text: String)
    external fun uncomposeText(rustObj: Long)

    external fun toggleEraserSVG(rustObj: Long, select: Boolean)

    external fun getCursorRect(rustObj: Long): String

    external fun insertTextAtCursor(rustObj: Long, text: String)
}

@Serializable
data class JTextRange(val none: Boolean, val start: Int, val end: Int)
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