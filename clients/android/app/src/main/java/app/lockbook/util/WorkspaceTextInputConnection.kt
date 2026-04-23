package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.text.Editable
import android.text.Selection
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.view.inputmethod.TextAttribute
import app.lockbook.App
import app.lockbook.screen.WorkspaceTextInputWrapper
import java.util.concurrent.atomic.AtomicReference


data class CursorMonitorStatus(var monitor: Boolean = false, var editorBounds: Boolean = false, var characterBounds: Boolean = false, var insertionMarker: Boolean = false)

@SuppressLint("SoonBlockedPrivateApi")
class WorkspaceTextInputConnection(val workspaceView: WorkspaceView, val textInputWrapper: WorkspaceTextInputWrapper) : BaseInputConnection(textInputWrapper, true) {
    private val logger = AppLogger.getLogger("InputBridging")

    val wsEditable = WorkspaceTextEditable(workspaceView, this)

    var batchEditCount = AtomicReference(0)

    private var cursorMonitorStatus = CursorMonitorStatus()

    private fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    private fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun notifySelectionUpdated(isImmediate: Boolean = false) {
        pushTextMutationEvent(WorkspaceView.WsTextMutation.NotifySelectionUpdate)
    }

    fun pushTextMutationEvent(event: WorkspaceView.WsTextMutation){
        workspaceView.textMutations.get().add(event to workspaceView.pendingWorkspaceTextState.get())
    }
    fun applySelectionNotification(isImmediate: Boolean = false) {
//        logger.i("APPLY SEL")

        getInputMethodManager().updateSelection(
            textInputWrapper,
            wsEditable.selectionStart.get(),
            wsEditable.selectionEnd.get(),
            wsEditable.composingStart,
            wsEditable.composingEnd
        )
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)
        logger.i("SNED KEY")

        if (event != null) {
            val content = event.unicodeChar.toChar().toString()
            pushTextMutationEvent(WorkspaceView.WsTextMutation.SendKeyEvent(
                event.keyCode,
                content,
                event.action == KeyEvent.ACTION_DOWN,
                event.isAltPressed,
                event.isCtrlPressed,
                event.isShiftPressed
            ))
        }

        workspaceView.drawImmediately()

        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {

        when (id) {
            android.R.id.selectAll ->pushTextMutationEvent(WorkspaceView.WsTextMutation.SelectAll)
            android.R.id.cut ->pushTextMutationEvent(WorkspaceView.WsTextMutation.ClipboardCut)
            android.R.id.copy -> pushTextMutationEvent(WorkspaceView.WsTextMutation.ClipboardCopy)
            android.R.id.paste -> {
                getClipboardManager().primaryClip?.getItemAt(0)?.text.let { clipboardText -> pushTextMutationEvent(WorkspaceView.WsTextMutation.ClipboardPaste(clipboardText.toString()))
                }
            }
            android.R.id.copyUrl,
            android.R.id.switchInputMethod,
            android.R.id.startSelectingText,
            android.R.id.stopSelectingText -> {}
            else -> return false
        }

        workspaceView.drawImmediately()

        return true
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
        logger.i("REQ CURSOR UPDATES")

        val isImmediate = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_IMMEDIATE) != 0
        val isMonitor = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_MONITOR) != 0

        if (isImmediate) {
            notifySelectionUpdated(true)
        }

        if (isMonitor) {
            val newMonitorStatus = CursorMonitorStatus(true)

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                val editorBounds = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_EDITOR_BOUNDS) != 0
                val characterBounds = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_CHARACTER_BOUNDS) != 0
                val insertionMarker = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_INSERTION_MARKER) != 0

                if (editorBounds || characterBounds || insertionMarker) {
                    return false
                }

                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                    val lineBounds = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_VISIBLE_LINE_BOUNDS) != 0
                    val textAppearance = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_TEXT_APPEARANCE) != 0

                    if (lineBounds || textAppearance) {
                        return false
                    }
                }
            }

            cursorMonitorStatus = newMonitorStatus
        }

        return true
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int, cursorUpdateFilter: Int): Boolean {
        return requestCursorUpdates(cursorUpdateMode or cursorUpdateFilter)
    }

    @Synchronized
    override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText {
        val et = ExtractedText()
        val text: CharSequence = wsEditable
        et.text = text
        et.selectionStart = wsEditable.selectionStart.get()
        et.selectionEnd = wsEditable.selectionEnd.get()
        et.startOffset = 0
        et.partialStartOffset = -1
        et.partialEndOffset = -1
        return et
    }

    @Synchronized
    override fun beginBatchEdit(): Boolean {
        logger.i("START BATCH" + batchEditCount.get())

        batchEditCount.getAndUpdate { it + 1 }
        return true
    }

    @Synchronized
    override fun endBatchEdit(): Boolean {
        logger.d("END BATCH" + batchEditCount.get())

        batchEditCount.getAndUpdate { (it - 1.coerceAtLeast(0)) }
        notifySelectionUpdated()

        val isBatchEditing = batchEditCount.get() > 0
        if (!isBatchEditing) {
            wsEditable.flushQueue()
        }
        return isBatchEditing
    }

    override fun getEditable(): Editable {
        return wsEditable
    }

    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
        if (wsEditable.selectionEnd.get() != wsEditable.selectionStart.get() && wsEditable.selectionEnd.get() - wsEditable.selectionStart.get() == beforeLength - afterLength){
            logger.d("DEL SURROUNDING a: ${beforeLength} ${afterLength}")
            wsEditable.replace(wsEditable.selectionStart.get(), wsEditable.selectionEnd.get(), "")
            return true
        }
        logger.d("DEL SURROUNDING b: ${beforeLength} ${afterLength}")
        return super.deleteSurroundingText(beforeLength, afterLength)
    }

    override fun commitText(
        text: CharSequence,
        newCursorPosition: Int,
        textAttribute: TextAttribute?
    ): Boolean {
        logger.i("COMMIT TEXT ${text} ${newCursorPosition} ${textAttribute}")
        return super.commitText(text, newCursorPosition, textAttribute)
    }

    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
        logger.i("COMMIT TEXT ${text} ${newCursorPosition}")
        return super.commitText(text, newCursorPosition)
    }
    /**
     * REPL 1235 1235 d
     * --
     * REPLACE 1237 1237 o
     * DELETE 1235 1237
     * REPLACE 1237 1237 so
     */
}
