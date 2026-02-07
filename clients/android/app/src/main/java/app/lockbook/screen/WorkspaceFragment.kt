package app.lockbook.screen

import android.animation.ValueAnimator
import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.Selection
import android.text.Spannable
import android.text.Spanned
import android.view.GestureDetector
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.view.animation.Interpolator
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.interpolator.view.animation.FastOutLinearInInterpolator
import androidx.interpolator.view.animation.LinearOutSlowInInterpolator
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.FragmentWorkspaceBinding
import app.lockbook.model.FilesListViewModel
import app.lockbook.model.FinishedAction
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.model.UpdateMainScreenUI
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceTabType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.HorizontalTabItemHolder
import app.lockbook.util.VerticalTabItemHolder
import app.lockbook.util.WorkspaceView
import app.lockbook.util.getIconResource
import app.lockbook.workspace.JTextRange
import app.lockbook.workspace.Workspace
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import net.lockbook.File
import kotlin.getValue
import kotlin.math.abs

private const val EXPANDED_BOTTOM_SHEET_HEIGHT = 600

class WorkspaceFragment : Fragment() {
    private var _binding: FragmentWorkspaceBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private val model: WorkspaceViewModel by activityViewModels()

    private var bottomSheetContractedHeight = 0
    companion object {
        val TAG = "WorkspaceFragment"
        val BACKSTACK_TAG = "WorkspaceBackstack"
    }

    private val filesListModel: FilesListViewModel by activityViewModels()

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

                    getCurrentFile()?.let {
                        activityModel.launchTransientScreen(TransientScreen.ShareFile(it))
                    }
                }
                R.id.menu_text_editor_share_externally -> {
                    getCurrentFile()?.let {
                        activityModel.shareSelectedFiles(listOf(it), requireContext().cacheDir)
                    }
                }
            }

            true
        }

        binding.workspaceToolbar.setOnClickListener {
            getCurrentFile()?.let {
                activityModel.launchTransientScreen(TransientScreen.Rename(it))
            }
        }

        val layoutParams = ConstraintLayout.LayoutParams(
            ConstraintLayout.LayoutParams.MATCH_CONSTRAINT,
            ConstraintLayout.LayoutParams.MATCH_CONSTRAINT
        ).apply {
            startToStart = ConstraintLayout.LayoutParams.PARENT_ID
            endToEnd = ConstraintLayout.LayoutParams.PARENT_ID
            topToTop = ConstraintLayout.LayoutParams.PARENT_ID
            bottomToBottom = ConstraintLayout.LayoutParams.PARENT_ID
        }

        binding.workspaceRoot.addView(workspaceWrapper, layoutParams)

        model.sync.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.sync()
        }

        model.openFile.observe(viewLifecycleOwner) { (id, newFile) ->
            workspaceWrapper.workspaceView.openDoc(id, newFile)
            workspaceWrapper.workspaceView.drawImmediately()

            activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenWorkspacePane)
        }

        model.createFile.observe(viewLifecycleOwner) { id ->
            workspaceWrapper.workspaceView.openDoc(id, true)
        }

        model.closeFile.observe(viewLifecycleOwner) { id ->
            workspaceWrapper.workspaceView.closeDoc(id)
            workspaceWrapper.workspaceView.drawImmediately()
        }

        model.currentTab.observe(viewLifecycleOwner) { tab ->
            updateCurrentTab(workspaceWrapper, tab)
            binding.closeAllTabs.isEnabled = !model.tabs.isEmpty()
        }

        model.bottomInset.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.setBottomInset(it)
        }

        model.isRendering.observe(viewLifecycleOwner) { isRendering ->
            if (isRendering) {
                workspaceWrapper.workspaceView.startRendering()
            } else {
                workspaceWrapper.workspaceView.stopRendering()
            }
        }

        binding.workspaceToolbar.setNavigationIcon(R.drawable.ic_baseline_arrow_back_24)

        getCurrentFile()?.let {
            binding.workspaceToolbar.setTitle(it.name)
        }

        binding.workspaceToolbar.setNavigationOnClickListener {
            (context?.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                .hideSoftInputFromWindow(view?.windowToken, 0)
            activityModel.updateMainScreenUI(UpdateMainScreenUI.CloseWorkspacePane)
        }
        workspaceWrapper.workspaceView.showTabs(false)

        model.finishedAction.observe(viewLifecycleOwner) { action ->
            when (action) {
                is FinishedAction.Delete -> workspaceWrapper.workspaceView.closeDoc(action.id)
                is FinishedAction.Rename -> {
                    workspaceWrapper.workspaceView.fileRenamed(action.id, action.name)
                    if (binding.workspaceToolbar.title != "") {
                        // we're showing the title in the menu bar. let's update it
                        binding.workspaceToolbar.setTitle(action.name)
                    }
                    val tabIndex = model.tabs.indexOfFirst { it.id == action.id }
                    if (tabIndex != -1) {
                        val updatedTab = model.tabs.get(tabIndex)
                        updatedTab.name = action.name

                        model.tabs.removeAt(tabIndex)
                        model.tabs.insert(tabIndex, updatedTab)

                        binding.tabsList.adapter?.notifyItemChanged(tabIndex)
                    }
                }
            }
        }

        setupTabList()

        binding.expandList.setOnClickListener {
            model._bottomSheetExpanded.value = !(model.bottomSheetExpanded.value ?: false)
        }

        model.bottomSheetExpanded.observe(viewLifecycleOwner) { shouldExpand ->
            toggleBottomSheetExpansion(shouldExpand)
        }

        binding.closeAllTabs.setOnClickListener {
            workspaceWrapper.workspaceView.closeAllTabs()
            workspaceWrapper.workspaceView.drawImmediately()
        }

        model.hideMaterialToolbar.observe(viewLifecycleOwner) { distanceY ->

            val isKeyboardVisible = model.keyboardVisible.value ?: false

            if (distanceY > 0) {
                hideBottomSheet()
                hideMaterialToolbar()
            } else if (distanceY < 0 && isKeyboardVisible) {
                showMaterialToolbar()
            } else if (distanceY <0 && !isKeyboardVisible) {
                showBottomSheet()
                showMaterialToolbar()
            }
        }

        model.keyboardVisible.observe(viewLifecycleOwner) { keyboardVisible ->
            if (keyboardVisible) {
                binding.standardBottomSheet.visibility = View.GONE
                activityModel.updateMainScreenUI(UpdateMainScreenUI.HideBottomViewNavigation)
            } else {
                binding.standardBottomSheet.visibility = View.VISIBLE
            }
        }

        return binding.root
    }

    private fun toggleBottomSheetExpansion(shouldExpand: Boolean) {
        val currOrientation = (binding.tabsList.layoutManager as LinearLayoutManager).orientation
        if ((currOrientation == LinearLayoutManager.VERTICAL && shouldExpand) ||
            (currOrientation == LinearLayoutManager.HORIZONTAL && !shouldExpand)
        ) {
            return
        }

        val (newOrientation, newHeight) = if (currOrientation == LinearLayoutManager.VERTICAL) {
            LinearLayoutManager.HORIZONTAL to ViewGroup.LayoutParams.WRAP_CONTENT
        } else {
            LinearLayoutManager.VERTICAL to EXPANDED_BOTTOM_SHEET_HEIGHT
        }

        animateBottomSheetHeight(newHeight) {
            binding.expandList.visibility = if (binding.expandList.isVisible) {
                View.GONE
            } else {
                View.VISIBLE
            }

            binding.closeAllTabs.visibility = if (binding.closeAllTabs.isVisible) {
                View.GONE
            } else {
                View.VISIBLE
            }

            binding.tabsList.layoutManager = LinearLayoutManager(
                requireContext(),
                newOrientation,
                false
            )
            setupTabList()
        }
    }

    private fun animateBottomSheetHeight(targetHeight: Int, onMidpoint: () -> Unit) {
        val bottomSheet = binding.standardBottomSheet
        val startHeight = bottomSheet.height
        val isExpanding = targetHeight != ViewGroup.LayoutParams.WRAP_CONTENT
        val interpolator: Interpolator = if (isExpanding) LinearOutSlowInInterpolator() else FastOutLinearInInterpolator()
        val animationDuration = if (isExpanding) { 300 } else { 200 }

        // If target is WRAP_CONTENT, measure it first
        val finalHeight = if (!isExpanding) {
            bottomSheet.measure(
                View.MeasureSpec.makeMeasureSpec(bottomSheet.width, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED)
            )
            bottomSheetContractedHeight
        } else {
            bottomSheetContractedHeight = bottomSheet.measuredHeight
            targetHeight
        }

        val animator = ValueAnimator.ofInt(startHeight, finalHeight)
        animator.addUpdateListener { animation ->
            val progress = animation.animatedFraction
            val value = animation.animatedValue as Int

            // Update height
            bottomSheet.layoutParams.height = value
            bottomSheet.requestLayout()

            // Update opacity based on progress
            // Fade out in first half (0.0 -> 0.5), fade in during second half (0.5 -> 1.0)
            val alpha = if (progress < 0.5f) {
                1f - (progress * 2f) // 1.0 -> 0.0
            } else {
                (progress - 0.5f) * 2f // 0.0 -> 1.0
            }
            binding.tabsList.alpha = alpha
        }

        animator.addUpdateListener(object : ValueAnimator.AnimatorUpdateListener {
            override fun onAnimationUpdate(animation: ValueAnimator) {
                if (animation.animatedFraction >= 0.5f) {
                    onMidpoint()
                    animation.removeUpdateListener(this)
                }
            }
        })

        animator.duration = animationDuration.toLong()
        animator.interpolator = interpolator
        animator.start()
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        ViewCompat.setOnApplyWindowInsetsListener(view) { v, insets ->
            val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())

            if (imeVisible) {
                model._keyboardVisible.postValue(true)
            } else {
                model._keyboardVisible.postValue(false)
            }

            model._bottomInset.value = (-systemBars.bottom + ime.bottom).coerceAtLeast(0)

            val filteredInsets = WindowInsetsCompat.Builder(insets)
                .setInsets(
                    WindowInsetsCompat.Type.ime(),
                    androidx.core.graphics.Insets.NONE // Mask keyboard height to 0
                )
                .build()

            ViewCompat.onApplyWindowInsets(v, filteredInsets)

            insets
        }
    }

    @SuppressLint("NotifyDataSetChanged")
    private fun updateCurrentTab(workspaceWrapper: WorkspaceWrapperView, newTab: WorkspaceTab) {
        val tabFile = filesListModel.fileModel.idsAndFiles[newTab.id]
        if (tabFile == null) {
            model.tabs.set(emptyList())
            binding.tabsList.adapter?.notifyDataSetChanged()
            return
        }

        updateToolbarOnTabChange(newTab.type, tabFile.name)

        val openTabs = workspaceWrapper.workspaceView.getTabs()
            .mapNotNull { tabId -> filesListModel.fileModel.idsAndFiles[tabId] }
            .toList()

        model.tabs.set(openTabs)
        binding.tabsList.adapter?.notifyDataSetChanged()

        workspaceWrapper.updateWrapperBasedOnTab(newTab.type)
    }

    private fun updateToolbarOnTabChange(newTab: WorkspaceTabType, tabTitle: String?) {
        when (newTab) {
            WorkspaceTabType.Welcome,
            WorkspaceTabType.Loading,
            WorkspaceTabType.Graph -> {
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share).isVisible =
                    false
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share_externally).isVisible =
                    false
                binding.workspaceToolbar.setTitle("")
            }

            WorkspaceTabType.Svg,
            WorkspaceTabType.Image,
            WorkspaceTabType.Pdf,
            WorkspaceTabType.Markdown,
            WorkspaceTabType.PlainText -> {
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share).isVisible = true
                binding.workspaceToolbar.menu.findItem(R.id.menu_text_editor_share_externally).isVisible =
                    true
                binding.workspaceToolbar.setTitle(tabTitle)
            }
        }
    }

    fun hideBottomSheet() {
        binding.standardBottomSheet.animate()
            .translationY(binding.standardBottomSheet.measuredHeight.toFloat())
            .setDuration(300)
            .start()
    }

    fun showBottomSheet() {
        binding.standardBottomSheet.animate()
            .translationY(0f)
            .setDuration(300)
            .start()
    }

    fun hideMaterialToolbar() {
        binding.appBarLayout.animate()
            .translationY(-binding.appBarLayout.height.toFloat())
            .setDuration(300)
            .start()
    }

    fun showMaterialToolbar() {
        binding.appBarLayout.animate()
            .translationY(0f)
            .setDuration(300)
            .start()
    }

    private fun getCurrentFile(): File? {
        val currentTab = model.currentTab.value ?: return null
        return filesListModel.fileModel.idsAndFiles[currentTab.id]
    }

    private fun setupTabList() {
        binding.tabsList.setup {
            withDataSource(model.tabs)

            val orientation = (binding.tabsList.layoutManager as LinearLayoutManager).orientation
            if (orientation == LinearLayoutManager.HORIZONTAL) {
                withItem<File, HorizontalTabItemHolder>(R.layout.horizontal_tab_item) {
                    onBind(::HorizontalTabItemHolder) { i, item ->
                        name.text = item.name
                        name.isChecked = getCurrentFile()?.id == item.id
                    }
                    onClick {
                        switchTab(item.id)
                    }
                }
            } else {
                withItem<File, VerticalTabItemHolder>(R.layout.vertical_tab_item) {
                    onBind(::VerticalTabItemHolder) { i, item ->
                        name.text = item.name
                        val isSelected = getCurrentFile()?.id == item.id

                        name.isChecked = isSelected
                        name.setIconResource(item.getIconResource())

                        closeButton.isChecked = isSelected
                        closeButton.setOnClickListener {
                            model._closeFile.value = item.id
                            binding.tabsList.adapter?.notifyItemRemoved(i)
                        }

                        name.setOnClickListener {
                            switchTab(item.id)
                        }
                    }
                }
            }
        }

        binding.tabsList.post {
            val selectedPosition = model.tabs.indexOfFirst { it.id == getCurrentFile()?.id }
            if (selectedPosition != -1) {
                binding.tabsList.scrollToPosition(selectedPosition)
            }
        }
    }

    private fun switchTab(id: String) {
        model._openFile.value = id to false
    }
}

@SuppressLint("ViewConstructor")
class WorkspaceWrapperView(context: Context, val model: WorkspaceViewModel) : FrameLayout(context) {
    val workspaceView: WorkspaceView
    var currentTab = WorkspaceTabType.Welcome

    var currentWrapper: View? = null

    private val scrollListener = object : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(e: MotionEvent): Boolean {
            return true
        }
        override fun onScroll(
            e1: MotionEvent?,
            e2: MotionEvent,
            distanceX: Float,
            distanceY: Float
        ): Boolean {
            model._hideMaterialToolbar.postValue(distanceY)

            return super.onScroll(e1, e2, distanceX, distanceY)
        }
    }

    private val scrollDetector: GestureDetector = GestureDetector(context, scrollListener)

    companion object {
        const val TAB_BAR_HEIGHT = 50
        const val TEXT_TOOL_BAR_HEIGHT = 45
//        val SVG_TOOL_BAR_HEIGHT = 50
    }

    val REG_LAYOUT_PARAMS = ViewGroup.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.MATCH_PARENT
    )

    val WS_TEXT_LAYOUT_PARAMS = MarginLayoutParams(
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

    fun updateWrapperBasedOnTab(newTab: WorkspaceTabType) {
        if (newTab.viewWrapperId() == currentTab.viewWrapperId()) {
            return
        }

        when (currentTab) {
            WorkspaceTabType.Welcome,
            WorkspaceTabType.Svg,
            WorkspaceTabType.Image,
            WorkspaceTabType.Pdf,
            WorkspaceTabType.Loading,
            WorkspaceTabType.Graph -> { }
            WorkspaceTabType.Markdown,
            WorkspaceTabType.PlainText -> {
                (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                    .hideSoftInputFromWindow(this.windowToken, 0)

                currentWrapper?.clearFocus()

                (currentWrapper as WorkspaceTextInputWrapper).wsInputConnection.closeConnection()

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
            WorkspaceTabType.Welcome,
            WorkspaceTabType.Svg,
            WorkspaceTabType.Image,
            WorkspaceTabType.Pdf,
            WorkspaceTabType.Loading,
            WorkspaceTabType.Graph -> {}
            WorkspaceTabType.Markdown,
            WorkspaceTabType.PlainText -> {
                val touchYOffset: Float
                if (model.showTabs.value == true) {
                    touchYOffset = TAB_BAR_HEIGHT * context.resources.displayMetrics.scaledDensity
                } else {
                    touchYOffset = TEXT_TOOL_BAR_HEIGHT * context.resources.displayMetrics.scaledDensity
                }

                currentWrapper = WorkspaceTextInputWrapper(context, workspaceView, touchYOffset)
                workspaceView.wrapperView = currentWrapper

                addView(currentWrapper, WS_TEXT_LAYOUT_PARAMS)
            }
        }

        currentTab = newTab
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onInterceptTouchEvent(event: MotionEvent?): Boolean {

        if (event != null && currentTab != WorkspaceTabType.Svg && currentTab != WorkspaceTabType.Image) {
            scrollDetector.onTouchEvent(event)
        }

        if (model.bottomSheetExpanded.value ?: true) {
            model._bottomSheetExpanded.postValue(false)
        }
        return false
    }
}

@SuppressLint("ViewConstructor")
class WorkspaceTextInputWrapper(context: Context, val workspaceView: WorkspaceView, val touchYOffset: Float) : View(context) {
    val wsInputConnection = WorkspaceTextInputConnection(workspaceView, this)

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
                val keyboardShown = WindowInsetsCompat
                    .toWindowInsetsCompat(rootWindowInsets)
                    .isVisible(WindowInsetsCompat.Type.ime())

                val bottomSheetExpanded = workspaceView.model.bottomSheetExpanded.value ?: false

                if (!bottomSheetExpanded && !keyboardShown && duration < 300 && abs(event.x - touchStartX).toInt() < ViewConfiguration.get(
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

        workspaceView.drawImmediately()

        return true
    }

    override fun onCheckIsTextEditor(): Boolean {
        return true
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection {
        if (outAttrs != null) {
            outAttrs.initialCapsMode = wsInputConnection.getCursorCapsMode(EditorInfo.TYPE_CLASS_TEXT)
            outAttrs.hintText = "Type here"

            outAttrs.inputType =
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES

            outAttrs.initialSelStart = wsInputConnection.wsEditable.getSelection().start
            outAttrs.initialSelEnd = wsInputConnection.wsEditable.getSelection().end
        }

        return wsInputConnection
    }
}

data class CursorMonitorStatus(var monitor: Boolean = false, var editorBounds: Boolean = false, var characterBounds: Boolean = false, var insertionMarker: Boolean = false)

class WorkspaceTextInputConnection(val workspaceView: WorkspaceView, val textInputWrapper: WorkspaceTextInputWrapper) : BaseInputConnection(textInputWrapper, true) {
    val wsEditable = WorkspaceTextEditable(workspaceView, this)
    private var batchEditCount = 0

    private var cursorMonitorStatus = CursorMonitorStatus()

    private fun getInputMethodManager(): InputMethodManager = App.applicationContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    private fun getClipboardManager(): ClipboardManager = App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    fun notifySelectionUpdated(isImmediate: Boolean = false) {
        if ((batchEditCount == 0 && cursorMonitorStatus.monitor) || isImmediate) {
            val selection = wsEditable.getSelection()

            getInputMethodManager().updateSelection(
                textInputWrapper,
                selection.start,
                selection.end,
                wsEditable.composingStart,
                wsEditable.composingEnd
            )
        }
    }

    override fun sendKeyEvent(event: KeyEvent?): Boolean {
        super.sendKeyEvent(event)

        if (event != null) {
            val content = event.unicodeChar.toChar().toString()
            Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, event.keyCode, content, event.action == KeyEvent.ACTION_DOWN, event.isAltPressed, event.isCtrlPressed, event.isShiftPressed)
        }

        workspaceView.drawImmediately()

        return true
    }

    override fun performContextMenuAction(id: Int): Boolean {
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

    override fun beginBatchEdit(): Boolean {
        batchEditCount += 1

        return true
    }

    override fun endBatchEdit(): Boolean {
        batchEditCount = (batchEditCount - 1).coerceAtLeast(0)
        notifySelectionUpdated()

        return batchEditCount > 0
    }

    override fun getEditable(): Editable {
        return wsEditable
    }
}

class WorkspaceTextEditable(val view: WorkspaceView, val wsInputConnection: WorkspaceTextInputConnection) : Editable {

    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0

    var composingStart = -1
    var composingEnd = -1

    private var composingFlag = 0
    private var composingTag: Any? = null

    val selectionStart: Int get() {
        return getSelection().start
    }

    val selectionEnd: Int get() {
        return getSelection().end
    }

    override fun toString(): String {
        return Workspace.getAllText(WorkspaceView.WGPU_OBJ)
    }

    fun getSelection(): JTextRange = Json.decodeFromString(Workspace.getSelection(WorkspaceView.WGPU_OBJ))

    override fun get(index: Int): Char {
        return Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, index, index).getOrNull(0) ?: '0'
    }

    override fun subSequence(startIndex: Int, endIndex: Int): CharSequence {
        return Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, startIndex, endIndex)
    }

    override fun getChars(start: Int, end: Int, dest: CharArray?, destoff: Int) {
        dest?.let { realDest ->
            val text = Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, start, end)

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
        val spans: MutableList<Any> = mutableListOf()
        val spanRange = start..end

        if (type != null) {
            val instanceComposingTag = composingTag

            if (instanceComposingTag != null && type.isAssignableFrom(instanceComposingTag.javaClass) && (spanRange.contains(composingStart) || spanRange.contains(composingEnd))) {
                spans.add(instanceComposingTag)
            }

            if (type.isAssignableFrom(Selection.SELECTION_START.javaClass) && spanRange.contains(getSelection().start)) {
                spans.add(Selection.SELECTION_START)
            }

            if (type.isAssignableFrom(Selection.SELECTION_END.javaClass) && spanRange.contains(getSelection().end)) {
                spans.add(Selection.SELECTION_END)
            }
        }

        @Suppress("UNCHECKED_CAST")
        val returnSpans = java.lang.reflect.Array.newInstance(type, spans.size) as Array<T>

        for (i in spans.indices) {
            returnSpans[i] = spans[i] as T
        }

        return returnSpans
    }

    override fun getSpanStart(tag: Any?): Int {
        if (tag == Selection.SELECTION_START) {
            return selectionStart
        }

        if (tag == Selection.SELECTION_END) {
            return selectionEnd
        }

        if (tag == composingTag || ((tag ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            return composingStart
        }

        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {
        if (tag == Selection.SELECTION_START || tag == Selection.SELECTION_END) {
            TODO("not needed")
        }

        if (tag == composingTag || ((tag ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
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
                if (tag == composingTag) {
                    return composingFlag
                }

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
            Workspace.setSelection(WorkspaceView.WGPU_OBJ, start, end)
            view.drawImmediately()
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            Workspace.setSelection(WorkspaceView.WGPU_OBJ, start, end)
            view.drawImmediately()
        } else if ((flags and Spanned.SPAN_COMPOSING) != 0) {
            composingFlag = flags
            composingTag = what
            composingStart = start
            composingEnd = end
        } else {
            return
        }

        wsInputConnection.notifySelectionUpdated()
    }

    override fun removeSpan(what: Any?) {
        if (what == composingTag || ((what ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }

    override fun append(text: CharSequence?): Editable {
        text?.let { realText ->
            Workspace.append(WorkspaceView.WGPU_OBJ, realText.toString())

            view.drawImmediately()
        }

        return this
    }

    override fun append(text: CharSequence?, start: Int, end: Int): Editable {
        Workspace.append(WorkspaceView.WGPU_OBJ, text?.substring(start, end) ?: "null")
        view.drawImmediately()

        return this
    }

    override fun append(text: Char): Editable {
        Workspace.append(WorkspaceView.WGPU_OBJ, text.toString())
        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()

        return this
    }

    override fun replace(st: Int, en: Int, source: CharSequence?, start: Int, end: Int): Editable {
        source?.let { realText ->
            replace(st, en, realText.subSequence(start, end))
        }

        return this
    }

    private fun getComposingSpansFromSpannable(spannable: Spannable): Pair<Int, Int> {
        for (span in spannable.getSpans(0, spannable.length, Object::class.java)) {
            val flags = spannable.getSpanFlags(span)

            if ((flags and Spanned.SPAN_COMPOSING) != 0) {
                return Pair(spannable.getSpanStart(span), spannable.getSpanEnd(span))
            }
        }

        return Pair(-1, -1)
    }

    override fun replace(st: Int, en: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            if (st == selectionStart && en == selectionEnd) {
                if (realText == "\n") {
                    Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
                } else {
                    Workspace.insertTextAtCursor(WorkspaceView.WGPU_OBJ, realText.toString())
                }
            } else {
                Workspace.replace(WorkspaceView.WGPU_OBJ, st, en, realText.toString())
            }

            if (en < composingStart) {
                val replacedLen = en - st

                composingStart = composingStart - replacedLen + realText.length
                composingEnd = composingEnd - replacedLen + realText.length
            }

            val spannableSource = realText as? Spannable
            if (spannableSource != null) {
                val (sourceComposingStart, sourceComposingEnd) = if (composingTag == null) {
                    getComposingSpansFromSpannable(spannableSource)
                } else {
                    Pair(spannableSource.getSpanStart(composingTag), spannableSource.getSpanEnd(composingTag))
                }

                if (sourceComposingStart != -1) {
                    val newStart = st + sourceComposingStart

                    if (composingStart == -1 || composingStart > newStart) {
                        composingStart = newStart
                    }
                }

                if (sourceComposingEnd != -1) {
                    val newEnd = st + sourceComposingEnd

                    if (composingEnd < newEnd) {
                        composingEnd = newEnd
                    }
                }
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?, start: Int, end: Int): Editable {
        text?.let { realText ->
            val subRealText = realText.substring(start, end)

            if (subRealText == "\n" && selectionEnd == where && selectionStart == where) {
                Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
            } else {
                Workspace.insert(WorkspaceView.WGPU_OBJ, where, subRealText)
            }

            if (where < composingStart) {
                composingStart += subRealText.length
                composingEnd += subRealText.length
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            if (realText == "\n" && selectionEnd == where && selectionStart == where) {
                Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
            } else {
                Workspace.insert(WorkspaceView.WGPU_OBJ, where, realText.toString())
            }

            if (where < composingStart) {
                composingStart += realText.length
                composingEnd += realText.length
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun delete(st: Int, en: Int): Editable {
        Workspace.replace(WorkspaceView.WGPU_OBJ, st, en, "")

        if (en < composingStart) {
            composingStart -= (en - st)
            composingEnd -= (en - st)
        }

        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()

        return this
    }

    override fun clear() {
        Workspace.clear(WorkspaceView.WGPU_OBJ)

        composingStart = -1
        composingEnd = -1

        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()
    }

    override fun clearSpans() {
        if (composingStart != -1 || composingEnd != -1) {
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }
    override fun setFilters(filters: Array<out InputFilter>?) {}

    override fun getFilters(): Array<InputFilter> = arrayOf()
    override val length: Int get() {
        return Workspace.getTextLength(WorkspaceView.WGPU_OBJ)
    }
}
