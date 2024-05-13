package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Matrix
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.Selection
import android.text.style.SuggestionSpan
import android.util.ArraySet
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.DeleteGesture
import android.view.inputmethod.DeleteRangeGesture
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.HandwritingGesture
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.view.inputmethod.InsertGesture
import android.view.inputmethod.InsertModeGesture
import android.view.inputmethod.JoinOrSplitGesture
import android.view.inputmethod.PreviewableHandwritingGesture
import android.view.inputmethod.RemoveSpaceGesture
import android.view.inputmethod.SelectGesture
import android.view.inputmethod.SelectRangeGesture
import android.view.inputmethod.TextAttribute
import android.widget.FrameLayout
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.FragmentWorkspaceBinding
import app.lockbook.model.CoreModel
import app.lockbook.model.FinishedAction
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.WorkspaceView
import app.lockbook.workspace.JRect
import app.lockbook.workspace.JTextRange
import app.lockbook.workspace.Workspace
import com.github.michaelbull.result.unwrap
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import timber.log.Timber
import java.util.concurrent.Executor
import java.util.function.IntConsumer
import kotlin.math.abs


class WorkspaceFragment : Fragment() {
    private var _binding: FragmentWorkspaceBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private val model: WorkspaceViewModel by activityViewModels()

    companion object {
        val TAG = "WorkspaceFragment"
        val BACKSTACK_TAG = "WorkspaceBackstack"
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentWorkspaceBinding.inflate(inflater, container, false)

        val workspaceWrapper = WorkspaceWrapperView(requireContext(), model)

        binding.workspaceToolbar.setOnMenuItemClickListener { it ->
            when (it.itemId) {
                R.id.menu_text_editor_share -> {
                    (requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                        .hideSoftInputFromWindow(workspaceWrapper.windowToken, 0)

                    val file = CoreModel.getFileById(model.selectedFile.value!!).unwrap()

                    activityModel.launchTransientScreen(TransientScreen.ShareFile(file))
                }
                R.id.menu_text_editor_share_externally -> {
                    activityModel.shareSelectedFiles(listOf(CoreModel.getFileById(model.selectedFile.value!!).unwrap()), requireContext().cacheDir)
                }
            }

            true
        }

        val layoutParams = ConstraintLayout.LayoutParams(
            ConstraintLayout.LayoutParams.MATCH_CONSTRAINT,
            ConstraintLayout.LayoutParams.MATCH_CONSTRAINT
        ).apply {
            startToStart = ConstraintLayout.LayoutParams.PARENT_ID
            endToEnd = ConstraintLayout.LayoutParams.PARENT_ID
            topToBottom = R.id.workspace_toolbar
            bottomToBottom = ConstraintLayout.LayoutParams.PARENT_ID
        }

        binding.workspaceRoot.addView(workspaceWrapper, layoutParams)

        model.sync.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.sync()
        }

        model.openFile.observe(viewLifecycleOwner) { (id, newFile) ->
            workspaceWrapper.workspaceView.openDoc(id, newFile)
        }

        model.docCreated.observe(viewLifecycleOwner) { id ->
            workspaceWrapper.workspaceView.openDoc(id, true)
        }

        model.closeDocument.observe(viewLifecycleOwner) { id ->
            workspaceWrapper.workspaceView.closeDoc(id)
        }

        model.currentTab.observe(viewLifecycleOwner) { tab ->
            updateCurrentTab(workspaceWrapper, tab)
        }

        model.showTabs.observe(viewLifecycleOwner) { show ->
            if (!show) {
                binding.workspaceToolbar.setNavigationIcon(R.drawable.ic_baseline_arrow_back_24)

                binding.workspaceToolbar.setNavigationOnClickListener {
                    val currentDoc = model.selectedFile.value

                    if (currentDoc != null) {
                        workspaceWrapper.workspaceView.closeDoc(currentDoc)
                    }
                }
            }

            workspaceWrapper.workspaceView.showTabs(show)
        }

        model.finishedAction.observe(viewLifecycleOwner) { action ->
            when (action) {
                is FinishedAction.Delete -> workspaceWrapper.workspaceView.closeDoc(action.id)
                is FinishedAction.Rename -> workspaceWrapper.workspaceView.fileRenamed(action.id, action.name)
            }
        }

        return binding.root
    }

    private fun updateCurrentTab(workspaceWrapper: WorkspaceWrapperView, newTab: WorkspaceTab) {
        when (newTab) {
            WorkspaceTab.Welcome,
            WorkspaceTab.Loading -> {
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share).isVisible = false
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share_externally).isVisible = false
            }
            WorkspaceTab.Svg,
            WorkspaceTab.Image,
            WorkspaceTab.Pdf,
            WorkspaceTab.Markdown,
            WorkspaceTab.PlainText -> {
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share).isVisible = true
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share_externally).isVisible = true
            }
        }

        workspaceWrapper.updateCurrentTab(newTab)
    }
}

@SuppressLint("ViewConstructor")
class WorkspaceWrapperView(context: Context, val model: WorkspaceViewModel) : FrameLayout(context) {
    val workspaceView: WorkspaceView
    var currentTab = WorkspaceTab.Welcome

    var currentWrapper: View? = null

    companion object {
        const val TAB_BAR_HEIGHT = 50
        const val TEXT_TOOL_BAR_HEIGHT = 45
//        val SVG_TOOL_BAR_HEIGHT = 50
    }

    val REG_LAYOUT_PARAMS = ViewGroup.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.MATCH_PARENT
    )

    val WS_TEXT_LAYOUT_PARAMS = ViewGroup.MarginLayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.MATCH_PARENT
    ).apply {
        topMargin = (TAB_BAR_HEIGHT * context.resources.displayMetrics.scaledDensity).toInt()
        bottomMargin = (TEXT_TOOL_BAR_HEIGHT * context.resources.displayMetrics.scaledDensity).toInt()
    }

    init {
        workspaceView = WorkspaceView(context, model)
        addView(workspaceView, REG_LAYOUT_PARAMS)
    }

    fun updateCurrentTab(newTab: WorkspaceTab) {
        if (newTab.viewWrapperId() == currentTab.viewWrapperId()) {
            return
        }

        when (currentTab) {
            WorkspaceTab.Welcome,
            WorkspaceTab.Svg,
            WorkspaceTab.Image,
            WorkspaceTab.Pdf,
            WorkspaceTab.Loading -> { }
            WorkspaceTab.Markdown,
            WorkspaceTab.PlainText -> {
                (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                    .hideSoftInputFromWindow(this.windowToken, 0)

                currentWrapper?.clearFocus()

                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    (currentWrapper as WorkspaceTextInputWrapper).wsInputConnection.closeConnection()
                }

                val instanceWrapper = currentWrapper
                Handler(Looper.getMainLooper()).postDelayed(
                    {
                        removeView(instanceWrapper)
                    },
                    200
                )
            }
        }

        when (newTab) {
            WorkspaceTab.Welcome,
            WorkspaceTab.Svg,
            WorkspaceTab.Image,
            WorkspaceTab.Pdf,
            WorkspaceTab.Loading -> {}
            WorkspaceTab.Markdown,
            WorkspaceTab.PlainText -> {
                val touchYOffset: Float
                if(model.showTabs.value == true) {
                    touchYOffset = (TAB_BAR_HEIGHT + TEXT_TOOL_BAR_HEIGHT) * context.resources.displayMetrics.scaledDensity
                } else {
                    touchYOffset = TEXT_TOOL_BAR_HEIGHT * context.resources.displayMetrics.scaledDensity
                }

                currentWrapper = WorkspaceTextInputWrapper(context, workspaceView, touchYOffset)
                workspaceView.wrapperView = currentWrapper
                Timber.e("creating workspace 2")

                addView(currentWrapper, WS_TEXT_LAYOUT_PARAMS)
            }
        }

        currentTab = newTab
    }
}

//@SuppressLint("ViewConstructor")
//class WorkspaceTextInputWrapper(context: Context, private val workspaceView: WorkspaceView) : EditText(context) {
//    val wsInputConnection = WorkspaceTextInputConnection(workspaceView)
//
//    init {
//        Timber.e("initing workspace")
//        inputType = InputType.TYPE_TEXT_FLAG_MULTI_LINE
//        isSingleLine = false
//    }
//
//    override fun onDraw(canvas: Canvas) {
////        super.onDraw(canvas)
//
//        workspaceView.invalidate()
//    }
//
//    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
//        return wsInputConnection
//    }
//
//    override fun getEditableText(): Editable {
//        return wsInputConnection.wsEditable
//    }
//
//    @SuppressLint("ClickableViewAccessibility")
//    override fun onTouchEvent(event: MotionEvent?): Boolean {
//        requestFocus()
//
//        super.onTouchEvent(event)
//
//        if (event != null) {
//            workspaceView.forwardedTouchEvent(event, (WorkspaceWrapperView.TAB_BAR_HEIGHT * context.resources.displayMetrics.scaledDensity).toInt())
//        }
//
//        workspaceView.invalidate()
//
//        return true
//    }
//
//    override fun getOffsetForPosition(x: Float, y: Float): Int {
//        val position: JTextPosition = Json.decodeFromString(WorkspaceView.WORKSPACE.textOffsetForPosition(WorkspaceView.WGPU_OBJ, x, y + (WorkspaceWrapperView.TAB_BAR_HEIGHT * context.resources.displayMetrics.scaledDensity).toInt()))
//
//        Timber.e("position ${position.position} from len ${wsInputConnection.wsEditable.length}")
//
//        return position.position
//    }
//
//    override fun getText(): Editable? {
//        if(wsInputConnection == null) {
//            return null
//        }
//
//        return wsInputConnection.wsEditable
//    }
//}

@SuppressLint("ViewConstructor")
class WorkspaceTextInputWrapper(context: Context, val workspaceView: WorkspaceView, val touchYOffset: Float) : View(context) {
    val wsInputConnection = WorkspaceTextInputConnection(workspaceView, this)

    private var touchStartX = 0f
    private var touchStartY = 0f

    init {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    companion object {
        const val BASE_FONT_SIZE = 16
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        requestFocus()

        when (event?.action) {
            MotionEvent.ACTION_DOWN -> {
                touchStartX = event.x
                touchStartY = event.y
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
                        .showSoftInput(this, InputMethodManager.SHOW_IMPLICIT)
                }
            }
        }

        if (event != null) {
            workspaceView.forwardedTouchEvent(event, touchYOffset)
        }

        workspaceView.invalidate()

        return true
    }

    override fun onCheckIsTextEditor(): Boolean {
        return true
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection {
        if(outAttrs != null) {
            outAttrs.initialCapsMode = wsInputConnection.getCursorCapsMode(EditorInfo.TYPE_CLASS_TEXT)
            outAttrs.hintText = "Type here"
            outAttrs.inputType =
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
//            or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT or InputType.TYPE_TEXT_FLAG_AUTO_COMPLETE
//            outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                outAttrs.setInitialSurroundingText(wsInputConnection.wsEditable.toString())
            }

            outAttrs.initialSelStart = wsInputConnection.wsEditable.getSelection().start
            outAttrs.initialSelEnd = wsInputConnection.wsEditable.getSelection().end

            val gestures = ArrayList<Class<out HandwritingGesture?>>()
            if (Build.VERSION.SDK_INT >= 34) {
                gestures.add(SelectGesture::class.java)
                gestures.add(SelectRangeGesture::class.java)
                gestures.add(DeleteGesture::class.java)
                gestures.add(DeleteRangeGesture::class.java)
                gestures.add(InsertGesture::class.java)
                gestures.add(RemoveSpaceGesture::class.java)
                gestures.add(JoinOrSplitGesture::class.java)
                gestures.add(InsertModeGesture::class.java)
                outAttrs.supportedHandwritingGestures = gestures

                val previews: MutableSet<Class<out PreviewableHandwritingGesture?>> = ArraySet()

                previews.add(SelectGesture::class.java)
                previews.add(SelectRangeGesture::class.java)
                previews.add(DeleteGesture::class.java)
                previews.add(DeleteRangeGesture::class.java)
                outAttrs.supportedHandwritingGesturePreviews = previews
            }
        }

        return wsInputConnection
    }
}

class WorkspaceTextInputConnection(val workspaceView: WorkspaceView, val textInputWrapper: WorkspaceTextInputWrapper) : BaseInputConnection(textInputWrapper, true) {
    val wsEditable = WorkspaceTextEditable(workspaceView)
    var monitorCursorUpdates = false

    init {
        Timber.e("created ws input connection")
    }

    private fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    private fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun getCursorRect(): JRect = Json.decodeFromString(WorkspaceView.WORKSPACE.getCursorRect(WorkspaceView.WGPU_OBJ))

    fun notifySelectionUpdated() {
        val selection = wsEditable.getSelection()
        val cursorRect = getCursorRect()
        val scaleDensity = App.applicationContext().resources.displayMetrics.scaledDensity

        val matrix = Matrix()
        matrix.setScale(scaleDensity, scaleDensity)

        Timber.e("notifying selection updated ${selection.start} ${selection.end}")

        getInputMethodManager().updateSelection(textInputWrapper, selection.start, selection.end, wsEditable.composingStart, wsEditable.composingEnd)

//        getInputMethodManager()
//            .updateCursorAnchorInfo(
//                textInputWrapper,
//                CursorAnchorInfo.Builder()
//                    .setMatrix(matrix)
//                    .setSelectionRange(selection.start, selection.end)
//                    .setInsertionMarkerLocation(cursorRect.minX, cursorRect.minY, cursorRect.maxY, cursorRect.maxY, 0)
//                    .build()
//            )
    }

    // I can just override a bunch of these methods and fix the inconsistent behavior

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)

        if (event != null) {
            val content = event.unicodeChar.toChar().toString()

            Timber.e("sending key ${content} ${event.action} ${event.keyCode}")
            WorkspaceView.WORKSPACE.sendKeyEvent(WorkspaceView.WGPU_OBJ, event.keyCode, content, event.action == KeyEvent.ACTION_DOWN, event.isAltPressed, event.isCtrlPressed, event.isShiftPressed)
        }

        workspaceView.invalidate()

        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {
        when (id) {
            android.R.id.selectAll -> WorkspaceView.WORKSPACE.selectAll(WorkspaceView.WGPU_OBJ)
            android.R.id.cut -> WorkspaceView.WORKSPACE.clipboardCut(WorkspaceView.WGPU_OBJ)
            android.R.id.copy -> WorkspaceView.WORKSPACE.clipboardCopy(WorkspaceView.WGPU_OBJ)
            android.R.id.paste -> {
                getClipboardManager().primaryClip?.getItemAt(0)?.text.let { clipboardText ->
                    WorkspaceView.WORKSPACE.clipboardPaste(
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

        workspaceView.invalidate()

        return true
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
        val immediateFlag = cursorUpdateMode and InputConnection.CURSOR_UPDATE_IMMEDIATE == InputConnection.CURSOR_UPDATE_IMMEDIATE
        val monitorFlag = cursorUpdateMode and InputConnection.CURSOR_UPDATE_MONITOR == InputConnection.CURSOR_UPDATE_MONITOR

        Timber.e("requesting cursor updates... ${immediateFlag} ${monitorFlag}")

        if (immediateFlag) {
            notifySelectionUpdated()
        }

        if (monitorFlag) {
            monitorCursorUpdates = true
        }

        return true
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int, cursorUpdateFilter: Int): Boolean {
        return requestCursorUpdates(cursorUpdateMode)
    }

    override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
        Timber.e("the text \"${text}\" and new cursor position ${newCursorPosition} ")
        return super.setComposingText(text, newCursorPosition)
    }

    override fun getEditable(): Editable {
        return wsEditable
    }
}

class WorkspaceTextEditable(val view: WorkspaceView) : Editable {

    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0
    private var composingFlag = 0

    fun getSelection(): JTextRange = Json.decodeFromString(WorkspaceView.WORKSPACE.getSelection(WorkspaceView.WGPU_OBJ))
    fun getComposingText(): JTextRange = Json.decodeFromString(WorkspaceView.WORKSPACE.getComposing(WorkspaceView.WGPU_OBJ))

    override fun get(index: Int): Char =
        WorkspaceView.WORKSPACE.getTextInRange(WorkspaceView.WGPU_OBJ, index, index)[0]

    override fun subSequence(startIndex: Int, endIndex: Int): CharSequence =
        WorkspaceView.WORKSPACE.getTextInRange(WorkspaceView.WGPU_OBJ, startIndex, endIndex)

    override fun getChars(start: Int, end: Int, dest: CharArray?, destoff: Int) {
        dest?.let { realDest ->
            val text = WorkspaceView.WORKSPACE.getTextInRange(WorkspaceView.WGPU_OBJ, start, end)

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
        return java.lang.reflect.Array.newInstance(type, 0) as Array<T>
    }

    override fun getSpanStart(tag: Any?): Int {
        Timber.e("getting span start: ${(tag ?: Unit)::class.qualifiedName} ${(tag ?: Unit)::class.simpleName}")

        if (tag == Selection.SELECTION_START) {
            return getSelection().start
        }

        if (tag == Selection.SELECTION_END) {
            return getSelection().end
        }

        if ((tag ?: Unit)::class.simpleName == "ComposingText") {
            Timber.e("v: ${composingStart} len=${length}")
            return composingStart
        }

        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {
        Timber.e("getting span end: ${(tag ?: Unit)::class.qualifiedName}")

        if (tag == Selection.SELECTION_START) {
            return getSelection().start
        }

        if (tag == Selection.SELECTION_END) {
            return getSelection().end
        }

        if ((tag ?: Unit)::class.simpleName == "ComposingText") {
            Timber.e("v: ${composingEnd} len=${length}")
            return composingEnd
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {
        return when (tag) {
            Selection.SELECTION_START -> {
                selectionStartSpanFlag
            }
            Selection.SELECTION_END -> {
                selectionEndSpanFlag
            }
            else -> {
                if((tag ?: Unit)::class.simpleName == "ComposingText") {
                    return composingFlag
                }

                0
            }
        }
    }

    override fun nextSpanTransition(start: Int, limit: Int, type: Class<*>?): Int {
        return -1
    }

    var composingStart = -1
    var composingEnd = -1

    override fun setSpan(what: Any?, start: Int, end: Int, flags: Int) {
        Timber.e("setting span... ${(what ?: Unit)::class.simpleName} $start $end")
        if (what == Selection.SELECTION_START) {
            selectionStartSpanFlag = flags
            WorkspaceView.WORKSPACE.setSelection(WorkspaceView.WGPU_OBJ, start, end)
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            WorkspaceView.WORKSPACE.setSelection(WorkspaceView.WGPU_OBJ, start, end)
        } else if ((what ?: Unit)::class.simpleName == "ComposingText") {
            composingFlag = flags
            composingStart = start
            composingEnd = end
        }

//        view.invalidate()
    }

    override fun removeSpan(what: Any?) {
        Timber.e("remove span... ${(what ?: Unit)::class.simpleName}")
        if((what ?: Unit)::class.simpleName == "ComposingText") {
            composingStart = -1
            composingEnd = -1
        }
    }

    override fun append(text: CharSequence?): Editable {
        text?.let { realText ->
            WorkspaceView.WORKSPACE.append(WorkspaceView.WGPU_OBJ, realText.toString())
        }

        return this
    }

    override fun append(text: CharSequence?, start: Int, end: Int): Editable {
        text?.let { realText ->
            WorkspaceView.WORKSPACE.append(WorkspaceView.WGPU_OBJ, realText.substring(start, end))
        }

        return this
    }

    override fun append(text: Char): Editable {
        WorkspaceView.WORKSPACE.append(WorkspaceView.WGPU_OBJ, text.toString())

        return this
    }

    override fun replace(st: Int, en: Int, source: CharSequence?, start: Int, end: Int): Editable {
        source?.let { realText ->

            Timber.e("replacing... start=$st end=$en with ${realText.substring(start, end)} of len=${length}")

            WorkspaceView.WORKSPACE.replace(WorkspaceView.WGPU_OBJ, st, en, realText.substring(start, end))
        }

        return this
    }

    override fun replace(st: Int, en: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            Timber.e("replacing... start=$st end=$en with $realText len=${length}")
            Timber.w("stack trace START")
            Thread.currentThread().stackTrace.forEach { Timber.w(it.toString()) }
            Timber.w("stack trace END")

            WorkspaceView.WORKSPACE.replace(WorkspaceView.WGPU_OBJ, st, en, realText.toString())
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?, start: Int, end: Int): Editable {
        text?.let { realText ->
            WorkspaceView.WORKSPACE.insert(WorkspaceView.WGPU_OBJ, where, realText.substring(start, end))
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            WorkspaceView.WORKSPACE.insert(WorkspaceView.WGPU_OBJ, where, realText.toString())
        }

        return this
    }

    override fun delete(st: Int, en: Int): Editable {
        Timber.e("deleting... start=$st end=$en")

        WorkspaceView.WORKSPACE.replace(WorkspaceView.WGPU_OBJ, st, en, "")

        return this
    }

    override fun clear() {
        WorkspaceView.WORKSPACE.clear(WorkspaceView.WGPU_OBJ)
    }

    override fun clearSpans() {}
    override fun setFilters(filters: Array<out InputFilter>?) {}

    // no text needs to be filtered
    override fun getFilters(): Array<InputFilter> = arrayOf()
    override val length: Int get() {
        return WorkspaceView.WORKSPACE.getTextLength(WorkspaceView.WGPU_OBJ)
    }
}
