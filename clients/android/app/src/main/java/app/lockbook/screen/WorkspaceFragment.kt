package app.lockbook.screen

import android.animation.ValueAnimator
import android.annotation.SuppressLint
import android.content.Context
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.GestureDetector
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.view.animation.Interpolator
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import android.widget.Toast
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.inputmethod.EditorInfoCompat
import androidx.core.view.inputmethod.InputConnectionCompat
import androidx.core.view.inputmethod.InputContentInfoCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.interpolator.view.animation.FastOutLinearInInterpolator
import androidx.interpolator.view.animation.LinearOutSlowInInterpolator
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentWorkspaceBinding
import app.lockbook.model.FileTreeViewModel
import app.lockbook.model.FinishedAction
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.model.UpdateMainScreenUI
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceTabType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.HorizontalTabItemHolder
import app.lockbook.util.MAX_CONTENT_SIZE
import app.lockbook.util.VerticalTabItemHolder
import app.lockbook.util.WorkspaceTextInputConnection
import app.lockbook.util.WorkspaceView
import app.lockbook.util.getIconResource
import app.lockbook.workspace.Workspace
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import net.lockbook.File
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

    private val filesListModel: FileTreeViewModel by activityViewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentWorkspaceBinding.inflate(inflater, container, false)

        val workspaceWrapper = WorkspaceWrapperView(requireContext(), model)
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

        onCreateToolbar(workspaceWrapper)
        onCreateTabList(workspaceWrapper)

        model.openFile.observe(viewLifecycleOwner) { (id, newFile) ->
            workspaceWrapper.workspaceView.openDoc(id, newFile)
            activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenWorkspacePane)
        }

        model.createDocAt.observe(viewLifecycleOwner) { it ->
            workspaceWrapper.workspaceView.createDocAt(it)
        }

        model.closeFile.observe(viewLifecycleOwner) { id ->
            workspaceWrapper.workspaceView.closeDoc(id)
        }

        model.currentTab.observe(viewLifecycleOwner) { tab ->
            updateCurrentTab(workspaceWrapper, tab)
            updateForwardButtonState(workspaceWrapper)
        }

        model.bottomInset.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.setBottomInset(it)
        }

        model.keyboardVisible.observe(viewLifecycleOwner) { keyboardVisible ->
            if (keyboardVisible) {
                binding.standardBottomSheet.visibility = View.GONE
            } else {
                binding.standardBottomSheet.visibility = View.VISIBLE
            }
        }

        model.workspaceBackRequested.observe(viewLifecycleOwner) {
            val navigatedBack = workspaceWrapper.workspaceView.back()
            if (!navigatedBack) {
                activityModel.updateMainScreenUI(UpdateMainScreenUI.CloseWorkspacePane)
            }
        }

        model.workspaceForwardRequested.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.forward()
        }

        model.finishedAction.observe(viewLifecycleOwner) { action ->
            when (action) {
                is FinishedAction.Delete -> workspaceWrapper.workspaceView.closeDoc(action.id)
                is FinishedAction.Rename -> {
                    workspaceWrapper.workspaceView.fileRenamed(action.id, action.name)
                    binding.workspaceToolbar.setTitle(action.name)
                    // todo: why the surgical update, can i just rebuild state
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

        return binding.root
    }

    private fun onCreateToolbar(workspaceWrapper: WorkspaceWrapperView) {
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

        binding.workspaceToolbar.setNavigationIcon(R.drawable.ic_baseline_arrow_back_24)
        binding.workspaceToolbar.setNavigationOnClickListener {
            (context?.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                .hideSoftInputFromWindow(view?.windowToken, 0)
            activityModel.updateMainScreenUI(UpdateMainScreenUI.CloseWorkspacePane)
        }

        binding.workspaceToolbar.setOnClickListener {
            getCurrentFile()?.let {
                activityModel.launchTransientScreen(TransientScreen.Rename(it))
            }
        }

        // is this needed
        getCurrentFile()?.let {
            binding.workspaceToolbar.setTitle(it.name)
        }

        model.hideToolbar.observe(viewLifecycleOwner) { distanceY ->
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
    }

    override fun onDestroyView() {
        _binding = null
        super.onDestroyView()
    }

    private fun toggleTabListExpansion(shouldExpand: Boolean) {
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
            binding.forwardWs.visibility = if (binding.forwardWs.isVisible) {
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

        model._showKeyboard.observe(viewLifecycleOwner) {
            if (it) {
                (context?.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                    .showSoftInput(view, InputMethodManager.SHOW_IMPLICIT)
            } else {
                (context?.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                    .hideSoftInputFromWindow(view.windowToken, 0)
            }
        }

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

        var tabTitle = filesListModel.fileModel.idsAndFiles[newTab.id]?.name

        if (isMissingTab(tabTitle, newTab)) {
            filesListModel.fileModel.refreshFiles()
            tabTitle = filesListModel.fileModel.idsAndFiles[newTab.id]?.name
        }

        if (isMissingTab(tabTitle, newTab)) {
            // todo: differentiate between startup nulls and legitimate nulls
            return
        }

        updateToolbarOnTabChange(newTab.type, tabTitle)

        val openTabs = workspaceWrapper.workspaceView.getTabs()
            .mapNotNull { tabId -> filesListModel.fileModel.idsAndFiles[tabId] }
            .toList()

        model.tabs.set(openTabs)
        binding.tabsList.adapter?.notifyDataSetChanged()
        binding.closeAllTabs.isEnabled = !model.tabs.isEmpty()

        scrollToCurrentTab(newTab.id)

        workspaceWrapper.updateWrapperBasedOnTab(newTab.type)
        showBottomSheet()
        showMaterialToolbar()
    }

    private fun scrollToCurrentTab(newTab: String?) {
        if (newTab == null) return
        val selectedPosition = model.tabs.indexOfFirst { it.id == newTab }
        if (selectedPosition != -1) {
            binding.tabsList.scrollToPosition(selectedPosition)
        }
    }

    private fun isMissingTab(tabTitle: String?, newTab: WorkspaceTab): Boolean {
        return tabTitle == null && newTab.type != WorkspaceTabType.Welcome
    }

    private fun updateToolbarOnTabChange(newTab: WorkspaceTabType, tabTitle: String?) {
        when (newTab) {
            WorkspaceTabType.Loading -> {}
            WorkspaceTabType.Welcome,
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

    private fun onCreateTabList(workspaceWrapper: WorkspaceWrapperView) {

        setupTabList()

        model.tabListExpanded.observe(viewLifecycleOwner) { shouldExpand ->
            toggleTabListExpansion(shouldExpand)
        }

        binding.expandList.setOnClickListener {
            model._tabListExpanded.value = !(model.tabListExpanded.value ?: false)
        }

        binding.forwardWs.setOnClickListener {
            workspaceWrapper.workspaceView.forward()
        }

        binding.closeAllTabs.setOnClickListener {
            workspaceWrapper.workspaceView.closeAllTabs()
            workspaceWrapper.workspaceView.drawImmediately()
            model._tabListExpanded.postValue(false)
            activityModel.updateMainScreenUI(UpdateMainScreenUI.CloseWorkspacePane)
        }
    }

    private fun updateForwardButtonState(workspaceWrapper: WorkspaceWrapperView) {
        binding.forwardWs.isEnabled = workspaceWrapper.workspaceView.canForward()
    }
    @SuppressLint("NotifyDataSetChanged")
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
                            binding.tabsList.adapter?.notifyDataSetChanged()
                        }

                        name.setOnClickListener {
                            switchTab(item.id)
                        }
                    }
                }
            }
        }

        scrollToCurrentTab(getCurrentFile()?.id)
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

    companion object {
        const val TAB_BAR_HEIGHT = 50
        const val TEXT_TOOL_BAR_HEIGHT = 45
//        val SVG_TOOL_BAR_HEIGHT = 50
    }

    val REG_LAYOUT_PARAMS = ViewGroup.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.MATCH_PARENT
    )

    private fun topInsetPxForTextWrapper(): Int {
        val scaledDensity = context.resources.displayMetrics.scaledDensity
        val topInsetDp =  TEXT_TOOL_BAR_HEIGHT
        return (topInsetDp * scaledDensity).toInt()
    }

    private fun textWrapperLayoutParams(topInsetPx: Int): MarginLayoutParams {
        val scaledDensity = context.resources.displayMetrics.scaledDensity
        return MarginLayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        ).apply {
            topMargin = topInsetPx
            bottomMargin = (TEXT_TOOL_BAR_HEIGHT * scaledDensity).toInt()
        }
    }

    init {
        workspaceView = WorkspaceView(context, model)
        addView(workspaceView, REG_LAYOUT_PARAMS)
    }

    fun updateWrapperBasedOnTab(newTab: WorkspaceTabType) {
        detachWrapperIfPresent()
        attachWrapperIfNeeded(newTab)

        currentTab = newTab
    }

    private fun detachWrapperIfPresent() {
        val wrapper = currentWrapper ?: return

        // Clear the link from the native view immediately to avoid stale references.
        workspaceView.wrapperView = null
        currentWrapper = null

        if (wrapper is WorkspaceTextInputWrapper) {
            wrapper.wsInputConnection.closeConnection()
            wrapper.clearFocus()
            (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                .hideSoftInputFromWindow(this.windowToken, 0)
            Handler(Looper.getMainLooper()).postDelayed(
                {
                    if (wrapper.parent === this) {
                        removeView(wrapper)
                    }
                },
                200
            )
        }
    }

    private fun attachWrapperIfNeeded(newTab: WorkspaceTabType) {
        if (!newTab.isTextEdit()) {
            return
        }

        val topInsetPx = topInsetPxForTextWrapper()
        val wrapper = WorkspaceTextInputWrapper(context, workspaceView, topInsetPx.toFloat())
        currentWrapper = wrapper
        workspaceView.wrapperView = wrapper
        addView(wrapper, textWrapperLayoutParams(topInsetPx))
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onInterceptTouchEvent(event: MotionEvent?): Boolean {

        if (model.tabListExpanded.value ?: true) {
            model._tabListExpanded.postValue(false)
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

    private val tabSheetScrollListener = object : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(e: MotionEvent): Boolean {
            return true
        }
        override fun onScroll(
            e1: MotionEvent?,
            e2: MotionEvent,
            distanceX: Float,
            distanceY: Float
        ): Boolean {
            workspaceView.model._hideToolbar.value = distanceY
            return super.onScroll(e1, e2, distanceX, distanceY)
        }
    }
    private val tabSheetScrollDetector: GestureDetector = GestureDetector(context, tabSheetScrollListener)

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

                val bottomSheetExpanded = workspaceView.model.tabListExpanded.value ?: false

                val slopTouchThreshold = ViewConfiguration.get(context).scaledTouchSlop
                val nonSloppyTouch = abs(event.x - touchStartX).toInt() < slopTouchThreshold &&
                    abs(event.y - touchStartY).toInt() < slopTouchThreshold
                if (!bottomSheetExpanded && !keyboardShown && duration < 300 && nonSloppyTouch) {
                    (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                        .showSoftInput(this, InputMethodManager.SHOW_IMPLICIT)
                }
            }
        }

        if (event != null) {
            workspaceView.forwardedTouchEvent(event, touchYOffset)
            tabSheetScrollDetector.onTouchEvent(event)
        }


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

            EditorInfoCompat.setContentMimeTypes(outAttrs, arrayOf("image/*"))

            outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN
            outAttrs.initialSelStart = wsInputConnection.wsEditable.selectionStart
            outAttrs.initialSelEnd = wsInputConnection.wsEditable.selectionEnd
        }

        if (outAttrs == null) {
            return wsInputConnection
        }

        // Handle `commitContent` from IMEs (e.g. Gboard clipboard images).
        return InputConnectionCompat.createWrapper(wsInputConnection, outAttrs) { inputContentInfo, flags, _ ->
            handleCommitContent(inputContentInfo, flags)
        }
    }

    private fun handleCommitContent(inputContentInfo: InputContentInfoCompat, flags: Int): Boolean {
        val isImage = inputContentInfo.description.hasMimeType("image/*")
        if (!isImage) {
            Toast
                .makeText(context, "Clipboard content not supported", Toast.LENGTH_SHORT)
                .show()
            return false
        }

        val needsPermission =
            (flags and InputConnectionCompat.INPUT_CONTENT_GRANT_READ_URI_PERMISSION) != 0
        if (needsPermission) {
            try {
                inputContentInfo.requestPermission()
            } catch (_: Exception) {
                Toast
                    .makeText(context, "Could not read pasted content", Toast.LENGTH_SHORT)
                    .show()
                return false
            }
        }

        val uri = inputContentInfo.contentUri
        val appContext = context.applicationContext
        workspaceView.launchIo {
            val bytes = try {
                wsInputConnection.readAllBytesCapped(uri, MAX_CONTENT_SIZE)
            } catch (_: Exception) {
                null
            } finally {
                if (needsPermission) {
                    try {
                        inputContentInfo.releasePermission()
                    } catch (_: Exception) {
                    }
                }
            }

            if (bytes != null) {
                withContext(Dispatchers.Main){
                    Workspace.clipboardSendImage(WorkspaceView.WGPU_OBJ, bytes, true)
                    workspaceView.drawImmediately()
                }
            } else {
                Toast
                    .makeText(appContext, "Clipboard image too large or unreadable", Toast.LENGTH_SHORT)
                    .show()
            }
        }

        return true
    }
}
