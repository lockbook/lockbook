package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.text.Editable
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.Toast
import app.lockbook.App
import app.lockbook.screen.WorkspaceTextInputWrapper
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.util.concurrent.atomic.AtomicReference

data class CursorMonitorStatus(var monitor: Boolean = false, var editorBounds: Boolean = false, var characterBounds: Boolean = false, var insertionMarker: Boolean = false)

const val MAX_CONTENT_SIZE = 25 * 1024 * 1024

@SuppressLint("SoonBlockedPrivateApi")
class WorkspaceTextInputConnection(val workspaceView: WorkspaceView, val textInputWrapper: WorkspaceTextInputWrapper) : BaseInputConnection(textInputWrapper, true) {
    val wsEditable = WorkspaceTextEditable(workspaceView, this)

    var batchEditCount = AtomicReference(0)

    private var cursorMonitorStatus = CursorMonitorStatus()

    private fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    private fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun notifySelectionUpdated(isImmediate: Boolean = false) {
        workspaceView.textMutations.get().add(WorkspaceView.WsTextMutation.NotifySelectionUpdate to -1)
    }

    fun applySelectionNotification(isImmediate: Boolean = false) {

        val selection = wsEditable.getSelection()

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

        if (event != null) {
            val content = event.unicodeChar.toChar().toString()
            workspaceView.textMutations.get().add(
                WorkspaceView.WsTextMutation.SendKeyEvent(
                    event.keyCode,
                    content,
                    event.action == KeyEvent.ACTION_DOWN,
                    event.isAltPressed,
                    event.isCtrlPressed,
                    event.isShiftPressed
                ) to -1
            )
        }

        workspaceView.drawImmediately()

        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {

        when (id) {
            android.R.id.selectAll -> workspaceView.textMutations.get().add(WorkspaceView.WsTextMutation.SelectAll to -1)
            android.R.id.cut -> workspaceView.textMutations.get().add(WorkspaceView.WsTextMutation.ClipboardCut to -1)
            android.R.id.copy -> workspaceView.textMutations.get().add(WorkspaceView.WsTextMutation.ClipboardCopy to -1)
            android.R.id.paste -> {
                val clip = getClipboardManager().primaryClip ?: return false
                if (clip.itemCount < 1) return false

                val item = clip.getItemAt(0)

                // Some sources put clipboard data in an Intent; we don't support that paste path yet.
                if (item.intent != null && item.uri == null && item.text == null) {
                    Toast
                        .makeText(App.applicationContext(), "Clipboard content not supported", Toast.LENGTH_SHORT)
                        .show()
                    return false
                }

                val uri = item.uri

                if (isImageUri(uri, clip.description)) {
                    workspaceView.launchIo {
                        val bytes = try {
                            readAllBytesCapped(uri)
                        } catch (err: Exception) {
                            withContext(Dispatchers.Main) {
                                Toast
                                    .makeText(App.applicationContext(), err.message, Toast.LENGTH_SHORT)
                                    .show()
                            }
                            null
                        }

                        if (bytes != null) {
                            workspaceView.textMutations.get().add(
                                WorkspaceView.WsTextMutation.ClipboardPasteImage(bytes, true) to -1
                            )
                            workspaceView.drawImmediately()
                        }
                    }

                    return true
                }

                val clipboardText = item.text
                if (clipboardText != null) {
                    workspaceView.textMutations.get().add(
                        WorkspaceView.WsTextMutation.ClipboardPaste(clipboardText.toString()) to -1
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

    private fun isImageUri(uri: Uri?, description: android.content.ClipDescription?): Boolean {
        if (uri == null) return false
        val resolver = App.applicationContext().contentResolver
        val mime = resolver.getType(uri)
        if (mime != null) {
            if (mime.startsWith("image")) return true
        }
        if (description == null) return false
        return description.hasMimeType("image/*") ||
            description.hasMimeType("image/png") ||
            description.hasMimeType("image/jpeg") ||
            description.hasMimeType("image/webp") ||
            description.hasMimeType("image/gif")
    }

    fun readAllBytesCapped(uri: Uri, maxBytes: Int = MAX_CONTENT_SIZE): ByteArray? {
        val resolver = App.applicationContext().contentResolver

        // Best-effort size detection: if we know the size, we can allocate once and avoid
        // `ByteArrayOutputStream.toByteArray()`'s extra copy.
        val expectedSize = run {
            val fdSize = try {
                resolver.openAssetFileDescriptor(uri, "r")?.use { afd ->
                    val len = afd.length
                    if (len >= 0) len.toInt() else null
                }
            } catch (_: Exception) {
                null
            }

            fdSize ?: try {
                resolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { cursor ->
                    val idx = cursor.getColumnIndex(OpenableColumns.SIZE)
                    if (idx != -1 && cursor.moveToFirst()) {
                        val size = cursor.getLong(idx)
                        if (size in 0..Int.MAX_VALUE.toLong()) size.toInt() else null
                    } else {
                        null
                    }
                }
            } catch (_: Exception) {
                null
            }
        }

        if (expectedSize != null && expectedSize > maxBytes) throw Exception("Copied image too large")

        resolver.openInputStream(uri)?.use { input ->
            if (expectedSize != null && expectedSize != 0) {
                val bytes = ByteArray(expectedSize)
                var offset = 0
                while (offset < expectedSize) {
                    val read = input.read(bytes, offset, expectedSize - offset)
                    if (read <= 0) break
                    offset += read
                }
                return if (offset == expectedSize) bytes else bytes.copyOf(offset)
            }

            val out = ByteArrayOutputStream(1024 * 1024)
            val buffer = ByteArray(1024 * 1024)
            var total = 0
            while (true) {
                val read = input.read(buffer)
                if (read <= 0) break
                total += read
                if (total > maxBytes) return null
                out.write(buffer, 0, read)
            }
            return out.toByteArray()
        }

        return null
    }

    private fun readAllBytesCapped(uri: Uri?): ByteArray? {
        if (uri == null) return null
        return readAllBytesCapped(uri, MAX_CONTENT_SIZE)
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {

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

        batchEditCount.getAndUpdate { it + 1 }
        return true
    }

    @Synchronized
    override fun endBatchEdit(): Boolean {
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
}
