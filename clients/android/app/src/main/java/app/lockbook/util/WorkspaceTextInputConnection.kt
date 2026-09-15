package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Matrix
import android.graphics.RectF
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.text.Editable
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorBoundsInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.Toast
import app.lockbook.App
import app.lockbook.screen.WorkspaceTextInputWrapper
import app.lockbook.workspace.Workspace
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream

data class CursorMonitorStatus(
    var monitor: Boolean = false,
    var editorBounds: Boolean = false,
    var characterBounds: Boolean = false,
    var insertionMarker: Boolean = false,
    var lineBounds: Boolean = false,
)

const val MAX_CONTENT_SIZE = 25 * 1024 * 1024

private fun KeyEvent.isWorkspaceNavigationKey(): Boolean =
    when (keyCode) {
        KeyEvent.KEYCODE_DPAD_LEFT,
        KeyEvent.KEYCODE_DPAD_RIGHT,
        KeyEvent.KEYCODE_DPAD_UP,
        KeyEvent.KEYCODE_DPAD_DOWN,
        KeyEvent.KEYCODE_MOVE_HOME,
        KeyEvent.KEYCODE_MOVE_END,
        KeyEvent.KEYCODE_PAGE_UP,
        KeyEvent.KEYCODE_PAGE_DOWN,
        -> true

        else -> false
    }

private fun KeyEvent.isSoftKeyboardEvent(): Boolean =
    deviceId == KeyCharacterMap.VIRTUAL_KEYBOARD ||
        (flags and KeyEvent.FLAG_SOFT_KEYBOARD) != 0

@SuppressLint("SoonBlockedPrivateApi")
class WorkspaceTextInputConnection(
    val workspaceView: WorkspaceView,
    val textInputWrapper: WorkspaceTextInputWrapper,
) : BaseInputConnection(textInputWrapper, true) {
    val wsEditable = WorkspaceTextEditable(workspaceView, this)

    var batchEditCount = 0

    private var cursorMonitorStatus = CursorMonitorStatus()

    private fun getInputMethodManager(): InputMethodManager =
        App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager

    private fun getClipboardManager(): ClipboardManager =
        App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun forwardWorkspaceKeyEvent(event: KeyEvent) {
        val content = event.unicodeChar.toChar().toString()
        Workspace.sendKeyEvent(
            WorkspaceView.wgpuObj,
            event.keyCode,
            content,
            event.action == KeyEvent.ACTION_DOWN,
            event.isAltPressed,
            event.isCtrlPressed,
            event.isShiftPressed,
        )
    }

    fun notifySelectionUpdated() {
        val selection = wsEditable.getSelection()
        getInputMethodManager().updateSelection(
            textInputWrapper,
            selection.start,
            selection.end,
            wsEditable.composingStart,
            wsEditable.composingEnd,
        )
        if (cursorMonitorStatus.monitor) {
            updateCursorAnchorInfo()
        }
    }

    fun onEditorGeometryChanged() {
        if (cursorMonitorStatus.monitor) {
            updateCursorAnchorInfo()
        }
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        if (event == null) {
            return super.sendKeyEvent(null)
        }

        forwardWorkspaceKeyEvent(event)
        workspaceView.drawImmediately()
        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {
        when (id) {
            android.R.id.selectAll -> {
                Workspace.selectAll(WorkspaceView.wgpuObj)
            }

            android.R.id.cut -> {
                Workspace.clipboardCut(WorkspaceView.wgpuObj)
            }

            android.R.id.copy -> {
                Workspace.clipboardCopy(WorkspaceView.wgpuObj)
            }

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
                        val bytes =
                            try {
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
                            withContext(Dispatchers.Main) {
                                Workspace.clipboardSendImage(WorkspaceView.wgpuObj, bytes, true)
                                workspaceView.drawImmediately()
                            }
                        }
                    }

                    return true
                }

                val clipboardText = item.text
                if (clipboardText != null) {
                    Workspace.clipboardPaste(
                        WorkspaceView.wgpuObj,
                        clipboardText.toString(),
                    )
                }
            }

            android.R.id.copyUrl,
            android.R.id.switchInputMethod,
            android.R.id.startSelectingText,
            android.R.id.stopSelectingText,
            -> {}

            else -> {
                return false
            }
        }

        workspaceView.drawImmediately()

        return true
    }

    private fun isImageUri(
        uri: Uri?,
        description: android.content.ClipDescription?,
    ): Boolean {
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

    fun readAllBytesCapped(
        uri: Uri,
        maxBytes: Int = MAX_CONTENT_SIZE,
    ): ByteArray? {
        val resolver = App.applicationContext().contentResolver

        // Best-effort size detection: if we know the size, we can allocate once and avoid
        // `ByteArrayOutputStream.toByteArray()`'s extra copy.
        val expectedSize =
            run {
                val fdSize =
                    try {
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

        if (!isImmediate && !isMonitor) {
            cursorMonitorStatus = CursorMonitorStatus()
            return true
        }

        var editorBounds = false
        var characterBounds = false
        var insertionMarker = false
        var lineBounds = false
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            editorBounds = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_EDITOR_BOUNDS) != 0
            characterBounds = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_CHARACTER_BOUNDS) != 0
            insertionMarker = (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_INSERTION_MARKER) != 0
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                lineBounds =
                    (cursorUpdateMode and InputConnection.CURSOR_UPDATE_FILTER_VISIBLE_LINE_BOUNDS) != 0
            }
        }
        val anyFilter = editorBounds || characterBounds || insertionMarker || lineBounds
        // No filter bits: send everything we can (legacy IMEs).
        if (!anyFilter) {
            editorBounds = true
            characterBounds = true
            insertionMarker = true
            lineBounds = true
        }

        if (isMonitor) {
            cursorMonitorStatus =
                CursorMonitorStatus(
                    monitor = true,
                    editorBounds = editorBounds,
                    characterBounds = characterBounds,
                    insertionMarker = insertionMarker,
                    lineBounds = lineBounds,
                )
        }

        if (isImmediate || isMonitor) {
            updateCursorAnchorInfo(
                editorBounds = editorBounds,
                characterBounds = characterBounds,
                insertionMarker = insertionMarker,
                lineBounds = lineBounds,
            )
        }

        return true
    }

    override fun requestCursorUpdates(
        cursorUpdateMode: Int,
        cursorUpdateFilter: Int,
    ): Boolean = requestCursorUpdates(cursorUpdateMode or cursorUpdateFilter)

    @Synchronized
    override fun getExtractedText(
        request: ExtractedTextRequest?,
        flags: Int,
    ): ExtractedText {
        val et = ExtractedText()
        val text: CharSequence = wsEditable
        et.text = text
        et.selectionStart = wsEditable.selectionStart
        et.selectionEnd = wsEditable.selectionEnd
        et.startOffset = 0
        et.partialStartOffset = -1
        et.partialEndOffset = -1
        return et
    }

    override fun beginBatchEdit(): Boolean {
        batchEditCount += 1

        return true
    }

    override fun endBatchEdit(): Boolean {
        batchEditCount = (batchEditCount - 1).coerceAtLeast(0)

        return batchEditCount > 0
    }

    override fun getEditable(): Editable = wsEditable

    private fun updateCursorAnchorInfo() {
        updateCursorAnchorInfo(
            editorBounds = cursorMonitorStatus.editorBounds,
            characterBounds = cursorMonitorStatus.characterBounds,
            insertionMarker = cursorMonitorStatus.insertionMarker,
            lineBounds = cursorMonitorStatus.lineBounds,
        )
    }

    @SuppressLint("NewApi")
    private fun updateCursorAnchorInfo(
        editorBounds: Boolean,
        characterBounds: Boolean,
        insertionMarker: Boolean,
        lineBounds: Boolean,
    ) {
        if (WorkspaceView.wgpuObj == Long.MAX_VALUE) {
            return
        }

        val selection = wsEditable.getSelection()
        val builder = CursorAnchorInfo.Builder()
        builder.setSelectionRange(selection.start, selection.end)

        val matrix = Matrix()
        val loc = IntArray(2)
        textInputWrapper.getLocationOnScreen(loc)
        matrix.postTranslate(loc[0].toFloat(), loc[1].toFloat())
        builder.setMatrix(matrix)

        if (insertionMarker) {
            val caret = Workspace.cursorRectAt(WorkspaceView.wgpuObj, selection.end)
            if (!caret.none) {
                val (left, top) = eguiToWrapperLocal(caret.minX, caret.minY)
                val bottom = eguiToWrapperLocal(caret.maxX, caret.maxY).second
                builder.setInsertionMarkerLocation(
                    left,
                    top,
                    bottom,
                    bottom,
                    cursorAnchorFlags(left, top, left, bottom),
                )
            }
        }

        if (characterBounds) {
            val composingStart = wsEditable.composingStart
            val composingEnd = wsEditable.composingEnd
            val rangeStart: Int
            val rangeEnd: Int
            if (composingStart >= 0 && composingEnd > composingStart) {
                rangeStart = composingStart
                rangeEnd = composingEnd
            } else if (selection.isEmpty()) {
                rangeStart = (selection.end - 1).coerceAtLeast(0)
                rangeEnd = selection.end.coerceAtLeast(rangeStart)
            } else {
                rangeStart = selection.start
                rangeEnd = selection.end
            }
            val cappedEnd = rangeEnd.coerceAtMost(rangeStart + 64)
            if (cappedEnd > rangeStart) {
                val rects = Workspace.characterRects(WorkspaceView.wgpuObj, rangeStart, cappedEnd)
                for (i in rects.indices) {
                    val rect = rects[i]
                    if (rect.none) {
                        continue
                    }
                    val (left, top) = eguiToWrapperLocal(rect.minX, rect.minY)
                    val (right, bottom) = eguiToWrapperLocal(rect.maxX, rect.maxY)
                    builder.addCharacterBounds(
                        rangeStart + i,
                        left,
                        top,
                        right,
                        bottom,
                        cursorAnchorFlags(left, top, right, bottom),
                    )
                }
            }
        }

        if (lineBounds && Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            val caret = Workspace.cursorRectAt(WorkspaceView.wgpuObj, selection.end)
            if (!caret.none) {
                val (left, top) = eguiToWrapperLocal(caret.minX, caret.minY)
                val bottom = eguiToWrapperLocal(caret.maxX, caret.maxY).second
                builder.addVisibleLineBounds(0f, top, textInputWrapper.width.toFloat(), bottom)
            }
        }

        if (editorBounds && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val bounds =
                RectF(
                    0f,
                    0f,
                    textInputWrapper.width.toFloat(),
                    textInputWrapper.height.toFloat(),
                )
            builder.setEditorBoundsInfo(
                EditorBoundsInfo
                    .Builder()
                    .setEditorBounds(bounds)
                    .setHandwritingBounds(bounds)
                    .build(),
            )
        }

        getInputMethodManager().updateCursorAnchorInfo(textInputWrapper, builder.build())
    }

    private fun eguiToWrapperLocal(
        x: Float,
        y: Float,
    ): Pair<Float, Float> {
        val density = textInputWrapper.resources.displayMetrics.scaledDensity
        return x * density - textInputWrapper.left to y * density - textInputWrapper.top
    }

    private fun cursorAnchorFlags(
        left: Float,
        top: Float,
        right: Float,
        bottom: Float,
    ): Int {
        val w = textInputWrapper.width.toFloat()
        val h = textInputWrapper.height.toFloat()
        val visible = right > 0f && left < w && bottom > 0f && top < h
        val invisible = left < 0f || right > w || top < 0f || bottom > h
        var flags = 0
        if (visible) {
            flags = flags or CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION
        }
        if (invisible) {
            flags = flags or CursorAnchorInfo.FLAG_HAS_INVISIBLE_REGION
        }
        return flags
    }
}
