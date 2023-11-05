package app.lockbook.egui_editor

import android.text.Editable
import android.text.InputFilter
import android.view.Surface
import kotlinx.serialization.SerialInfo
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
public data class EditorResponse(
    @SerialName("text_updated")
    val textUpdated: Boolean,
    @SerialName("potential_title")
    val potentialTitle: String?,

    @SerialName("show_edit_menu")
    val showEditMenu: Boolean,
    @SerialName("has_selection")
    val hasSelection: Boolean,
    @SerialName("selection_updated")
    val selectionUpdated: Boolean,
    @SerialName("edit_menu_x")
    val editMenuX: Float,
    @SerialName("edit_menu_y")
    val editMenuY: Float,

    @SerialName("cursor_in_heading")
    val cursorInHeading: Boolean,
    @SerialName("cursor_in_bullet_list")
    val cursorInBulletList: Boolean,
    @SerialName("cursor_in_number_list")
    val cursorInNumberList: Boolean,
    @SerialName("cursor_in_todo_list")
    val cursorInTodoList: Boolean,
    @SerialName("cursor_in_bold")
    val cursorInBold: Boolean,
    @SerialName("cursor_in_italic")
    val cursorInItalic: Boolean,
    @SerialName("cursor_in_inline_code")
    val cursorInInlineCode: Boolean,
    @SerialName("cursor_in_strikethrough")
    val cursorInStrikethrough: Boolean,

    @SerialName("opened_url")
    val openedURL: String?
)

@Serializable
data class IntegrationOutput(
    @SerialName("redraw_in")
    val redrawIn: ULong,
    @SerialName("editor_response")
    val editorResponse: EditorResponse
)

@Serializable
data class AndroidRect(
    @SerialName("min_x")
    val minX: Float,
    @SerialName("min_y")
    val minY: Float,
    @SerialName("max_x")
    val maxX: Float,
    @SerialName("max_y")
    val maxY: Float,
)

class EGUIEditor {
    init {
        System.loadLibrary("egui_editor")
    }

    external fun createWgpuCanvas(surface: Surface, core: Long, content: String, scaleFactor: Float, darkMode: Boolean): Long
    external fun enterFrame(rustObj: Long): String
    external fun resizeEditor(rustObj: Long, surface: Surface, scaleFactor: Float)
    external fun dropWgpuCanvas(rustObj: Long)

    external fun touchesBegin(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesMoved(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesEnded(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)

    external fun getAllText(rustObj: Long): String
    external fun setSelection(rustObj: Long, start: Int, end: Int)
    external fun getSelection(rustObj: Long): String
    external fun sendKeyEvent(rustObj: Long, keyCode: Int, content: String, pressed: Boolean, alt: Boolean, ctrl: Boolean, shift: Boolean): Int

    // Editable stuff
    external fun getTextLength(rustObj: Long): Int
    external fun clear(rustObj: Long)
    external fun replace(rustObj: Long, start: Int, end: Int, text: String)
    external fun insert(rustObj: Long, index: Int, text: String)
    external fun append(rustObj: Long, text: String)
    external fun getTextInRange(rustObj: Long, start: Int, end: Int): String

    // context menu
    external fun selectAll(rustObj: Long)
    external fun clipboardCut(rustObj: Long)
    external fun clipboardCopy(rustObj: Long)
    external fun clipboardPaste(rustObj: Long)
    external fun clipboardChanged(rustObj: Long, content: String)
    external fun hasCopiedText(rustObj: Long): Boolean
    external fun getCopiedText(rustObj: Long): String

    // markdown styling
    external fun applyStyleToSelectionHeading(rustObj: Long, headingSize: Int)

    external fun applyStyleToSelectionBulletedList(rustObj: Long)
    external fun applyStyleToSelectionNumberedList(rustObj: Long)
    external fun applyStyleToSelectionTodoList(rustObj: Long)

    external fun applyStyleToSelectionBold(rustObj: Long)
    external fun applyStyleToSelectionItalic(rustObj: Long)
    external fun applyStyleToSelectionInlineCode(rustObj: Long)
    external fun applyStyleToSelectionStrikethrough(rustObj: Long)

    external fun indentAtCursor(rustObj: Long, deindent: Boolean)

    external fun undoRedo(rustObj: Long, redo: Boolean)
}