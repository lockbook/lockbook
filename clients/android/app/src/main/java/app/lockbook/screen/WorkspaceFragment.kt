package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.InputFilter
import android.text.Selection
import android.text.SpannableString
import android.text.TextWatcher
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
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
import app.lockbook.util.InitError
import app.lockbook.util.WorkspaceView
import app.lockbook.workspace.Workspace
import com.github.michaelbull.result.unwrap
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import kotlinx.serialization.modules.SerializersModule
import timber.log.Timber
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
                    (currentWrapper as WorkspaceTextInputWrapper).inputConnection.closeConnection()
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

        workspaceView.postWrapperRender = null

        when (newTab) {
            WorkspaceTab.Welcome,
            WorkspaceTab.Svg,
            WorkspaceTab.Image,
            WorkspaceTab.Pdf,
            WorkspaceTab.Loading -> {}
            WorkspaceTab.Markdown,
            WorkspaceTab.PlainText -> {
                currentWrapper = WorkspaceTextInputWrapper(context, workspaceView)
                workspaceView.wrapperView = currentWrapper

                addView(currentWrapper, WS_TEXT_LAYOUT_PARAMS)
            }
        }

        currentTab = newTab
    }
}

@SuppressLint("ViewConstructor")
class WorkspaceTextInputWrapper(context: Context, val workspaceView: WorkspaceView) : View(context) {

    val inputConnection = WorkspaceTextInputConnection(workspaceView)

    private var touchStartX = 0f
    private var touchStartY = 0f

    init {
        isFocusable = true
        isFocusableInTouchMode = true
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
            workspaceView.forwardedTouchEvent(event, WorkspaceWrapperView.TAB_BAR_HEIGHT + WorkspaceWrapperView.TEXT_TOOL_BAR_HEIGHT)
        }

        workspaceView.invalidate()

        return true
    }

    override fun onCheckIsTextEditor(): Boolean {
        return true
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection {
        return inputConnection
    }
}

class WorkspaceTextInputConnection(val view: WorkspaceView) : BaseInputConnection(view, true) {
    val wsEditable = WorkspaceTextEditable(view)
    private var monitorCursorUpdates = false

    private fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    private fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    private fun notifySelectionUpdated() {
        getInputMethodManager()
            .updateCursorAnchorInfo(
                view,
                CursorAnchorInfo.Builder()
                    .setSelectionRange(wsEditable.getSelection().start, wsEditable.getSelection().end)
                    .build()
            )
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)

        if (event != null) {
            val content = event.unicodeChar.toChar().toString()

            WorkspaceView.WORKSPACE.sendKeyEvent(WorkspaceView.WGPU_OBJ, event.keyCode, content, event.action == KeyEvent.ACTION_DOWN, event.isAltPressed, event.isCtrlPressed, event.isShiftPressed)
        }

        view.invalidate()

        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {
        when (id) {
            android.R.id.selectAll -> WorkspaceView.WORKSPACE.selectAll(WorkspaceView.WGPU_OBJ)
            android.R.id.cut -> WorkspaceView.WORKSPACE.clipboardCut(WorkspaceView.WGPU_OBJ)
            android.R.id.copy -> WorkspaceView.WORKSPACE.clipboardCopy(WorkspaceView.WGPU_OBJ)
            android.R.id.paste -> {
                getClipboardManager().primaryClip?.getItemAt(0)?.text.let { clipboardText ->
                    WorkspaceView.WORKSPACE.clipboardPaste(WorkspaceView.WGPU_OBJ, clipboardText.toString())
                }
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
        return wsEditable
    }
}

@Serializable
data class JTextRange(val none: Boolean, val start: Int, val end: Int)

class WorkspaceTextEditable(val view: WorkspaceView) : Editable {

    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0

    private var currentSelectionStart: Int? = null
    private var currentSelectionEnd: Int? = null

    private val postWrapperRender: () -> Unit = {
        currentSelectionStart = null
        currentSelectionEnd = null
    }

    init {
        view.postWrapperRender = postWrapperRender
    }

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
        if (tag == Selection.SELECTION_START) {
            return getSelection().start
        }

        if (tag == Selection.SELECTION_END) {
            return getSelection().end
        }

        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {
        if (tag == Selection.SELECTION_START) {
            return getSelection().start
        }

        if (tag == Selection.SELECTION_END) {
            return getSelection().end
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {
        return when (tag) {
            Selection.SELECTION_START -> {
//                Timber.e("get selection started flags...")
                selectionStartSpanFlag
            }
            Selection.SELECTION_END -> {
//                Timber.e("get selection ended flags...")
                selectionEndSpanFlag
            }
            else -> {
                0
            }
        }
    }

    override fun nextSpanTransition(start: Int, limit: Int, type: Class<*>?): Int {
//        Timber.e("getting next span transition")
        return -1
    }

    override fun setSpan(what: Any?, start: Int, end: Int, flags: Int) {
        if (what == Selection.SELECTION_START) {
            selectionStartSpanFlag = flags
            WorkspaceView.WORKSPACE.setSelection(WorkspaceView.WGPU_OBJ, start, end)
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            WorkspaceView.WORKSPACE.setSelection(WorkspaceView.WGPU_OBJ, start, end)
        }

        view.invalidate()
    }

    override fun removeSpan(what: Any?) {}

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
            WorkspaceView.WORKSPACE.replace(WorkspaceView.WGPU_OBJ, st, en, realText.substring(start, end))
        }

        return this
    }

    override fun replace(st: Int, en: Int, text: CharSequence?): Editable {
        text?.let { realText ->
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
