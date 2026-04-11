package app.lockbook.util

import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.text.Editable
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import app.lockbook.App
import app.lockbook.screen.WorkspaceTextInputWrapper
import app.lockbook.workspace.Workspace
import kotlin.concurrent.withLock

data class CursorMonitorStatus(var monitor: Boolean = false, var editorBounds: Boolean = false, var characterBounds: Boolean = false, var insertionMarker: Boolean = false)

class WorkspaceTextInputConnection(val workspaceView: WorkspaceView, val textInputWrapper: WorkspaceTextInputWrapper) : BaseInputConnection(textInputWrapper, true) {
    val wsEditable = WorkspaceTextEditable(workspaceView, this)

    var batchEditCount = 0

    private var cursorMonitorStatus = CursorMonitorStatus()

    private fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    private fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun notifySelectionUpdated(isImmediate: Boolean = false) {
        workspaceView.textMutations.add(WorkspaceView.WsTextMutation.NotifySelectionUpdate)
//        println("textInputConnection: notify selection update with $batchEditCount ${cursorMonitorStatus.monitor} $isImmediate")

//        if ((batchEditCount == 0 && cursorMonitorStatus.monitor) || isImmediate) {
//            val selection = wsEditable.getSelection()
//            println("textInputConnection: process selection update with $selection")
//
//            getInputMethodManager().updateSelection(
//                textInputWrapper,
//                selection.start,
//                selection.end,
//                wsEditable.composingStart,
//                wsEditable.composingEnd
//            )
//        }
    }

    fun applySelectionNotification(isImmediate: Boolean = false) {

        val selection = wsEditable.getSelection()
        println("WorkspaceTextEditable: SEL ${selection.start} ${selection.end}")

        getInputMethodManager().updateSelection(
                textInputWrapper,
                selection.start,
                selection.end,
                wsEditable.composingStart,
                wsEditable.composingEnd
        )
    }


    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)
//        println("textInputConnection: send key event ${event}")

        if (event != null) {
            val content = event.unicodeChar.toChar().toString()
            workspaceView.nativeLock.withLock {
                Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, event.keyCode, content, event.action == KeyEvent.ACTION_DOWN, event.isAltPressed, event.isCtrlPressed, event.isShiftPressed)
            }
        }

        workspaceView.drawImmediately()

        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {
        println("textInputConnection: preformContextMenuAction ${id}")
        when (id) {
            android.R.id.selectAll -> Workspace.selectAll(WorkspaceView.WGPU_OBJ)
            android.R.id.cut -> Workspace.clipboardCut(WorkspaceView.WGPU_OBJ)
            android.R.id.copy -> Workspace.clipboardCopy(WorkspaceView.WGPU_OBJ)
            android.R.id.paste -> {
                getClipboardManager().primaryClip?.getItemAt(0)?.text.let { clipboardText ->
                    Workspace.clipboardPaste(
                        WorkspaceView.WGPU_OBJ,
                        clipboardText.toString()
                    )
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
//        println("textInputConnection: requestCursorUpdates ${cursorUpdateMode}")

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
    override fun beginBatchEdit(): Boolean {
//        println("textInputConnection: begin batch edit")
        batchEditCount += 1

        return true
    }

    @Synchronized
    override fun endBatchEdit(): Boolean {
//        println("textInputConnection: end batch edit")
        batchEditCount = (batchEditCount - 1).coerceAtLeast(0)
        notifySelectionUpdated()

        return batchEditCount > 0
    }

    override fun getEditable(): Editable {
        return wsEditable
    }
}
