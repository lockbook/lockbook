package app.lockbook.workspace
import android.text.Editable
import android.text.InputFilter
import android.view.Surface
import kotlinx.serialization.SerialInfo
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import java.util.UUID

public data class IntegrationOutput(
    @SerialName("workspace_resp")
    val workspaceResp: FfiWorkspaceResp,
    @SerialName("redraw_in")
    val redrawIn: ULong,
    @SerialName("copied_text")
    val copiedText: String,
    @SerialName("url_opened")
    val urlOpened: String
)

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
    val newFolderBtnPressed: Boolean
)

class Workspace {
    init {
        System.loadLibrary("workspace")
    }

    external fun createWgpuCanvas(surface: Surface, core: Long, content: String, scaleFactor: Float, darkMode: Boolean): Long
    external fun enterFrame(rustObj: Long): String
    external fun resizeEditor(rustObj: Long, surface: Surface, scaleFactor: Float)
    external fun dropWgpuCanvas(rustObj: Long)

    external fun touchesBegin(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesMoved(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesEnded(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun sendKeyEvent(rustObj: Long, keyCode: Int, content: String, pressed: Boolean, alt: Boolean, ctrl: Boolean, shift: Boolean): Int

//    external fun getAllText(rustObj: Long): String
//    external fun setSelection(rustObj: Long, start: Int, end: Int)
//    external fun getSelection(rustObj: Long): String
//
//    // Editable stuff
//    external fun getTextLength(rustObj: Long): Int
//    external fun clear(rustObj: Long)
//    external fun replace(rustObj: Long, start: Int, end: Int, text: String)
//    external fun insert(rustObj: Long, index: Int, text: String)
//    external fun append(rustObj: Long, text: String)
//    external fun getTextInRange(rustObj: Long, start: Int, end: Int): String
//
//    // context menu
//    external fun selectAll(rustObj: Long)
//    external fun clipboardCut(rustObj: Long)
//    external fun clipboardCopy(rustObj: Long)
//    external fun clipboardPaste(rustObj: Long)
//    external fun clipboardChanged(rustObj: Long, content: String)
//    external fun hasCopiedText(rustObj: Long): Boolean
//    external fun getCopiedText(rustObj: Long): String
//
//    // markdown styling
//    external fun applyStyleToSelectionHeading(rustObj: Long, headingSize: Int)
//
//    external fun applyStyleToSelectionBulletedList(rustObj: Long)
//    external fun applyStyleToSelectionNumberedList(rustObj: Long)
//    external fun applyStyleToSelectionTodoList(rustObj: Long)
//
//    external fun applyStyleToSelectionBold(rustObj: Long)
//    external fun applyStyleToSelectionItalic(rustObj: Long)
//    external fun applyStyleToSelectionInlineCode(rustObj: Long)
//    external fun applyStyleToSelectionStrikethrough(rustObj: Long)
//
//    external fun indentAtCursor(rustObj: Long, deindent: Boolean)
//
//    external fun undoRedo(rustObj: Long, redo: Boolean)
}