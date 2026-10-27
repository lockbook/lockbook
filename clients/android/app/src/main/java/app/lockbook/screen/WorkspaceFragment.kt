package app.lockbook.screen

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.Paint
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.GestureDetector
import android.view.HapticFeedbackConstants
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import android.widget.Toast
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.widget.PopupMenu
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.core.graphics.ColorUtils
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.inputmethod.EditorInfoCompat
import androidx.core.view.inputmethod.InputConnectionCompat
import androidx.core.view.inputmethod.InputContentInfoCompat
import androidx.core.view.isVisible
import androidx.core.view.updatePadding
import androidx.core.widget.NestedScrollView
import androidx.core.widget.doAfterTextChanged
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.interpolator.view.animation.FastOutLinearInInterpolator
import androidx.interpolator.view.animation.LinearOutSlowInInterpolator
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.StaggeredGridLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentWorkspaceBinding
import app.lockbook.databinding.WorkspaceTabsDialogBinding
import app.lockbook.model.FileTreeViewModel
import app.lockbook.model.FinishedAction
import app.lockbook.model.MainNavigationAction
import app.lockbook.model.MainScreenViewModel
import app.lockbook.model.OpenFilePresentation
import app.lockbook.model.OpenTab
import app.lockbook.model.QuickCaptureViewModel
import app.lockbook.model.SearchPresentation
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceTabType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.model.workspaceDocumentItems
import app.lockbook.ui.CreateFileBottomSheetFragment
import app.lockbook.util.AttachmentStager
import app.lockbook.util.DocumentCoverStore
import app.lockbook.util.DocumentPreviewLoader
import app.lockbook.util.MAX_ATTACHMENT_SIZE_BYTES
import app.lockbook.util.MarkdownToolbarView
import app.lockbook.util.VerticalTabItemHolder
import app.lockbook.util.WorkspaceTextInputConnection
import app.lockbook.util.WorkspaceView
import app.lockbook.util.getIconResource
import app.lockbook.workspace.Workspace
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.color.MaterialColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import kotlin.math.abs
import kotlin.math.roundToInt
import java.io.File as JavaFile

private const val MAX_PICKED_PHOTOS = 10

class WorkspaceFragment : Fragment() {
    private var _binding: FragmentWorkspaceBinding? = null
    val binding get() = _binding!!

    private val mainScreenModel: MainScreenViewModel by activityViewModels()
    private val model: WorkspaceViewModel by activityViewModels()
    private val quickCapture: QuickCaptureViewModel by viewModels()

    private var toolbarVisible: Boolean? = null
    private var systemControlInsets = Insets.NONE
    private var toolbarHiddenByScroll = false
    private var tabsDialog: BottomSheetDialog? = null
    private var tabsContent: WorkspaceTabsDialogBinding? = null
    private var documentActionsController: FileSelectionBottomBarController? = null
    private var documentActionsBackCallback: OnBackPressedCallback? = null
    private var documentActionFile: File? = null
    private var workspaceView: WorkspaceView? = null
    private var pendingTabClose: String? = null
    private var pendingTabCloseJob: Job? = null
    private var cameraFile: JavaFile? = null
    private var captureExpansion: QuickCaptureExpansion? = null
    private var captureStart: Runnable? = null
    private var captureEditorId: String? = null
    private var coverFileId: String? = null
    private var documentsAdapter: WorkspaceDocumentsAdapter? = null
    private val documentCovers by lazy { DocumentCoverStore(requireContext().applicationContext) }
    private val documentPreviews by lazy { DocumentPreviewLoader(requireContext().applicationContext, documentCovers) }
    private var previewRefresh: Job? = null

    private val chooseCover =
        registerForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
            val id = coverFileId
            coverFileId = null
            if (uri != null && id != null) {
                viewLifecycleOwner.lifecycleScope.launch {
                    val saved = withContext(Dispatchers.IO) { runCatching { documentCovers.save(id, uri) }.isSuccess }
                    if (saved) documentsAdapter?.refreshCover(id) else toast(R.string.workspace_cover_error)
                }
            }
        }

    private val cameraPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) launchCamera() else toast(R.string.workspace_camera_permission_required)
        }
    private val takePicture =
        registerForActivityResult(ActivityResultContracts.TakePicture()) { success ->
            val file = cameraFile
            cameraFile = null
            if (success && file != null) {
                importAttachments(listOf(Uri.fromFile(file)), file.name)
            } else {
                file?.delete()
            }
        }
    private val pickPhotos =
        registerForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(MAX_PICKED_PHOTOS)) { uris ->
            if (uris.size > MAX_PICKED_PHOTOS) toast(R.string.workspace_photo_selection_limit)
            importAttachments(uris.take(MAX_PICKED_PHOTOS))
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        cameraFile = savedInstanceState?.getString("cameraPath")?.let(::JavaFile)
        coverFileId = savedInstanceState?.getString("coverFileId")
        captureEditorId = savedInstanceState?.getString("captureEditorId")
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        cameraFile?.let { outState.putString("cameraPath", it.absolutePath) }
        coverFileId?.let { outState.putString("coverFileId", it) }
        captureEditorId?.let { outState.putString("captureEditorId", it) }
    }

    companion object {
        val TAG = "WorkspaceFragment"
        private const val QUICK_CAPTURE_FOLDER = "quick_capture_folder"
    }

    private val filesListModel: FileTreeViewModel by activityViewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = FragmentWorkspaceBinding.inflate(inflater, container, false)

        val workspaceWrapper = WorkspaceWrapperView(requireContext(), model)
        workspaceView = workspaceWrapper.workspaceView
        val layoutParams =
            ConstraintLayout
                .LayoutParams(
                    ConstraintLayout.LayoutParams.MATCH_CONSTRAINT,
                    ConstraintLayout.LayoutParams.MATCH_CONSTRAINT,
                ).apply {
                    startToStart = ConstraintLayout.LayoutParams.PARENT_ID
                    endToEnd = ConstraintLayout.LayoutParams.PARENT_ID
                    topToTop = ConstraintLayout.LayoutParams.PARENT_ID
                    bottomToBottom = ConstraintLayout.LayoutParams.PARENT_ID
                }

        binding.workspaceRoot.addView(workspaceWrapper, layoutParams)

        onCreateEmptyWorkspace()
        onCreateToolbar(workspaceWrapper)
        onCreateDocumentActions()

        model.openFile.observe(viewLifecycleOwner) { request ->
            finishPendingTabClose()
            model.isFileTreeSyncedToCurrentTab = true
            workspaceWrapper.workspaceView.openDoc(request)

            when (request.presentation) {
                OpenFilePresentation.Preserve -> {
                    // The initiating navigation or tab-switch flow already owns the pane layout.
                }

                OpenFilePresentation.ShowDetail -> {
                    mainScreenModel.navigate(MainNavigationAction.ShowDetail)
                }
            }
        }

        model.takePhotoRequested.observe(viewLifecycleOwner) {
            hideEditorKeyboard()
            if (ContextCompat.checkSelfPermission(requireContext(), Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED
            ) {
                launchCamera()
            } else {
                cameraPermission.launch(Manifest.permission.CAMERA)
            }
        }

        model.choosePhotosRequested.observe(viewLifecycleOwner) {
            hideEditorKeyboard()
            pickPhotos.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
        }

        // Forward real IME visibility into the editor so touch long-press
        // can pick drag-reorder (keyboard down) vs text selection (up).
        model.keyboardVisible.observe(viewLifecycleOwner) { visible ->
            workspaceWrapper.workspaceView.setKeyboardShown(visible)
            workspaceWrapper.updateNativeToolbar()
        }

        model.closeFile.observe(viewLifecycleOwner) { id ->
            workspaceWrapper.workspaceView.closeDoc(id)
        }

        model.currentTab.observe(viewLifecycleOwner) { tab ->
            if (documentActionFile?.id != tab.id) hideDocumentActions()
            updateCurrentTab(workspaceWrapper, tab)
            workspaceWrapper.updateNativeToolbar()
            finishCaptureHandoff(workspaceWrapper, tab)
        }

        model.bottomInset.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.setBottomInset(it)
            workspaceWrapper.setBottomInset(it)
        }

        model.keyboardVisible.observe(viewLifecycleOwner) { keyboardVisible ->
            if (keyboardVisible) {
                toolbarHiddenByScroll = false
                hideDocumentActions()
            }
            updateToolbarVisibility()
        }

        model.workspaceBackRequested.observe(viewLifecycleOwner) {
            val view = workspaceWrapper.workspaceView
            if (model.outlineOpen.value == true) {
                view.toggleOutline()
                return@observe
            }
            if (view.back()) return@observe
            val tab = model.currentTab.value
            val closeDetailOnBack = tab?.let { model.shouldCloseDetailOnBack(it.sessionId) } ?: true
            val activity = requireActivity() as MainScreenActivity
            val deferClose = closeDetailOnBack && model.openTabCount.value == 1 && activity.isFocusingDetail()
            if (tab?.type != WorkspaceTabType.Welcome) {
                val closed = !deferClose && view.closeCurrentTabToWelcome()
                if (!closeDetailOnBack && closed) return@observe
            }
            if (deferClose) {
                tab?.sessionId?.let { sessionId ->
                    finishPendingTabClose()
                    pendingTabClose = sessionId
                    pendingTabCloseJob =
                        viewLifecycleOwner.lifecycleScope.launch {
                            delay(250)
                            finishPendingTabClose()
                        }
                }
            }
            mainScreenModel.navigate(MainNavigationAction.ShowSidebar)
        }

        model.backGestureStarted.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.touchForwarder.cancelBackGestureTouches()
        }

        model.workspaceForwardRequested.observe(viewLifecycleOwner) {
            workspaceWrapper.workspaceView.forward()
        }

        model.finishedAction.observe(viewLifecycleOwner) { action ->
            when (action) {
                is FinishedAction.Delete -> {
                    workspaceWrapper.workspaceView.closeDoc(action.id)
                }

                is FinishedAction.Rename -> {
                    workspaceWrapper.workspaceView.fileRenamed(action.id, action.name)
                    if (getCurrentFile()?.id == action.id) updateDocumentName(action.name)
                    val tabIndex = model.tabs.indexOfFirst { it.file.id == action.id }
                    if (tabIndex != -1) {
                        val updatedTab = model.tabs.get(tabIndex)
                        updatedTab.file.name = action.name

                        model.tabs.removeAt(tabIndex)
                        model.tabs.insert(tabIndex, updatedTab)

                        tabsContent?.tabsList?.adapter?.notifyItemChanged(tabIndex)
                    }
                }
            }
        }

        return binding.root
    }

    private fun hideEditorKeyboard() {
        workspaceView?.let { editor ->
            (requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                .hideSoftInputFromWindow(editor.windowToken, 0)
        }
    }

    private fun launchCamera() {
        try {
            val dir = JavaFile(requireContext().cacheDir, "camera").apply { mkdirs() }
            val file = JavaFile.createTempFile("photo_", ".jpg", dir)
            cameraFile = file
            val uri = FileProvider.getUriForFile(requireContext(), "${requireContext().packageName}.fileprovider", file)
            takePicture.launch(uri)
        } catch (_: Exception) {
            cameraFile?.delete()
            cameraFile = null
            toast(R.string.workspace_camera_unavailable)
        }
    }

    private fun importAttachments(
        uris: List<Uri>,
        nameHint: String? = null,
    ) {
        if (uris.isEmpty()) return
        val appContext = requireContext().applicationContext
        lifecycleScope.launch {
            for (uri in uris) {
                val staged =
                    withContext(Dispatchers.IO) {
                        runCatching { AttachmentStager.stage(appContext, uri, nameHint) }.getOrNull()
                    }
                if (uri.scheme == "file") JavaFile(uri.path.orEmpty()).delete()
                if (staged == null) {
                    toast(R.string.workspace_attachment_unreadable)
                } else {
                    model.enqueueAttachment(staged)
                    workspaceView?.invalidate()
                }
            }
        }
    }

    private fun toast(id: Int) {
        context?.let { Toast.makeText(it, id, Toast.LENGTH_SHORT).show() }
    }

    private fun onCreateEmptyWorkspace() {
        val empty = binding.workspaceEmpty
        empty.captureFolder.paintFlags = empty.captureFolder.paintFlags or Paint.UNDERLINE_TEXT_FLAG
        childFragmentManager.setFragmentResultListener(QUICK_CAPTURE_FOLDER, viewLifecycleOwner) { _, result ->
            result.getString(CreateFileBottomSheetFragment.SELECTED_FOLDER_ID)?.let(quickCapture::selectSaveFolder)
        }
        empty.captureFolder.setOnClickListener {
            if (captureExpansion == null && quickCapture.isSaving.value != true &&
                childFragmentManager.findFragmentByTag(QUICK_CAPTURE_FOLDER) == null
            ) {
                hideEditorKeyboard()
                CreateFileBottomSheetFragment
                    .folderPicker(quickCapture.saveFolder.value?.id, QUICK_CAPTURE_FOLDER)
                    .show(childFragmentManager, QUICK_CAPTURE_FOLDER)
            }
        }
        quickCapture.saveFolder.observe(viewLifecycleOwner) { folder ->
            val name = if (folder == null || folder.isRoot) getString(R.string.home) else folder.name
            empty.captureFolder.text = name
            empty.captureFolder.contentDescription = getString(R.string.quick_capture_change_folder, name)
        }
        empty.emptyScroll.setOnScrollChangeListener(
            NestedScrollView.OnScrollChangeListener { _, _, y, _, oldY -> onWorkspaceScroll((y - oldY).toFloat()) },
        )
        val surface = MaterialColors.getColor(empty.root, com.google.android.material.R.attr.colorSurface)
        val captureColor =
            if (MaterialColors.isColorLight(surface)) {
                com.google.android.material.R.attr.colorSurfaceContainerLowest
            } else {
                com.google.android.material.R.attr.colorSurfaceContainerHigh
            }
        empty.captureSurface.setCardBackgroundColor(MaterialColors.getColor(empty.root, captureColor))
        empty.captureSurface.strokeColor =
            ColorUtils.blendARGB(
                empty.captureSurface.cardBackgroundColor.defaultColor,
                MaterialColors.getColor(empty.root, androidx.appcompat.R.attr.colorPrimary),
                0.18f,
            )
        empty.captureText.setText(quickCapture.draft.value)
        empty.captureText.doAfterTextChanged { text -> quickCapture.setDraft(text.toString()) }
        quickCapture.draft.observe(viewLifecycleOwner) { text ->
            if (empty.captureText.text.toString() != text) empty.captureText.setText(text)
            if (text.isNotEmpty() || quickCapture.isSaving.value != true) captureExpansion?.updateText(text)
            scheduleCaptureExpansion(text)
        }
        quickCapture.error.observe(viewLifecycleOwner) { error ->
            empty.captureInput.error = error
            if (error != null) {
                captureExpansion?.cancel()
                captureExpansion = null
                empty.captureText.isEnabled = true
            }
        }
        quickCapture.createdNote.observe(viewLifecycleOwner) { id ->
            captureEditorId = id
            binding.workspaceEmpty.captureText.performHapticFeedback(
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    HapticFeedbackConstants.CONFIRM
                } else {
                    HapticFeedbackConstants.KEYBOARD_TAP
                },
            )
            filesListModel.reloadFiles()
            mainScreenModel.navigate(MainNavigationAction.OpenDocument(id, newFile = false, closeDetailOnBack = false))
        }
        val adapter =
            WorkspaceDocumentsAdapter(
                scope = viewLifecycleOwner.lifecycleScope,
                previews = documentPreviews,
                openDocument = { file ->
                    if (captureExpansion != null) return@WorkspaceDocumentsAdapter
                    hideEditorKeyboard()
                    mainScreenModel.navigate(MainNavigationAction.OpenDocument(file.id, newFile = false, closeDetailOnBack = false))
                },
                showCoverMenu = ::showCoverMenu,
            )
        documentsAdapter = adapter
        val grid = StaggeredGridLayoutManager(2, StaggeredGridLayoutManager.VERTICAL)
        empty.documentGrid.layoutManager = grid
        empty.documentGrid.adapter = adapter
        empty.documentGrid.itemAnimator = null
        empty.documentGrid.addOnLayoutChangeListener { view, _, _, _, _, _, _, _, _ ->
            val columns = (view.width / (180 * resources.displayMetrics.density)).toInt().coerceIn(2, 4)
            if (grid.spanCount != columns) grid.spanCount = columns
        }
        filesListModel.recentFiles.observe(viewLifecycleOwner) { updateWorkspaceDocuments() }
        filesListModel.pinnedFiles.observe(viewLifecycleOwner) { updateWorkspaceDocuments() }
        model.bottomInset.observe(viewLifecycleOwner) { inset ->
            empty.root.updatePadding(bottom = inset)
        }
    }

    private fun updateWorkspaceDocuments() {
        if (!isWelcomeVisible()) return
        val items = workspaceDocumentItems(filesListModel.pinnedFiles.value.orEmpty(), filesListModel.recentFiles.value.orEmpty())
        binding.workspaceEmpty.documentSection.isVisible = items.isNotEmpty()
        previewRefresh?.cancel()
        if (items.isEmpty()) {
            documentsAdapter?.submitList(emptyList())
            return
        }
        previewRefresh =
            viewLifecycleOwner.lifecycleScope.launch {
                val cursors = documentPreviews.cursorPositions()
                documentsAdapter?.submitList(items.map { it.copy(cursor = cursors[it.file.id] ?: 0) })
            }
    }

    private fun scheduleCaptureExpansion(text: String) {
        val input = binding.workspaceEmpty.captureText
        captureStart?.let(input::removeCallbacks)
        captureStart = null
        if (text.isBlank() || captureExpansion != null || quickCapture.isSaving.value == true || !isWelcomeVisible()) return
        captureStart =
            Runnable {
                captureStart = null
                if (_binding == null || !binding.root.isShown ||
                    !isWelcomeVisible() || quickCapture.isSaving.value == true
                ) {
                    return@Runnable
                }
                val expansion = QuickCaptureExpansion(binding.root, binding.workspaceEmpty.captureSurface, input)
                captureExpansion = expansion
                expansion.start {
                    if (quickCapture.draft.value
                            .orEmpty()
                            .isBlank()
                    ) {
                        expansion.cancel()
                        captureExpansion = null
                    } else {
                        input.clearComposingText()
                        quickCapture.save()
                    }
                }
            }.also { input.postDelayed(it, 350) }
    }

    private fun finishCaptureHandoff(
        wrapper: WorkspaceWrapperView,
        tab: WorkspaceTab,
    ) {
        if (captureEditorId != tab.id || tab.type != WorkspaceTabType.Markdown) return
        val editor = wrapper.currentWrapper ?: return
        val input = binding.workspaceEmpty.captureText
        val keepKeyboard = input.hasFocus() && model.keyboardVisible.value == true
        input.clearComposingText()
        val pendingText = quickCapture.draft.value.orEmpty()
        val length = Workspace.getTextLength(WorkspaceView.wgpuObj)
        Workspace.setSelection(WorkspaceView.wgpuObj, length, length)
        if (pendingText.isNotEmpty()) Workspace.insert(WorkspaceView.wgpuObj, length, pendingText)

        // Transfer the IME connection before hiding its old editor, so the keyboard stays open.
        editor.requestFocus()
        captureEditorId = null
        binding.workspaceEmpty.root.isVisible = false
        if (pendingText.isNotEmpty()) quickCapture.setDraft("")
        val keyboard = requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        keyboard.restartInput(editor)
        if (keepKeyboard) {
            editor.post {
                if (editor.hasFocus()) keyboard.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT)
            }
        }
        wrapper.workspaceView.drawImmediately()
        captureExpansion?.finish { captureExpansion = null }
    }

    private fun showCoverMenu(
        file: File,
        anchor: View,
    ) {
        PopupMenu(requireContext(), anchor).apply {
            menu.add(0, 1, 0, R.string.workspace_choose_cover)
            if (documentCovers.file(file.id).exists()) menu.add(0, 2, 1, R.string.workspace_remove_cover)
            setOnMenuItemClickListener { item ->
                if (item.itemId == 1) {
                    coverFileId = file.id
                    chooseCover.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
                } else {
                    viewLifecycleOwner.lifecycleScope.launch {
                        val removed = withContext(Dispatchers.IO) { documentCovers.file(file.id).delete() }
                        if (removed) documentsAdapter?.refreshCover(file.id) else toast(R.string.workspace_cover_error)
                    }
                }
                true
            }
            show()
        }
    }

    private fun onCreateToolbar(workspaceWrapper: WorkspaceWrapperView) {
        model.outlineOpen.observe(viewLifecycleOwner) { open ->
            if (open) {
                hideDocumentActions()
                tabsDialog?.dismiss()
            }
            updateToolbarVisibility()
            workspaceWrapper.updateNativeToolbar()
        }
        binding.documentName.setOnClickListener {
            if (documentActionFile == null) showDocumentActions() else hideDocumentActions()
        }
        binding.restoreSplit.setOnClickListener {
            hideEditorKeyboard()
            mainScreenModel.navigate(MainNavigationAction.ShowSplitOrSidebar)
        }
        binding.searchWs.setOnClickListener {
            mainScreenModel.navigate(MainNavigationAction.OpenSearch(SearchPresentation.FullScreen))
        }
        binding.switchTabs.setOnClickListener { showTabPicker(workspaceWrapper) }
        binding.root.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> updateControlInsets() }
        binding.workspaceToolbar.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> updateControlInsets() }
        model.openTabCount.observe(viewLifecycleOwner) { count -> updateTabControls(count) }
        updateRestoreSplitButtonVisibility((activity as? MainScreenActivity)?.isFocusingDetail() == true)
        updateWorkspaceSearchButtonVisibility((activity as? MainScreenActivity)?.isShowingSplit() != true)
        updateDocumentName(getCurrentFile()?.name)
        workspaceWrapper.onVerticalScroll = ::onWorkspaceScroll
    }

    private fun isWelcomeVisible(): Boolean =
        model.currentTab.value
            ?.type
            .let { it == null || it == WorkspaceTabType.Welcome }

    private fun canToolbarHideOnScroll(): Boolean = isWelcomeVisible() || model.currentTab.value?.type == WorkspaceTabType.Markdown

    private fun onWorkspaceScroll(distanceY: Float) {
        if (model.outlineOpen.value == true) return
        if (distanceY > 0 && model.currentTab.value?.type == WorkspaceTabType.Markdown) hideDocumentActions()
        if (distanceY != 0f && canToolbarHideOnScroll() &&
            model.keyboardVisible.value != true && documentActionFile == null && captureExpansion == null
        ) {
            toolbarHiddenByScroll = distanceY > 0
            updateToolbarVisibility()
        }
    }

    private fun updateTabControls(tabCount: Int) {
        if (tabCount == 0) tabsDialog?.dismiss()
        val empty = isWelcomeVisible()
        binding.switchTabs.isVisible = tabCount > 0
        binding.workspaceEmpty.root.isVisible = empty || captureExpansion != null
        binding.workspaceRoot.importantForAccessibility =
            if (empty) View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS else View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
        if (empty) {
            binding.workspaceEmpty.captureText.isEnabled = true
            quickCapture.refreshSaveFolder()
            filesListModel.reloadFiles()
        } else if (captureExpansion == null) {
            binding.workspaceEmpty.captureText.clearFocus()
        }
        if (tabCount > 0) {
            binding.documentName.setIconResource(R.drawable.ic_baseline_arrow_down_24)
        } else {
            binding.documentName.icon = null
        }
    }

    private fun onCreateDocumentActions() {
        val backCallback =
            object : OnBackPressedCallback(false) {
                override fun handleOnBackPressed() = hideDocumentActions()
            }
        documentActionsBackCallback = backCallback
        requireActivity().onBackPressedDispatcher.addCallback(viewLifecycleOwner, backCallback)
        binding.workspaceDocumentActions.root.isClickable = true
        val dispatcher =
            FileSelectionActionDispatcher(
                fragment = this,
                mainScreenModel = mainScreenModel,
                fileTreeModel = filesListModel,
                snackbarAnchor = binding.workspaceToolbar,
                onClearSelection = {},
            )
        documentActionsController =
            FileSelectionBottomBarController(
                root = binding.workspaceDocumentActions.root,
                onAction = { action, files ->
                    hideDocumentActions()
                    dispatcher.dispatch(action, files)
                },
                onClearSelection = ::hideDocumentActions,
                onVisibilityChanged = { visible ->
                    backCallback.isEnabled = visible
                    binding.workspaceToolbar.importantForAccessibility =
                        if (visible) {
                            View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
                        } else {
                            View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
                        }
                },
            )
    }

    private fun showDocumentActions() {
        val file = getCurrentFile() ?: return
        documentActionFile = file
        documentActionsController?.render(FileSelectionUiState(selectedFiles = listOf(file)), file.name)
    }

    private fun hideDocumentActions() {
        if (documentActionFile == null) return
        documentActionFile = null
        documentActionsController?.render(FileSelectionUiState())
    }

    private fun updateDocumentName(name: String?) {
        documentActionFile?.let { file ->
            documentActionsController?.render(FileSelectionUiState(selectedFiles = listOf(file)), name ?: file.name)
        }
        val title = name?.takeIf { it.isNotBlank() }
        binding.documentName.text = title.orEmpty()
        binding.documentName.isVisible = title != null
        binding.documentName.isEnabled = title != null
        binding.documentName.contentDescription = title?.let { getString(R.string.workspace_document_actions, it) }
    }

    fun updateRestoreSplitButtonVisibility(isDetailFocused: Boolean) {
        _binding?.restoreSplit?.isVisible = isDetailFocused
    }

    fun updateWorkspaceSearchButtonVisibility(isSearchAvailable: Boolean) {
        _binding?.searchWs?.isVisible = isSearchAvailable
    }

    private fun updateControlInsets() {
        val views = _binding ?: return
        val surface = workspaceView ?: return
        if (surface.width == 0 || surface.height == 0) return
        val window = surface.rootView
        val origin = IntArray(2).also(surface::getLocationInWindow)
        val windowOrigin = IntArray(2).also(window::getLocationInWindow)
        val left = (windowOrigin[0] + systemControlInsets.left - origin[0]).coerceAtLeast(0)
        val top = (windowOrigin[1] + systemControlInsets.top - origin[1]).coerceAtLeast(0)
        val right = (origin[0] + surface.width - windowOrigin[0] - window.width + systemControlInsets.right).coerceAtLeast(0)
        val systemBottom = (origin[1] + surface.height - windowOrigin[1] - window.height + systemControlInsets.bottom).coerceAtLeast(0)
        val toolbar = views.workspaceToolbar
        val toolbarBottom =
            if (toolbar.isVisible && model.keyboardVisible.value != true) {
                val toolbarOrigin = IntArray(2).also(toolbar::getLocationInWindow)
                (origin[1] + surface.height - toolbarOrigin[1]).coerceIn(0, surface.height)
            } else {
                0
            }
        // Coordinates are relative to the SurfaceView: system padding already outside it is not counted twice.
        surface.setSafeInsets(Insets.of(left, top, right, maxOf(systemBottom, toolbarBottom)))
    }

    private fun updateToolbarVisibility() {
        val hiddenByScroll = canToolbarHideOnScroll() && toolbarHiddenByScroll
        val shouldShow = model.keyboardVisible.value != true && model.outlineOpen.value != true && !hiddenByScroll
        if (toolbarVisible == shouldShow) return
        toolbarVisible = shouldShow
        val toolbar = binding.workspaceToolbar
        toolbar.animate().cancel()
        updateControlInsets()
        if (shouldShow) {
            toolbar.isVisible = true
            toolbar
                .animate()
                .alpha(1f)
                .translationY(0f)
                .setUpdateListener { updateControlInsets() }
                .setDuration(220)
                .start()
        } else {
            toolbar
                .animate()
                .alpha(0f)
                .translationY(toolbar.height.toFloat() + 16 * resources.displayMetrics.density)
                .setUpdateListener { updateControlInsets() }
                .setDuration(180)
                .withEndAction {
                    toolbar.isVisible = false
                    updateControlInsets()
                }.start()
        }
    }

    fun finishPendingTabClose() {
        val sessionId = pendingTabClose ?: return
        pendingTabClose = null
        pendingTabCloseJob?.cancel()
        pendingTabCloseJob = null
        workspaceView?.closeSession(sessionId)
    }

    override fun onDestroyView() {
        finishPendingTabClose()
        previewRefresh?.cancel()
        previewRefresh = null
        captureStart?.let { binding.workspaceEmpty.captureText.removeCallbacks(it) }
        captureStart = null
        captureExpansion?.cancel()
        captureExpansion = null
        binding.workspaceEmpty.documentGrid.adapter = null
        documentsAdapter = null
        tabsDialog?.dismiss()
        tabsDialog = null
        tabsContent = null
        binding.workspaceDocumentActions.root
            .animate()
            .cancel()
        documentActionsController = null
        documentActionsBackCallback = null
        documentActionFile = null
        binding.workspaceToolbar.animate().cancel()
        toolbarVisible = null
        toolbarHiddenByScroll = false
        workspaceView = null
        _binding = null
        super.onDestroyView()
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
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

        val actionSheet = binding.workspaceDocumentActions.root
        val sheetBottomPadding = actionSheet.contentPaddingBottom
        val sheetMinHeight = actionSheet.minimumHeight
        ViewCompat.setOnApplyWindowInsetsListener(view) { v, insets ->
            val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            systemControlInsets =
                insets.getInsets(
                    WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or
                        WindowInsetsCompat.Type.systemGestures(),
                )

            if (imeVisible) {
                model._keyboardVisible.postValue(true)
            } else {
                model._keyboardVisible.postValue(false)
            }

            model._bottomInset.value = (ime.bottom - systemBars.bottom).coerceAtLeast(0)

            val filteredInsets =
                WindowInsetsCompat
                    .Builder(insets)
                    .setInsets(
                        WindowInsetsCompat.Type.ime(),
                        androidx.core.graphics.Insets.NONE, // Mask keyboard height to 0
                    ).build()

            ViewCompat.onApplyWindowInsets(v, filteredInsets)

            // Extend the sheet's surface through the space reserved for system navigation,
            // keeping its controls and the editor inside the existing safe area.
            val navigationInset = v.paddingBottom
            val sheetParams = actionSheet.layoutParams as ConstraintLayout.LayoutParams
            if (sheetParams.bottomMargin != -navigationInset) {
                sheetParams.bottomMargin = -navigationInset
                actionSheet.layoutParams = sheetParams
            }
            if (actionSheet.contentPaddingBottom != sheetBottomPadding + navigationInset) {
                actionSheet.setContentPadding(
                    actionSheet.contentPaddingLeft,
                    actionSheet.contentPaddingTop,
                    actionSheet.contentPaddingRight,
                    sheetBottomPadding + navigationInset,
                )
            }
            if (actionSheet.minimumHeight != sheetMinHeight + navigationInset) {
                actionSheet.minimumHeight = sheetMinHeight + navigationInset
            }

            updateControlInsets()
            v.post { updateControlInsets() }
            insets
        }
    }

    @SuppressLint("NotifyDataSetChanged")
    private fun updateCurrentTab(
        workspaceWrapper: WorkspaceWrapperView,
        newTab: WorkspaceTab,
    ) {
        val nativeTabs = workspaceWrapper.workspaceView.getTabs()

        var tabTitle = filesListModel.fileModel.idsAndFiles[newTab.id]?.name
        if (isMissingTab(tabTitle, newTab)) {
            filesListModel.fileModel.refreshFiles()
            tabTitle = filesListModel.fileModel.idsAndFiles[newTab.id]?.name
        }

        val openTabs =
            nativeTabs.mapNotNull { raw ->
                val parts = raw.split(' ', limit = 2)
                val sessionId = parts[0]
                val destId = parts.getOrElse(1) { sessionId }
                filesListModel.fileModel.idsAndFiles[destId]?.let { file ->
                    OpenTab(sessionId, file)
                }
            }
        model.tabs.set(openTabs)
        notifyTabListsChanged()

        updateTabControls(nativeTabs.size)
        updateDocumentName(tabTitle)
        if (isMissingTab(tabTitle, newTab)) return
        workspaceWrapper.updateWrapperBasedOnTab(newTab.type)
        toolbarHiddenByScroll = false
        updateToolbarVisibility()
    }

    private fun isMissingTab(
        tabTitle: String?,
        newTab: WorkspaceTab,
    ): Boolean = tabTitle == null && newTab.type != WorkspaceTabType.Welcome

    private fun getCurrentFile(): File? {
        val currentTab = model.currentTab.value ?: return null
        return filesListModel.fileModel.idsAndFiles[currentTab.id]
    }

    private fun showTabPicker(workspaceWrapper: WorkspaceWrapperView) {
        if (tabsDialog != null) return
        val content = WorkspaceTabsDialogBinding.inflate(layoutInflater)
        ViewCompat.setAccessibilityPaneTitle(content.root, getString(R.string.workspace_switch_tabs))
        ViewCompat.setAccessibilityHeading(content.openTabsCount, true)
        content.openTabsCount.text = getString(R.string.workspace_open_tabs_count, model.tabs.size())
        val dialog = BottomSheetDialog(requireContext())
        tabsDialog = dialog
        tabsContent = content
        dialog.setContentView(content.root)
        dialog.setOnDismissListener {
            tabsContent?.tabsList?.adapter = null
            tabsContent = null
            tabsDialog = null
        }
        content.tabsList.layoutManager = LinearLayoutManager(requireContext())
        content.tabsList.setup {
            withDataSource(model.tabs)
            withItem<OpenTab, VerticalTabItemHolder>(R.layout.vertical_tab_item) {
                onBind(::VerticalTabItemHolder) { _, item ->
                    name.text = item.file.name
                    val selected = model.currentTab.value?.sessionId == item.sessionId
                    name.isChecked = selected
                    name.setIconResource(item.file.getIconResource())
                    closeButton.isChecked = selected
                    closeButton.setOnClickListener {
                        workspaceWrapper.workspaceView.closeSession(item.sessionId)
                    }
                    name.setOnClickListener {
                        workspaceWrapper.workspaceView.activateDoc(item.sessionId)
                    }
                }
            }
        }
        content.closeAllTabs.isEnabled = !model.tabs.isEmpty()
        content.closeAllTabs.setOnClickListener {
            dialog.dismiss()
            workspaceWrapper.workspaceView.closeAllTabs()
            workspaceWrapper.workspaceView.invalidate()
        }
        content.forwardWs.isVisible = workspaceWrapper.workspaceView.canForward()
        content.forwardWs.setOnClickListener {
            dialog.dismiss()
            workspaceWrapper.workspaceView.forward()
        }
        dialog.show()
        dialog.window?.apply {
            clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
            setDimAmount(0f)
        }
        val selected = model.tabs.indexOfFirst { it.sessionId == model.currentTab.value?.sessionId }
        if (selected >= 0) content.tabsList.scrollToPosition(selected)
    }

    @SuppressLint("NotifyDataSetChanged")
    private fun notifyTabListsChanged() {
        tabsContent?.let { content ->
            content.openTabsCount.text = getString(R.string.workspace_open_tabs_count, model.tabs.size())
            content.closeAllTabs.isEnabled = !model.tabs.isEmpty()
            content.forwardWs.isVisible = workspaceView?.canForward() == true
            content.tabsList.adapter?.notifyDataSetChanged()
        }
    }
}

@SuppressLint("ViewConstructor")
class WorkspaceWrapperView(
    context: Context,
    val model: WorkspaceViewModel,
) : FrameLayout(context) {
    val workspaceView: WorkspaceView
    private val markdownToolbar: MarkdownToolbarView
    var onVerticalScroll: ((Float) -> Unit)? = null

    private val toolbarScrollDetector =
        GestureDetector(
            context,
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onDown(event: MotionEvent): Boolean = true

                override fun onScroll(
                    first: MotionEvent?,
                    current: MotionEvent,
                    distanceX: Float,
                    distanceY: Float,
                ): Boolean {
                    if (current.pointerCount == 1 && abs(distanceY) > abs(distanceX)) {
                        onVerticalScroll?.invoke(distanceY)
                    }
                    return false
                }
            },
        ).apply { setIsLongpressEnabled(false) }

    override fun onInterceptTouchEvent(event: MotionEvent): Boolean {
        toolbarScrollDetector.onTouchEvent(event)
        return super.onInterceptTouchEvent(event)
    }

    var currentTab = WorkspaceTabType.Welcome

    var currentWrapper: View? = null

    private val regLayoutParams =
        ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT,
        )

    init {
        workspaceView = WorkspaceView(context, model)
        addView(workspaceView, regLayoutParams)
        markdownToolbar = MarkdownToolbarView(context, workspaceView)
        // Overlay the editor without changing its Rust viewport or scroll position.
        val markdownToolbarHeight = resources.getDimensionPixelSize(R.dimen.markdown_toolbar_height)
        addView(
            markdownToolbar,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                markdownToolbarHeight,
                android.view.Gravity.BOTTOM,
            ),
        )
        workspaceView.onMarkdownToolbarStateChanged = {
            markdownToolbar.refreshEditorState()
        }
    }

    fun setBottomInset(inset: Int) {
        val params = markdownToolbar.layoutParams as FrameLayout.LayoutParams
        if (params.bottomMargin != inset) {
            params.bottomMargin = inset
            markdownToolbar.layoutParams = params
        }
    }

    fun updateNativeToolbar() {
        val visible = shouldShowNativeToolbar()
        if (visible) markdownToolbar.refreshEditorState()
        if (visible && markdownToolbar.isVisible && markdownToolbar.translationY == 0f) return
        if (!visible && !markdownToolbar.isVisible) return
        markdownToolbar.animate().cancel()

        if (visible) {
            markdownToolbar.isVisible = true
            markdownToolbar.translationY = markdownToolbar.layoutParams.height.toFloat()
            markdownToolbar
                .animate()
                .translationY(0f)
                .setDuration(250)
                .setInterpolator(LinearOutSlowInInterpolator())
                .start()
        } else {
            markdownToolbar
                .animate()
                .translationY(markdownToolbar.layoutParams.height.toFloat())
                .setDuration(200)
                .setInterpolator(FastOutLinearInInterpolator())
                .withEndAction {
                    if (!shouldShowNativeToolbar()) {
                        markdownToolbar.isVisible = false
                        markdownToolbar.translationY = 0f
                    }
                }.start()
        }
    }

    private fun shouldShowNativeToolbar(): Boolean =
        model.keyboardVisible.value == true && model.outlineOpen.value != true &&
            workspaceView.canForwardTouches() &&
            model.currentTab.value?.type == WorkspaceTabType.Markdown &&
            Workspace.canEditMarkdown(WorkspaceView.wgpuObj)

    fun updateWrapperBasedOnTab(newTab: WorkspaceTabType) {
        if (newTab == currentTab) {
            return
        }

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
                200,
            )
        }
    }

    private fun attachWrapperIfNeeded(newTab: WorkspaceTabType) {
        if (!newTab.isTextEdit()) {
            return
        }

        val wrapper = WorkspaceTextInputWrapper(context, workspaceView)
        currentWrapper = wrapper
        workspaceView.wrapperView = wrapper
        addView(wrapper, FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT))
        markdownToolbar.bringToFront()
    }
}

@SuppressLint("ViewConstructor")
class WorkspaceTextInputWrapper(
    context: Context,
    val workspaceView: WorkspaceView,
) : View(context) {
    val wsInputConnection = WorkspaceTextInputConnection(workspaceView, this)

    var touchOffsetX = 0f
    var touchOffsetY = 0f

    fun applyTextInteractionRect(
        hasRect: Boolean,
        minX: Float,
        minY: Float,
        maxX: Float,
        maxY: Float,
        density: Float,
    ) {
        if (!hasRect) {
            return
        }

        val left = (minX * density).roundToInt()
        val top = (minY * density).roundToInt()
        val width = ((maxX - minX) * density).roundToInt().coerceAtLeast(1)
        val height = ((maxY - minY) * density).roundToInt().coerceAtLeast(1)

        val lp =
            (layoutParams as? FrameLayout.LayoutParams) ?: FrameLayout.LayoutParams(width, height)
        if (lp.leftMargin == left &&
            lp.topMargin == top &&
            lp.width == width &&
            lp.height == height
        ) {
            touchOffsetX = left.toFloat()
            touchOffsetY = top.toFloat()
            return
        }

        lp.leftMargin = left
        lp.topMargin = top
        lp.rightMargin = 0
        lp.bottomMargin = 0
        lp.width = width
        lp.height = height
        layoutParams = lp

        touchOffsetX = left.toFloat()
        touchOffsetY = top.toFloat()
    }

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
                val keyboardShown =
                    WindowInsetsCompat
                        .toWindowInsetsCompat(rootWindowInsets)
                        .isVisible(WindowInsetsCompat.Type.ime())

                val slopTouchThreshold = ViewConfiguration.get(context).scaledTouchSlop
                val nonSloppyTouch =
                    abs(event.x - touchStartX).toInt() < slopTouchThreshold &&
                        abs(event.y - touchStartY).toInt() < slopTouchThreshold
                if (!keyboardShown && duration < 300 && nonSloppyTouch &&
                    !workspaceView.willConsumeTouches(event.x + touchOffsetX, event.y + touchOffsetY)
                ) {
                    (context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                        .showSoftInput(this, InputMethodManager.SHOW_IMPLICIT)
                }
            }
        }

        if (event != null) {
            workspaceView.touchForwarder.forward(event, touchOffsetX, touchOffsetY)
        }

        return true
    }

    override fun onCheckIsTextEditor(): Boolean = true

    private fun shouldForwardToRust(event: KeyEvent): Boolean =
        when (event.keyCode) {
            KeyEvent.KEYCODE_BACK,
            KeyEvent.KEYCODE_HOME,
            KeyEvent.KEYCODE_APP_SWITCH,
            KeyEvent.KEYCODE_MENU,
            KeyEvent.KEYCODE_SEARCH,
            KeyEvent.KEYCODE_POWER,
            KeyEvent.KEYCODE_SLEEP,
            KeyEvent.KEYCODE_WAKEUP,
            KeyEvent.KEYCODE_VOLUME_UP,
            KeyEvent.KEYCODE_VOLUME_DOWN,
            KeyEvent.KEYCODE_VOLUME_MUTE,
            KeyEvent.KEYCODE_MUTE,
            KeyEvent.KEYCODE_BRIGHTNESS_UP,
            KeyEvent.KEYCODE_BRIGHTNESS_DOWN,
            KeyEvent.KEYCODE_CALL,
            KeyEvent.KEYCODE_ENDCALL,
            KeyEvent.KEYCODE_HEADSETHOOK,
            KeyEvent.KEYCODE_CAMERA,
            -> false

            else -> true
        }

    private fun forwardWorkspaceKeyEvent(event: KeyEvent): Boolean {
        if (!shouldForwardToRust(event)) {
            return false
        }

        wsInputConnection.forwardWorkspaceKeyEvent(event)
        workspaceView.drawImmediately()
        return true
    }

    override fun onKeyDown(
        keyCode: Int,
        event: KeyEvent,
    ): Boolean {
        if (forwardWorkspaceKeyEvent(event)) {
            return true
        }

        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(
        keyCode: Int,
        event: KeyEvent,
    ): Boolean {
        if (forwardWorkspaceKeyEvent(event)) {
            return true
        }

        return super.onKeyUp(keyCode, event)
    }

    override fun onKeyShortcut(
        keyCode: Int,
        event: KeyEvent,
    ): Boolean {
        if (forwardWorkspaceKeyEvent(event)) {
            return true
        }

        return super.onKeyShortcut(keyCode, event)
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo?): InputConnection {
        if (outAttrs != null) {
            outAttrs.initialCapsMode = wsInputConnection.getCursorCapsMode(EditorInfo.TYPE_CLASS_TEXT)
            outAttrs.hintText = "Type here"

            outAttrs.inputType =
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT or
                InputType.TYPE_TEXT_FLAG_CAP_SENTENCES

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

    private fun handleCommitContent(
        inputContentInfo: InputContentInfoCompat,
        flags: Int,
    ): Boolean {
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
            val bytes =
                try {
                    wsInputConnection.readAllBytesCapped(uri, MAX_ATTACHMENT_SIZE_BYTES)
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
                withContext(Dispatchers.Main) {
                    Workspace.clipboardSendImage(WorkspaceView.wgpuObj, bytes, true)
                    workspaceView.invalidate()
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
