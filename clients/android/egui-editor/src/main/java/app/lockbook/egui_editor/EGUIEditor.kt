package app.lockbook.egui_editor

import android.text.Editable
import android.text.InputFilter
import android.view.Surface
import kotlinx.serialization.SerialInfo
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

public class EGUIEditor {
    init {
        System.loadLibrary("egui_editor")
    }

    external fun createWgpuCanvas(surface: Surface, core: Long, content: String, scaleFactor: Float, darkMode: Boolean): Long
    external fun enterFrame(rustObj: Long): String
    external fun resizeEditor(rustObj: Long, surface: Surface, scaleFactor: Float)

    external fun touchesBegin(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesMoved(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)
    external fun touchesEnded(rustObj: Long, id: Int, x: Float, y: Float, pressure: Float)

    external fun addText(rustObj: Long, content: String)
    external fun dropWgpuCanvas(rustObj: Long)
    external fun getTextBeforeCursor(rustObj: Long, n: Int): String
    external fun getTextAfterCursor(rustObj: Long, n: Int): String
    external fun getAllText(rustObj: Long): String
    external fun getSelectedText(rustObj: Long): String
    external fun deleteSurroundingText(rustObj: Long, beforeLength: Int, afterlength: Int)
    external fun setSelection(rustObj: Long, start: Int, end: Int)
    external fun getSelection(rustObj: Long): String
    external fun sendKeyEvent(rustObj: Long, keyCode: Int, content: String, pressed: Boolean, alt: Boolean, ctrl: Boolean, shift: Boolean): Int
}

@Serializable
data class EditorResponse(
    @SerialName("text_updated")
    val textUpdated: Boolean,
    @SerialName("potential_title")
    val potentialTitle: String?,
    @SerialName("has_selection")
    val hasSelection: Boolean,
    @SerialName("selection_updated")
    val selectionUpdated: Boolean,
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
)

@Serializable
data class IntegrationOutput(
    @SerialName("redraw_in")
    val redrawIn: ULong,
    @SerialName("editor_response")
    val editorResponse: EditorResponse
)