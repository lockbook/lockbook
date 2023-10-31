package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Bundle
import android.text.Editable
import android.text.InputFilter
import android.text.Selection
import android.text.method.TextKeyListener
import android.text.style.SuggestionSpan
import android.util.AttributeSet
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
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputContentInfo
import android.view.inputmethod.InputMethodManager
import android.view.textservice.SentenceSuggestionsInfo
import android.view.textservice.SpellCheckerSession
import android.view.textservice.SuggestionsInfo
import android.view.textservice.TextInfo
import android.view.textservice.TextServicesManager
import app.lockbook.App
import app.lockbook.egui_editor.AndroidRect
import app.lockbook.egui_editor.EGUIEditor
import app.lockbook.egui_editor.IntegrationOutput
import app.lockbook.model.CoreModel
import app.lockbook.model.TextEditorViewModel
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import timber.log.Timber
import java.util.Locale
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
            inputManager = BaseEGUIInputConnect(this, eguiEditor, wgpuObj!!, frameOutputJsonParser)
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

        val responseJson = eguiEditor.enterFrame(wgpuObj ?: return)
        val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)
//        Timber.e("refresh! ${response.editorResponse.selectionUpdated} && ${inputManager?.monitorCursorUpdates == true}")
        if(response.editorResponse.selectionUpdated && inputManager!!.monitorCursorUpdates) {
            inputManager!!.notifySelectionUpdated()
        }

        if(response.editorResponse.textUpdated) {
            val allText = eguiEditor.getAllText(wgpuObj!!)
            textSaver!!.waitAndSaveContents(allText)

            inputManager!!.spellChecker.getSentenceSuggestions(arrayOf(TextInfo(eguiEditor.getAllText(
                wgpuObj!!
            ))), 3)
        }

        if(response.editorResponse.showEditMenu && Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            showContextMenu(response.editorResponse.editMenuX, response.editorResponse.editMenuY)
        }

        invalidate()
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection? {
        return inputManager
    }

    override fun onCheckIsTextEditor(): Boolean {
        return true
    }
}

class BaseEGUIInputConnect(val view: View, val eguiEditor: EGUIEditor, val wgpuObj: Long, val jsonParser: Json): BaseInputConnection(view, true) {
    private var keyListener = TextKeyListener.getInstance(true, TextKeyListener.Capitalize.SENTENCES)
    private val eguiEditorEditable = EGUIEditorEditable(view, eguiEditor, wgpuObj)
    var monitorCursorUpdates = false

    val spellChecker = getTextServicesManager().newSpellCheckerSession(
        null,
        Locale.ENGLISH,
        object : SpellCheckerSession.SpellCheckerSessionListener {
            override fun onGetSuggestions(results: Array<out SuggestionsInfo>?) {
                Timber.e("THE SUGGESTIONS: ${results}")
            }

            override fun onGetSentenceSuggestions(results: Array<out SentenceSuggestionsInfo>?) {
                Timber.e("THE SENTENCE SUGGESTIONS:")

                val completions = mutableListOf<CompletionInfo>()

                results?.forEach {
                    for(i in 0 until it.suggestionsCount) {
                        val sugg = it.getSuggestionsInfoAt(i) ?: continue
                        for(j in 0 until sugg.suggestionsCount) {

                            completions.add(CompletionInfo(((i * it.suggestionsCount) + j).toLong(), (i * it.suggestionsCount) + j, sugg.getSuggestionAt(j), sugg.getSuggestionAt(j)))
                            Timber.e("A suggestion for $j in $i: ${sugg.getSuggestionAt(j)}")
                        }
                    }
                }

                setComposingRegion(0, 10)

                Timber.e("sending in completions: ${completions.take(2).toTypedArray().map { it.text }}")
                getInputMethodManager().displayCompletions(null, completions.take(2).toTypedArray())
            }

        },
        true
    )!!

    fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    fun getTextServicesManager(): TextServicesManager = App.applicationContext().getSystemService(Context.TEXT_SERVICES_MANAGER_SERVICE) as TextServicesManager

    fun notifySelectionUpdated() {
        Timber.e("updating selection: ${view.measuredWidth} ${view.measuredHeight}")
        var cursorAnchor = CursorAnchorInfo.Builder()
        for(index in editable.indices) {
            val rect: AndroidRect = Json.decodeFromString(eguiEditor.getCharacterRect(wgpuObj, index))
            cursorAnchor = cursorAnchor.addCharacterBounds(index, rect.minX, rect.minY, rect.maxX, rect.maxY, CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION)
        }

        getInputMethodManager()
            .updateCursorAnchorInfo(view,
                cursorAnchor
                    .setSelectionRange(eguiEditorEditable.getSelection().first, eguiEditorEditable.getSelection().second)
                    .setMatrix(null)
                    .setInsertionMarkerLocation(100f, 100f, 100f, 100f, CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION)
                    .build())
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)

        event?.let { realEvent ->
            val content = realEvent.unicodeChar.toChar().toString()

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {

            }

            eguiEditor.sendKeyEvent(wgpuObj, realEvent.keyCode, content, realEvent.action == KeyEvent.ACTION_DOWN, realEvent.isAltPressed, realEvent.isCtrlPressed, realEvent.isShiftPressed)
        }

        return true
    }

    override fun performSpellCheck(): Boolean {
        return super.performSpellCheck()
    }

    override fun commitCompletion(text: CompletionInfo?): Boolean {
        Timber.e("committing completion: ${text?.text}")

        return super.commitCompletion(text)
    }

    override fun performContextMenuAction(id: Int): Boolean {
        return when(id) {
            android.R.id.selectAll -> {
                eguiEditor.selectAll(wgpuObj)
                true
            }
            android.R.id.cut -> {
                eguiEditor.cut(wgpuObj)
                true
            }
            android.R.id.copy -> {
                eguiEditor.copy(wgpuObj)
                true
            }
            android.R.id.paste -> {
                eguiEditor.paste(wgpuObj)
                true
            }
            android.R.id.copyUrl,
            android.R.id.switchInputMethod,
            android.R.id.startSelectingText,
            android.R.id.stopSelectingText -> false
            else -> false
        }
    }

    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
        Timber.e("committing text")
//        (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).
        return super.commitText(text, newCursorPosition)
    }

    override fun commitContent(
        inputContentInfo: InputContentInfo,
        flags: Int,
        opts: Bundle?
    ): Boolean {
        Timber.e("committing content")
        return super.commitContent(inputContentInfo, flags, opts)
    }

    override fun commitCorrection(correctionInfo: CorrectionInfo?): Boolean {
        Timber.e("committing correction")
        return super.commitCorrection(correctionInfo)
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

        Timber.e("requesting cursor updates: $immediateFlag $monitorFlag")

        return true
    }

    override fun getEditable(): Editable {
        return eguiEditorEditable
    }
}

class EGUIEditorEditable(val view: View, val eguiEditor: EGUIEditor, val wgpuObj: Long): Editable {

    var selectionStartSpanFlag = 0
    var selectionEndSpanFlag = 0

    var composingFlag = 0

    var composingStart = -1
    var composingEnd = -1

    var suggestionSpans = mutableListOf<SuggestionSpanInfo>()

    data class SuggestionSpanInfo(val span: SuggestionSpan, val start: Int, val end: Int, val flags: Int)

    fun getSelection(): Pair<Int, Int> {
        val selStr = eguiEditor.getSelection(wgpuObj)
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
        } else if((tag?.let { it::class.qualifiedName }
                ?: "null") == "android.view.inputmethod.ComposingText") {
            return composingStart
        }


        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {
        Timber.e("get span end: ${tag?.let { it::class.simpleName } ?: "null"}")

        if(tag == Selection.SELECTION_END) {
            return getSelection().second
        } else if((tag?.let { it::class.qualifiedName }
                ?: "null") == "android.view.inputmethod.ComposingText") {
            return composingEnd
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {
        Timber.e("get span flags: ${tag?.let { it::class.simpleName } ?: "null"}")

        return when (tag) {
            Selection.SELECTION_START -> selectionStartSpanFlag
            Selection.SELECTION_END -> selectionEndSpanFlag
            else -> {
                if((tag?.let { it::class.qualifiedName }
                        ?: "null") == "android.view.inputmethod.ComposingText") {
                    return composingFlag
                }

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
        Timber.e("set span: ${what?.let { it::class.qualifiedName } ?: "null"}")

        if(what == Selection.SELECTION_START) {
            selectionStartSpanFlag = flags
            eguiEditor.setSelection(wgpuObj, start, getSelection().second)
        } else if(what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            eguiEditor.setSelection(wgpuObj, getSelection().first, end)
        } else if((what?.let { it::class.qualifiedName }
                ?: "null") == "android.view.inputmethod.ComposingText") {
            composingFlag = flags
            composingStart = start
            composingEnd = end
        } else if(what is SuggestionSpan) {
            suggestionSpans.add(SuggestionSpanInfo(what, start, end, flags))
        }

        Timber.e("set suggestion for ${what == Selection.SELECTION_START} ${what == Selection.SELECTION_END}")
    }

    override fun removeSpan(what: Any?) {
        Timber.e("remove span: ${what?.let { it::class.qualifiedName } ?: "null"}")
    }

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
