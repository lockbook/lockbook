package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.os.Bundle
import android.os.Handler
import android.text.InputType
import android.util.AttributeSet
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.ViewConfiguration
import android.view.inputmethod.CompletionInfo
import android.view.inputmethod.CorrectionInfo
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputContentInfo
import android.view.inputmethod.InputMethodManager
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
    private var inputManager: EGUIInputManager? = null

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
            inputManager = EGUIInputManager(this, eguiEditor, wgpuObj!!)
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

        if(inputManager?.stopEditsAndDisplay == false) {
            val responseJson = eguiEditor.enterFrame(wgpuObj ?: return)
//            Timber.e("the out: $responseJson")
            val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)
//            Timber.e("SUCCESS: ${response.redrawIn}")
            if(response.editorResponse.selectionUpdated && inputManager?.monitorCursorUpdates == true) {
                inputManager!!.notifySelectionUpdated()
            }

            if(response.editorResponse.textUpdated) {
                val allText = eguiEditor.getAllText(wgpuObj!!)
                textSaver!!.waitAndSaveContents(allText)
            }
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

class EGUIInputManager(val view: View, val eguiEditor: EGUIEditor, val wgpuObj: Long): InputConnection {
    var stopEditsAndDisplay = false
    var monitorCursorUpdates = false

    fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager

    private fun getSelection(): Pair<Int, Int> {
        val selStr = eguiEditor.getSelection(wgpuObj)
        Timber.e("SELSTART: ${selStr}")
        val selections = selStr.split(" ").map { it.toIntOrNull() ?: 0 }

        return Pair(selections.getOrNull(0) ?: 0, selections.getOrNull(1) ?: 0)
    }

    fun notifySelectionUpdated() {
        getInputMethodManager()
            .updateCursorAnchorInfo(view,
                CursorAnchorInfo.Builder()
                    .setSelectionRange(getSelection().first, getSelection().second)
                    .build())
    }

//    private fun insertionMarkLocations()

    override fun getTextBeforeCursor(n: Int, flags: Int): CharSequence? {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return eguiEditor.getTextBeforeCursor(wgpuObj, n)
    }

    override fun getTextAfterCursor(n: Int, flags: Int): CharSequence? {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")

        return eguiEditor.getTextAfterCursor(wgpuObj, n)
    }

    // not necessarily required
    override fun getSelectedText(flags: Int): CharSequence {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName} ${eguiEditor.getSelectedText(wgpuObj)}")

        return eguiEditor.getSelectedText(wgpuObj)
    }

    override fun getCursorCapsMode(reqModes: Int): Int {
        return InputType.TYPE_CLASS_TEXT
    }

    override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        val extractedText = ExtractedText()
        extractedText.text = eguiEditor.getAllText(wgpuObj)

        return extractedText
    }

    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        eguiEditor.deleteSurroundingText(wgpuObj, beforeLength, afterLength)

        return true
    }

    // not necessarily required
    override fun deleteSurroundingTextInCodePoints(beforeLength: Int, afterLength: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return false
    }

    override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        // todo

        return true
    }

    // not necessarily required
    override fun setComposingRegion(start: Int, end: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return false
    }

    override fun finishComposingText(): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        // todo

        return true
    }

    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        // todo

        return true
    }

    override fun commitCompletion(text: CompletionInfo?): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        // todo

        return true
    }

    // not necessarily required
    override fun commitCorrection(correctionInfo: CorrectionInfo?): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return false
    }

    override fun setSelection(start: Int, end: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        eguiEditor.setSelection(wgpuObj, start, end)

        return true
    }

    // not important for our use case
    override fun performEditorAction(editorAction: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        // todo
        return true
    }

    override fun beginBatchEdit(): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        stopEditsAndDisplay = true
        return true
    }

    override fun endBatchEdit(): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        stopEditsAndDisplay = false
        return false
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        event?.let { realEvent ->
            val content = realEvent.unicodeChar.toChar().toString()

            eguiEditor.sendKeyEvent(wgpuObj, realEvent.keyCode, content, realEvent.action == KeyEvent.ACTION_DOWN, realEvent.isAltPressed, realEvent.isCtrlPressed, realEvent.isShiftPressed)
        }

        return true
    }

    override fun clearMetaKeyStates(states: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return true
    }

    // not important for our use case
    override fun reportFullscreenMode(enabled: Boolean): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return true
    }

    // not important for our use case
    override fun performPrivateCommand(action: String?, data: Bundle?): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return true
    }

    // not necessarily required
    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")

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

    // not necessarily required
    override fun getHandler(): Handler? {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return null
    }

    // not necessarily required
    override fun closeConnection() {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
    }

    // not necessarily required for initial run
    override fun commitContent(
        inputContentInfo: InputContentInfo,
        flags: Int,
        opts: Bundle?
    ): Boolean {
        Timber.e("calling ${Thread.currentThread().stackTrace[2].methodName}")
        return true
    }

}