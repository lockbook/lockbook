package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.net.Uri
import android.os.Build
import android.text.Editable
import android.text.InputFilter
import android.text.Selection
import android.util.AttributeSet
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.ViewConfiguration
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import androidx.core.content.ContextCompat.startActivity
import app.lockbook.App
import app.lockbook.egui_editor.EGUIEditor
import app.lockbook.egui_editor.IntegrationOutput
import app.lockbook.model.CoreModel
import app.lockbook.model.TextEditorViewModel
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import timber.log.Timber
import kotlin.math.abs

class MarkdownEditor : SurfaceView, SurfaceHolder.Callback2 {
    private var wgpuObj = Long.MAX_VALUE

    private var eguiEditor = EGUIEditor()
    private var inputManager: BaseEGUIInputConnect? = null

    private var touchStartX = 0f
    private var touchStartY = 0f

    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    private var textSaver: TextEditorViewModel? = null

    var redrawTask: Runnable = Runnable {
        invalidate()
    }

    constructor(context: Context, textSaver: TextEditorViewModel) : super(context) {
        this.textSaver = textSaver
    }
    constructor(context: Context, attrs: AttributeSet) : super(context, attrs) {
    }
    constructor(context: Context, attrs: AttributeSet, defStyle: Int) : super(
        context,
        attrs,
        defStyle
    ) {
    }

    init {
        holder.addCallback(this)
        this.setZOrderOnTop(true)
        holder.setFormat(PixelFormat.TRANSPARENT)
    }

    private fun adjustTouchPoint(axis: Float): Float {
        return axis / context.resources.displayMetrics.scaledDensity
    }

    fun getText(): String? {
        if (wgpuObj == Long.MAX_VALUE) {
            return null
        }

        return eguiEditor.getAllText(wgpuObj)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (wgpuObj == Long.MAX_VALUE) {
            return true
        }

        if (event != null) {
            requestFocus()

            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    touchStartX = event.x
                    touchStartY = event.y

                    eguiEditor.touchesBegin(
                        wgpuObj,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y),
                        event.pressure
                    )
                }

                MotionEvent.ACTION_MOVE -> {
                    eguiEditor.touchesMoved(
                        wgpuObj,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y),
                        event.pressure
                    )
                }

                MotionEvent.ACTION_UP -> {
                    val duration = event.eventTime - event.downTime
                    if (duration < 300 && abs(event.x - touchStartX).toInt() < ViewConfiguration.get(
                            context
                        ).scaledTouchSlop && abs(event.y - touchStartY).toInt() < ViewConfiguration.get(
                                context
                            ).scaledTouchSlop
                    ) {
                        (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                            .showSoftInput(this, 0)
                    }

                    eguiEditor.touchesEnded(
                        wgpuObj,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y),
                        event.pressure
                    )
                }
            }

            invalidate()
        }

        return true
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        eguiEditor.resizeEditor(wgpuObj, holder.surface, context.resources.displayMetrics.scaledDensity)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        holder.let { h ->
            wgpuObj = eguiEditor.createWgpuCanvas(h.surface, CoreModel.getPtr(), textSaver!!.currentContent, context.resources.displayMetrics.scaledDensity, (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES)
            inputManager = BaseEGUIInputConnect(this, eguiEditor, wgpuObj)

            if (textSaver!!.savedCursorEnd != -1 && textSaver!!.savedCursorStart != -1) {
                Timber.e("setting the saved cursor: ${textSaver!!.savedCursorEnd} ${textSaver!!.savedCursorStart}")
                eguiEditor.setSelection(wgpuObj, textSaver!!.savedCursorStart, textSaver!!.savedCursorEnd)
            }

            (App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).restartInput(this)
            setWillNotDraw(false)
        }

        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        if (wgpuObj != Long.MAX_VALUE) {
            inputManager?.eguiEditorEditable?.getSelection()?.let { selection ->
                Timber.e("saving cursors: ${selection.first} ${selection.second}")
                textSaver!!.savedCursorStart = selection.first
                textSaver!!.savedCursorEnd = selection.second
            }

            eguiEditor.dropWgpuCanvas(wgpuObj)
            wgpuObj = Long.MAX_VALUE
        }
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        invalidate()
    }

    override fun draw(canvas: Canvas?) {
        super.draw(canvas)

        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        val responseJson = eguiEditor.enterFrame(wgpuObj)
        val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)

        textSaver!!._editorUpdate.postValue(response.editorResponse)

        if (response.editorResponse.selectionUpdated && inputManager!!.monitorCursorUpdates) {
            inputManager!!.notifySelectionUpdated()
        }

        if (response.editorResponse.textUpdated) {
            textSaver!!.waitAndSaveContents(eguiEditor.getAllText(wgpuObj))
        }

        if (eguiEditor.hasCopiedText(wgpuObj)) {
            inputManager?.getClipboardManager()?.setPrimaryClip(ClipData.newPlainText("lb copied text", eguiEditor.getCopiedText(wgpuObj)))
        }

        response.editorResponse.openedURL?.let { openedURL ->
            val browserIntent = Intent(Intent.ACTION_VIEW, Uri.parse(openedURL))
            startActivity(context, browserIntent, null)
        }

        handler.removeCallbacks(redrawTask)

        val redrawIn = response.redrawIn.toLong()
        if (redrawIn != -1L) {
            if (redrawIn < 100) {
                invalidate()
            } else {
                handler.postDelayed(redrawTask, response.redrawIn.toLong())
            }
        }
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection? {
        return inputManager
    }

    override fun onCheckIsTextEditor(): Boolean {
        return true
    }

    fun insertStyling(styling: InsertMarkdownAction) {
        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        when (styling) {
            is InsertMarkdownAction.Heading -> {
                eguiEditor.applyStyleToSelectionHeading(wgpuObj, styling.headingSize)
            }
            InsertMarkdownAction.Bold -> eguiEditor.applyStyleToSelectionBold(wgpuObj)
            InsertMarkdownAction.BulletList -> eguiEditor.applyStyleToSelectionBulletedList(wgpuObj)
            InsertMarkdownAction.InlineCode -> eguiEditor.applyStyleToSelectionInlineCode(wgpuObj)
            InsertMarkdownAction.Italic -> eguiEditor.applyStyleToSelectionItalic(wgpuObj)
            InsertMarkdownAction.NumberList -> eguiEditor.applyStyleToSelectionNumberedList(wgpuObj)
            InsertMarkdownAction.Strikethrough -> eguiEditor.applyStyleToSelectionStrikethrough(wgpuObj)
            InsertMarkdownAction.TodoList -> eguiEditor.applyStyleToSelectionTodoList(wgpuObj)
        }

        invalidate()
    }

    fun indentAtCursor(deindent: Boolean) {
        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        eguiEditor.indentAtCursor(wgpuObj, deindent)
    }

    fun clipboardCut() {
        inputManager?.performContextMenuAction(android.R.id.cut)
    }

    fun clipboardCopy() {
        inputManager?.performContextMenuAction(android.R.id.copy)
    }

    fun clipboardPaste() {
        inputManager?.performContextMenuAction(android.R.id.paste)
    }

    fun undoRedo(redo: Boolean) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            if (redo) {
                inputManager?.performContextMenuAction(android.R.id.redo)
            } else {
                inputManager?.performContextMenuAction(android.R.id.undo)
            }
        }
    }
}

sealed class InsertMarkdownAction {
    data class Heading(val headingSize: Int) : InsertMarkdownAction()

    object BulletList : InsertMarkdownAction()
    object NumberList : InsertMarkdownAction()
    object TodoList : InsertMarkdownAction()

    object Bold : InsertMarkdownAction()
    object Italic : InsertMarkdownAction()
    object InlineCode : InsertMarkdownAction()
    object Strikethrough : InsertMarkdownAction()
}

class BaseEGUIInputConnect(val view: View, val eguiEditor: EGUIEditor, val wgpuObj: Long) : BaseInputConnection(view, true) {
    val eguiEditorEditable = EGUIEditorEditable(view, eguiEditor, wgpuObj)
    var monitorCursorUpdates = false

    fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun notifySelectionUpdated() {
        getInputMethodManager()
            .updateCursorAnchorInfo(
                view,
                CursorAnchorInfo.Builder()
                    .setSelectionRange(eguiEditorEditable.getSelection().first, eguiEditorEditable.getSelection().second)
                    .build()
            )
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)

        event?.let { realEvent ->
            val content = realEvent.unicodeChar.toChar().toString()

            eguiEditor.sendKeyEvent(wgpuObj, realEvent.keyCode, content, realEvent.action == KeyEvent.ACTION_DOWN, realEvent.isAltPressed, realEvent.isCtrlPressed, realEvent.isShiftPressed)
        }

        view.invalidate()

        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {
        when (id) {
            android.R.id.undo -> eguiEditor.undoRedo(wgpuObj, false)
            android.R.id.redo -> eguiEditor.undoRedo(wgpuObj, true)
            android.R.id.selectAll -> eguiEditor.selectAll(wgpuObj)
            android.R.id.cut -> eguiEditor.clipboardCut(wgpuObj)
            android.R.id.copy -> eguiEditor.clipboardCopy(wgpuObj)
            android.R.id.paste -> {
                getClipboardManager().primaryClip?.getItemAt(0)?.text.let { clipboardText ->
                    eguiEditor.clipboardChanged(wgpuObj, clipboardText.toString())
                }

                eguiEditor.clipboardPaste(wgpuObj)
            }
            android.R.id.copyUrl,
            android.R.id.switchInputMethod,
            android.R.id.startSelectingText,
            android.R.id.stopSelectingText -> {}
            else -> return false
        }

        view.invalidate()

        return true
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
        val immediateFlag = cursorUpdateMode and InputConnection.CURSOR_UPDATE_IMMEDIATE == InputConnection.CURSOR_UPDATE_IMMEDIATE
        val monitorFlag = cursorUpdateMode and InputConnection.CURSOR_UPDATE_MONITOR == InputConnection.CURSOR_UPDATE_MONITOR

        if (immediateFlag) {
            notifySelectionUpdated()
        }

        if (monitorFlag) {
            monitorCursorUpdates = true
        }

        return true
    }

    override fun getEditable(): Editable {
        return eguiEditorEditable
    }
}

class EGUIEditorEditable(val view: View, val eguiEditor: EGUIEditor, val wgpuObj: Long) : Editable {

    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0

    fun getSelection(): Pair<Int, Int> {
        val selStr = eguiEditor.getSelection(wgpuObj)
        Timber.e("sel str: $selStr")
        val selections = selStr.split(" ").map { it.toIntOrNull() ?: 0 }

        return Pair(selections.getOrNull(0) ?: 0, selections.getOrNull(1) ?: 0)
    }

    override fun get(index: Int): Char =
        eguiEditor.getTextInRange(wgpuObj, index, index)[0]

    override fun subSequence(startIndex: Int, endIndex: Int): CharSequence =
        eguiEditor.getTextInRange(wgpuObj, startIndex, endIndex)

    override fun getChars(start: Int, end: Int, dest: CharArray?, destoff: Int) {
        dest?.let { realDest ->
            val text = eguiEditor.getTextInRange(wgpuObj, start, end)

            var index = destoff
            for (char in text) {
                if (index < realDest.size) {
                    dest[index] = char

                    index++
                } else {
                    break
                }
            }
        }
    }
    override fun <T> getSpans(start: Int, end: Int, type: Class<T>?): Array<T> {
        return arrayOf<Any>() as Array<T>
    }

    override fun getSpanStart(tag: Any?): Int {
        if (tag == Selection.SELECTION_START) {
            return getSelection().first
        }

        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {
        if (tag == Selection.SELECTION_END) {
            return getSelection().second
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {
        return when (tag) {
            Selection.SELECTION_START -> selectionStartSpanFlag
            Selection.SELECTION_END -> selectionEndSpanFlag
            else -> {

                0
            }
        }
    }

    override fun nextSpanTransition(start: Int, limit: Int, type: Class<*>?): Int {
        return -1
    }

    override fun setSpan(what: Any?, start: Int, end: Int, flags: Int) {
        if (what == Selection.SELECTION_START) {
            selectionStartSpanFlag = flags
            eguiEditor.setSelection(wgpuObj, start, getSelection().second)
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            eguiEditor.setSelection(wgpuObj, getSelection().first, end)
        }
    }

    override fun removeSpan(what: Any?) {}

    override fun append(text: CharSequence?): Editable {
        text?.let { realText ->
            eguiEditor.append(wgpuObj, realText.toString())
        }

        return this
    }

    override fun append(text: CharSequence?, start: Int, end: Int): Editable {
        text?.let { realText ->
            eguiEditor.append(wgpuObj, realText.substring(start, end))
        }

        return this
    }

    override fun append(text: Char): Editable {
        eguiEditor.append(wgpuObj, text.toString())

        return this
    }

    override fun replace(st: Int, en: Int, source: CharSequence?, start: Int, end: Int): Editable {
        source?.let { realText ->
            eguiEditor.replace(wgpuObj, st, en, realText.substring(start, end))
        }

        return this
    }

    override fun replace(st: Int, en: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            eguiEditor.replace(wgpuObj, st, en, realText.toString())
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?, start: Int, end: Int): Editable {
        text?.let { realText ->
            eguiEditor.insert(wgpuObj, where, realText.substring(start, end))
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            eguiEditor.insert(wgpuObj, where, realText.toString())
        }

        return this
    }

    override fun delete(st: Int, en: Int): Editable {
        eguiEditor.replace(wgpuObj, st, en, "")

        return this
    }

    override fun clear() {
        eguiEditor.clear(wgpuObj)
    }

    override fun clearSpans() {}
    override fun setFilters(filters: Array<out InputFilter>?) {}

    // no text needs to be filtered
    override fun getFilters(): Array<InputFilter> = arrayOf()
    override val length: Int = eguiEditor.getTextLength(wgpuObj)
}
