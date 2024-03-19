package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Color
import android.os.Bundle
import android.text.Editable
import android.text.InputFilter
import android.text.Selection
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
import android.widget.FrameLayout
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.FragmentWorkspaceBinding
import app.lockbook.model.StateViewModel
import app.lockbook.model.UpdateMainScreenUI
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.WorkspaceView
import timber.log.Timber
import java.lang.Math.abs

class WorkspaceFragment: Fragment() {
    private var _binding: FragmentWorkspaceBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private val model: WorkspaceViewModel by activityViewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentWorkspaceBinding.inflate(inflater, container, false)

        val workspaceWrapper = WorkspaceWrapperView(requireContext(), model)

        val layoutParams = ConstraintLayout.LayoutParams(
            ConstraintLayout.LayoutParams.MATCH_PARENT,
            0
        ).apply {
            startToStart = ConstraintLayout.LayoutParams.PARENT_ID
            endToEnd = ConstraintLayout.LayoutParams.PARENT_ID
            topToBottom = R.id.workspace_toolbar
            bottomToBottom = ConstraintLayout.LayoutParams.PARENT_ID
        }

        binding.workspaceRoot.addView(workspaceWrapper, layoutParams)

        binding.workspaceToolbar.setNavigationOnClickListener {
            activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFile(null))
        }

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
            workspaceWrapper.updateCurrentTab(tab)
        }

        return binding.root
    }
}

@SuppressLint("ViewConstructor")
class WorkspaceWrapperView(context: Context, val model: WorkspaceViewModel): FrameLayout(context) {
    val workspaceView: WorkspaceView
    var currentTab = WorkspaceTab.Welcome

    var currentWrapper: ViewGroup? = null

    companion object {
        val TAB_BAR_HEIGHT = 50
        val TEXT_TOOL_BAR_HEIGHT = 42
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
        topMargin = TAB_BAR_HEIGHT + TEXT_TOOL_BAR_HEIGHT
    }

//    val WS_SVG_LAYOUT_PARAMS = ViewGroup.MarginLayoutParams(
//        ViewGroup.LayoutParams.MATCH_PARENT,
//        ViewGroup.LayoutParams.MATCH_PARENT
//    ).apply {
//        // Set top and bottom margins
//        topMargin = TAB_BAR_HEIGHT
//        bottomMargin = SVG_TOOL_BAR_HEIGHT
//    }

    init {
        workspaceView = WorkspaceView(context)
        workspaceView.model = model

        addView(workspaceView, REG_LAYOUT_PARAMS)
    }

    fun updateCurrentTab(newTab: WorkspaceTab) {
        Timber.e("UPDATING CURRENT TAB from ${currentTab} to ${newTab}")

        if(newTab.viewWrapperId() == currentTab.viewWrapperId()) {
            return
        }

        val parentViewGroup = currentWrapper?.parent as? ViewGroup
        parentViewGroup?.removeView(currentWrapper)

        when(newTab) {
            WorkspaceTab.Welcome,
            WorkspaceTab.Svg,
            WorkspaceTab.Image,
            WorkspaceTab.Pdf,
            WorkspaceTab.Loading -> {
//                addView(workspaceView, REG_LAYOUT_PARAMS)
            }
            WorkspaceTab.Markdown,
            WorkspaceTab.PlainText -> {
                val inputWrapper = WorkspaceTextInputWrapper(context)
//                inputWrapper.addView(workspaceView, WS_TEXT_LAYOUT_PARAMS)

                addView(inputWrapper, WS_TEXT_LAYOUT_PARAMS)
            }
        }
    }

}

class WorkspaceTextInputWrapper(context: Context): FrameLayout(context) {
    private var touchStartX = 0f
    private var touchStartY = 0f

    init {
        setBackgroundColor(Color.RED)
        Timber.e("INITED TEXT INPUT WRAPPER")
    }
    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        Timber.e("I FELT A TOUCH EVENT YEEEEE ${event?.action}")

        requestFocus()

        when(event?.action) {
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
                        .showSoftInput(this, 0)

                }
            }
        }

        return true
    }

    override fun onCheckIsTextEditor(): Boolean {
        return true
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection {
        return WorkspaceTextInputConnection(this)
    }
}

class WorkspaceTextInputConnection(val view: View) : BaseInputConnection(view, true) {
    private val wsEditable = WorkspaceTextEditable(view)
    var monitorCursorUpdates = false

    fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun notifySelectionUpdated() {
        getInputMethodManager()
            .updateCursorAnchorInfo(
                view,
                CursorAnchorInfo.Builder()
                    .setSelectionRange(wsEditable.getSelection().first, wsEditable.getSelection().second)
                    .build()
            )
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)

        event?.let { realEvent ->
            val content = realEvent.unicodeChar.toChar().toString()

            WorkspaceView.WORKSPACE.sendKeyEvent(WorkspaceView.WGPU_OBJ, realEvent.keyCode, content, realEvent.action == KeyEvent.ACTION_DOWN, realEvent.isAltPressed, realEvent.isCtrlPressed, realEvent.isShiftPressed)
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

class WorkspaceTextEditable(val view: View) : Editable {

    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0

    fun getSelection(): Pair<Int, Int> {
        val selStr = WorkspaceView.WORKSPACE.getSelection(WorkspaceView.WGPU_OBJ)
        val selections = selStr.split(" ").map { it.toIntOrNull() ?: 0 }

        return Pair(selections.getOrNull(0) ?: 0, selections.getOrNull(1) ?: 0)
    }

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
            WorkspaceView.WORKSPACE.setSelection(WorkspaceView.WGPU_OBJ, start, getSelection().second)
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            WorkspaceView.WORKSPACE.setSelection(WorkspaceView.WGPU_OBJ, getSelection().first, end)
        }
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
    override val length: Int = WorkspaceView.WORKSPACE.getTextLength(WorkspaceView.WGPU_OBJ)
}

