package app.lockbook.util

import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import app.lockbook.screen.WorkspaceTextInputWrapper
import app.lockbook.workspace.Workspace

/**
 * The keyboard's link to a text field of the workspace's own chrome, such as
 * the chat's model filter: typed text goes to egui as text and keys, not as
 * edits to a document. Composing text is typed, then erased and retyped as
 * the keyboard revises it.
 */
class ChromeInputConnection(
    private val wrapper: WorkspaceTextInputWrapper,
) : BaseInputConnection(wrapper, false) {
    private var composing = 0

    private fun key(code: Int) {
        for (pressed in listOf(true, false)) {
            Workspace.sendKeyEvent(WorkspaceView.wgpuObj, code, "", pressed, false, false, false)
        }
    }

    private fun type(text: CharSequence?): Int {
        val typed = text?.toString().orEmpty()
        if (typed.isNotEmpty()) {
            Workspace.insertTextAtCursor(WorkspaceView.wgpuObj, typed)
        }
        return typed.codePointCount(0, typed.length)
    }

    private fun eraseComposing() {
        repeat(composing) { key(KeyEvent.KEYCODE_DEL) }
        composing = 0
    }

    override fun commitText(
        text: CharSequence?,
        newCursorPosition: Int,
    ): Boolean {
        eraseComposing()
        type(text)
        wrapper.workspaceView.drawImmediately()
        return true
    }

    override fun setComposingText(
        text: CharSequence?,
        newCursorPosition: Int,
    ): Boolean {
        eraseComposing()
        composing = type(text)
        wrapper.workspaceView.drawImmediately()
        return true
    }

    override fun finishComposingText(): Boolean {
        composing = 0
        return true
    }

    override fun deleteSurroundingText(
        beforeLength: Int,
        afterLength: Int,
    ): Boolean {
        repeat(beforeLength) { key(KeyEvent.KEYCODE_DEL) }
        repeat(afterLength) { key(KeyEvent.KEYCODE_FORWARD_DEL) }
        wrapper.workspaceView.drawImmediately()
        return true
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        if (event != null) {
            wrapper.wsInputConnection.forwardWorkspaceKeyEvent(event)
            wrapper.workspaceView.drawImmediately()
        }
        return true
    }

    override fun performEditorAction(actionCode: Int): Boolean {
        key(KeyEvent.KEYCODE_ENTER)
        wrapper.workspaceView.drawImmediately()
        return true
    }
}
