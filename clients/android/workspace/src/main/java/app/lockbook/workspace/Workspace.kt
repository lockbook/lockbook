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


data class AndroidResponse(
    val redrawIn: Long,
    val copiedText: String,
    val hasURLOpened: Boolean,
    val urlOpened: String,
    val virtualKeyboardShown: Boolean?,
    val selectedFile: String,
    val docCreated: String,
    val tabsChanged: Boolean,
    val hasEditMenu: Boolean,
    val editMenuX: Float,
    val editMenuY: Float,
    val selectionUpdated: Boolean,
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
    external fun initWSOffloaded(surface: Surface, core: Long, darkMode: Boolean): Long
    external fun dropWS(ptr: Long)

    external fun enterFrame(rustObj: Long): AndroidResponse
    external fun enterFrameOffloaded(rustObj: Long): AndroidResponse
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
    external fun back(rustObj: Long): Boolean
    external fun forward(rustObj: Long): Boolean
    external fun canForward(rustObj: Long): Boolean

    external fun getTabs(rustObj: Long) : Array<String>

    external fun currentTab(rustObj: Long): NativeWorkspaceTab

    external fun fileRenamed(rustObj: Long, id: String, name: String): Int

    // text input
    external fun setSelection(rustObj: Long, start: Int, end: Int)
    external fun getSelection(rustObj: Long): JTextRange
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
    external fun clipboardSendImage(rustObj: Long, content: ByteArray, isPaste: Boolean)
    external fun isPenOnlyDraw(rustObj: Long) : Boolean
    external fun insertTextAtCursor(rustObj: Long, text: String)
}

data class NativeWorkspaceTab(
    val id: String,
    val type: Int,
)


@SuppressLint("UnsafeOptInUsageError")
@Serializable
data class JTextRange(val none: Boolean, val start: Int, val end: Int) {
    fun isEmpty(): Boolean = none || end - start == 0
}


const val NULL_UUID = "00000000-0000-0000-0000-000000000000"

fun String.isNullUUID(): Boolean {
    return this == NULL_UUID
}
