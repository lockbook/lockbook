package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.os.Bundle
import android.os.Handler
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.Selection
import android.text.Spannable
import android.text.TextUtils
import android.text.method.TextKeyListener
import android.text.style.SuggestionSpan
import android.util.AttributeSet
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.ViewConfiguration
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CompletionInfo
import android.view.inputmethod.CorrectionInfo
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputContentInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
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
    private var wgpuObj: Long? = null
    var content: String = ""

    private var eguiEditor = EGUIEditor()
    private var inputManager: BaseEGUIInputConnect? = null

    private var touchStartX = 0f
    private var touchStartY = 0f

    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    private var textSaver: TextEditorViewModel? = null

    constructor(context: Context) : super(context) {
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

    fun adjustTouchPoint(axis: Float): Float {
        return axis / context.resources.displayMetrics.scaledDensity
    }

    fun setText(text: String, textSaver: TextEditorViewModel) {
        this.textSaver = textSaver
        content = text
        eguiEditor.addText(wgpuObj ?: return, text)
        (App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).restartInput(this)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        wgpuObj?.let { wgpuObj ->
            if(event != null) {
                requestFocus()

                when(event.action) {
                    MotionEvent.ACTION_DOWN -> {
                        touchStartX = event.x
                        touchStartY = event.y

                        eguiEditor.touchesBegin(wgpuObj, event.getPointerId(0), adjustTouchPoint(event.x), adjustTouchPoint(event.y), event.pressure)
                    }
                    MotionEvent.ACTION_MOVE -> {
                        eguiEditor.touchesMoved(wgpuObj, event.getPointerId(0), adjustTouchPoint(event.x), adjustTouchPoint(event.y), event.pressure)
                    }
                    MotionEvent.ACTION_UP -> {
                        val duration = event.eventTime - event.downTime
                        if(duration < 300 && abs(event.x - touchStartX).toInt() < ViewConfiguration.get(context).scaledTouchSlop && abs(event.y - touchStartY).toInt() < ViewConfiguration.get(context).scaledTouchSlop) {
                            (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                                .showSoftInput(this, 0)
                        }

                        eguiEditor.touchesEnded(wgpuObj, event.getPointerId(0), adjustTouchPoint(event.x), adjustTouchPoint(event.y), event.pressure)
                    }
                }
            }
        }



        return true
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        Timber.e("Surface changed...")

        eguiEditor.resizeEditor(wgpuObj ?: return, holder.surface, context.resources.displayMetrics.scaledDensity)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        Timber.e("Surface created...")
        holder.let { h ->
            wgpuObj = eguiEditor.createWgpuCanvas(h.surface, CoreModel.getPtr(), content, context.resources.displayMetrics.scaledDensity, (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES)
            inputManager = BaseEGUIInputConnect(this, eguiEditor, wgpuObj!!)
            (App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).restartInput(this)
            setWillNotDraw(false)
        }

        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        if (wgpuObj != Long.MAX_VALUE) {
            eguiEditor.dropWgpuCanvas(wgpuObj ?: return)
            wgpuObj = Long.MAX_VALUE
        }
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        Timber.e("Surface redraw needed...")
    }

    override fun draw(canvas: Canvas?) {
        super.draw(canvas)

        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

//        if(inputManager?.stopEditsAndDisplay == false) {
            val responseJson = eguiEditor.enterFrame(wgpuObj ?: return)
//            Timber.e("the out: $responseJson")
            val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)
//            Timber.e("SUCCESS: ${response.redrawIn}")
        Timber.e("refresh! ${response.editorResponse.selectionUpdated} && ${inputManager?.monitorCursorUpdates == true}")
            if(response.editorResponse.selectionUpdated && true) {
                inputManager!!.notifySelectionUpdated()
            }

            if(response.editorResponse.textUpdated) {
                val allText = eguiEditor.getAllText(wgpuObj!!)
                textSaver!!.waitAndSaveContents(allText)
            }
//        }

        invalidate()
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection? {
        return inputManager
    }

    override fun onCheckIsTextEditor(): Boolean {
        return true
    }

}

class BaseEGUIInputConnect(val view: View, val eguiEditor: EGUIEditor, val wgpuObj: Long): BaseInputConnection(view, true) {
    private var keyListener = TextKeyListener.getInstance(true, TextKeyListener.Capitalize.SENTENCES)
    private val eguiEditorEditable = editable as EGUIEditorEditable
    var monitorCursorUpdates = false

    fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager

    fun notifySelectionUpdated() {
        getInputMethodManager()
            .updateCursorAnchorInfo(view,
                CursorAnchorInfo.Builder()
                    .setMatrix(null)
                    .setSelectionRange(eguiEditorEditable.getSelection().first, eguiEditorEditable.getSelection().second)
                    .setInsertionMarkerLocation(50f, 50f, 60f, 60f, CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION)
                    .build())
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)

        event?.let { realEvent ->
            val content = realEvent.unicodeChar.toChar().toString()

            getInputMethodManager().displayCompletions(view, arrayOf<CompletionInfo>(CompletionInfo(1, 1, "cookies")))

            eguiEditor.sendKeyEvent(wgpuObj, realEvent.keyCode, content, realEvent.action == KeyEvent.ACTION_DOWN, realEvent.isAltPressed, realEvent.isCtrlPressed, realEvent.isShiftPressed)
//            (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).displayCompletions(view, listOf<CompletionInfo>(CompletionInfo()))
        }

        return true
    }

    override fun commitCompletion(text: CompletionInfo?): Boolean {
        Timber.e("committing COMPLETION: ${text?.text}")

        return super.commitCompletion(text)
    }

    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
//        (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).
        return super.commitText(text, newCursorPosition)
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
        val immediateFlag = cursorUpdateMode and InputConnection.CURSOR_UPDATE_IMMEDIATE == InputConnection.CURSOR_UPDATE_IMMEDIATE
        val monitorFlag = cursorUpdateMode and InputConnection.CURSOR_UPDATE_MONITOR == InputConnection.CURSOR_UPDATE_MONITOR

        if(immediateFlag) {
            notifySelectionUpdated()
        }

        if(monitorFlag){
            monitorCursorUpdates = true
        }

        return true
    }

    override fun getEditable(): Editable {
        return EGUIEditorEditable(view, eguiEditor, wgpuObj)
    }
}

class EGUIEditorEditable(val view: View, val eguiEditor: EGUIEditor, val wgpuObj: Long): Editable {

    var selectionStartSpanFlag = 0
    var selectionEndSpanFlag = 0

    var suggestionSpans = mutableListOf<SuggestionSpanInfo>()

    data class SuggestionSpanInfo(val span: SuggestionSpan, val start: Int, val end: Int, val flags: Int)

    fun getSelection(): Pair<Int, Int> {
        val selStr = eguiEditor.getSelection(wgpuObj)
        Timber.e("SELSTART: ${selStr}")
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

            Timber.e("get chars out from $start to $end $text ${getSelection()}")

            var index = destoff;
            for(char in text) {
                if(index < realDest.size) {
                    dest[index] = char

                    index++
                } else {
                    break
                }
            }
        }
    }
    override fun <T> getSpans(start: Int, end: Int, type: Class<T>?): Array<T> {
        Timber.e("getting spans: ${type?.canonicalName}")

        if(type == SuggestionSpan::class.java) {
            Timber.e("Get suggestion span: ${suggestionSpans.map { it.span.spanTypeId }}")
            return suggestionSpans.map { it.span }.toTypedArray() as Array<T>
        } else {
            return arrayOf<Any>() as Array<T>
        }
    }

    override fun getSpanStart(tag: Any?): Int {
        Timber.e("getting span: ${tag?.let { it::class.simpleName } ?: "null"}")

        if(tag == Selection.SELECTION_START) {
            return getSelection().first
        }


        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {
        Timber.e("get span end: ${tag?.let { it::class.simpleName } ?: "null"}")

        if(tag == Selection.SELECTION_END) {
            return getSelection().second
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {
        Timber.e("get span flags: ${tag?.let { it::class.simpleName } ?: "null"}")

        return when (tag) {
            Selection.SELECTION_START -> selectionStartSpanFlag
            Selection.SELECTION_END -> selectionEndSpanFlag
            else -> {
                for (suggestionInfo in suggestionSpans) {
                    if (suggestionInfo == tag) {
                        return suggestionInfo.flags
                    }
                }

                0
            }
        }
    }

    override fun nextSpanTransition(start: Int, limit: Int, type: Class<*>?): Int {
        return -1
    }

    override fun setSpan(what: Any?, start: Int, end: Int, flags: Int) {
        Timber.e("set span: ${what?.let { it::class.simpleName } ?: "null"}")

        if(what == Selection.SELECTION_START) {
            selectionStartSpanFlag = flags
            eguiEditor.setSelection(wgpuObj, start, getSelection().second)
        } else if(what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            eguiEditor.setSelection(wgpuObj, getSelection().first, end)
        } else if(what is SuggestionSpan) {
            suggestionSpans.add(SuggestionSpanInfo(what, start, end, flags))
        }

        Timber.e("set suggestion for ${what == Selection.SELECTION_START} ${what == Selection.SELECTION_END}")
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
