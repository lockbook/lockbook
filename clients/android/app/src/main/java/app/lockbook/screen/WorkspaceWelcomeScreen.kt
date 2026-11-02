package app.lockbook.screen

import android.content.Context
import android.graphics.Paint
import android.os.Build
import android.os.Bundle
import android.view.HapticFeedbackConstants
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.widget.Toast
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.widget.PopupMenu
import androidx.core.graphics.ColorUtils
import androidx.core.view.isVisible
import androidx.core.view.updatePadding
import androidx.core.widget.NestedScrollView
import androidx.core.widget.doAfterTextChanged
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.StaggeredGridLayoutManager
import app.lockbook.R
import app.lockbook.databinding.WorkspaceEmptyBinding
import app.lockbook.model.MainNavigationAction
import app.lockbook.model.MainScreenViewModel
import app.lockbook.model.QuickCaptureViewModel
import app.lockbook.model.WorkspaceDocumentsViewModel
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceTabType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.ui.CreateFileBottomSheetFragment
import app.lockbook.util.DocumentCoverStore
import app.lockbook.util.WorkspaceView
import app.lockbook.workspace.Workspace
import com.google.android.material.color.MaterialColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File

/** Owns the Kotlin welcome screen and its handoff to the native document editor. */
internal class WorkspaceWelcomeScreen(
    private val fragment: Fragment,
    private val onWorkspaceScroll: (Float) -> Unit,
    private val hideEditorKeyboard: () -> Unit,
) {
    private val quickCapture: QuickCaptureViewModel by fragment.viewModels()
    private val workspaceDocuments: WorkspaceDocumentsViewModel by fragment.viewModels()
    private val model: WorkspaceViewModel by fragment.activityViewModels()
    private val mainScreenModel: MainScreenViewModel by fragment.activityViewModels()

    private var viewBinding: WorkspaceEmptyBinding? = null
    private val binding get() = viewBinding!!
    private var workspaceRoot: ViewGroup? = null
    private var captureExpansion: QuickCaptureExpansion? = null
    private var captureStart: Runnable? = null
    private var captureEditorId: String? = null
    private var coverFileId: String? = null
    private var documentsAdapter: WorkspaceDocumentsAdapter? = null
    private val documentCovers by lazy { DocumentCoverStore(fragment.requireContext().applicationContext) }

    val isExpanding: Boolean get() = captureExpansion != null
    val isVisible: Boolean
        get() =
            model.currentTab.value
                ?.type
                .let { it == null || it == WorkspaceTabType.Welcome }

    private val chooseCover =
        fragment.registerForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
            val id = coverFileId
            coverFileId = null
            if (uri != null && id != null) {
                fragment.viewLifecycleOwner.lifecycleScope.launch {
                    val saved = withContext(Dispatchers.IO) { runCatching { documentCovers.save(id, uri) }.isSuccess }
                    if (saved) documentsAdapter?.refreshCover(id) else toast(R.string.workspace_cover_error)
                }
            }
        }

    fun bind(
        root: ViewGroup,
        welcomeBinding: WorkspaceEmptyBinding,
    ) {
        workspaceRoot = root
        viewBinding = welcomeBinding
        val empty = binding
        empty.captureFolder.paintFlags = empty.captureFolder.paintFlags or Paint.UNDERLINE_TEXT_FLAG
        fragment.childFragmentManager.setFragmentResultListener(QUICK_CAPTURE_FOLDER, fragment.viewLifecycleOwner) { _, result ->
            result.getString(CreateFileBottomSheetFragment.SELECTED_FOLDER_ID)?.let(quickCapture::selectSaveFolder)
        }
        empty.captureFolder.setOnClickListener {
            if (captureExpansion == null && !quickCapture.isSaving &&
                fragment.childFragmentManager.findFragmentByTag(QUICK_CAPTURE_FOLDER) == null
            ) {
                hideEditorKeyboard()
                CreateFileBottomSheetFragment
                    .folderPicker(quickCapture.saveFolder.value?.id, QUICK_CAPTURE_FOLDER)
                    .show(fragment.childFragmentManager, QUICK_CAPTURE_FOLDER)
            }
        }
        quickCapture.saveFolder.observe(fragment.viewLifecycleOwner) { folder ->
            val name = if (folder == null || folder.isRoot) fragment.getString(R.string.home) else folder.name
            empty.captureFolder.text = name
            empty.captureFolder.contentDescription = fragment.getString(R.string.quick_capture_change_folder, name)
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
        quickCapture.draft.observe(fragment.viewLifecycleOwner) { text ->
            if (empty.captureText.text.toString() != text) empty.captureText.setText(text)
            if (text.isNotEmpty() || !quickCapture.isSaving) captureExpansion?.updateText(text)
            scheduleCaptureExpansion(text)
        }
        quickCapture.error.observe(fragment.viewLifecycleOwner) { error ->
            Toast.makeText(fragment.requireContext(), error, Toast.LENGTH_LONG).show()
            captureExpansion?.cancel()
            captureExpansion = null
            empty.captureText.isEnabled = true
        }
        quickCapture.createdNote.observe(fragment.viewLifecycleOwner) { id ->
            captureEditorId = id
            binding.captureText.performHapticFeedback(
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    HapticFeedbackConstants.CONFIRM
                } else {
                    HapticFeedbackConstants.KEYBOARD_TAP
                },
            )
            mainScreenModel.navigate(MainNavigationAction.OpenDocument(id, newFile = false, closeDetailOnBack = false))
        }
        val adapter =
            WorkspaceDocumentsAdapter(
                scope = fragment.viewLifecycleOwner.lifecycleScope,
                previews = workspaceDocuments.previews,
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
            val columns = (view.width / (180 * fragment.resources.displayMetrics.density)).toInt().coerceIn(2, 4)
            if (grid.spanCount != columns) grid.spanCount = columns
        }
        workspaceDocuments.documents.observe(fragment.viewLifecycleOwner) { items ->
            empty.documentSection.isVisible = items.isNotEmpty()
            adapter.submitList(items)
        }
        model.bottomInset.observe(fragment.viewLifecycleOwner) { inset ->
            empty.root.updatePadding(bottom = inset)
        }
    }

    private fun scheduleCaptureExpansion(text: String) {
        val input = binding.captureText
        captureStart?.let(input::removeCallbacks)
        captureStart = null
        if (text.isBlank() || captureExpansion != null || quickCapture.isSaving || !isVisible) return
        captureStart =
            Runnable {
                captureStart = null
                if (viewBinding == null || workspaceRoot?.isShown != true ||
                    !isVisible || quickCapture.isSaving
                ) {
                    return@Runnable
                }
                val expansion = QuickCaptureExpansion(requireNotNull(workspaceRoot), binding.captureSurface, input)
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

    fun finishCaptureHandoff(
        wrapper: WorkspaceWrapperView,
        tab: WorkspaceTab,
    ) {
        if (captureEditorId != tab.id || tab.type != WorkspaceTabType.Markdown) return
        val editor = wrapper.currentWrapper ?: return
        val input = binding.captureText
        val keepKeyboard = input.hasFocus() && model.keyboardVisible.value == true
        input.clearComposingText()
        val pendingText = quickCapture.draft.value.orEmpty()
        val length = Workspace.getTextLength(WorkspaceView.wgpuObj)
        Workspace.setSelection(WorkspaceView.wgpuObj, length, length)
        if (pendingText.isNotEmpty()) Workspace.insert(WorkspaceView.wgpuObj, length, pendingText)

        // Transfer the IME connection before hiding its old editor, so the keyboard stays open.
        editor.requestFocus()
        captureEditorId = null
        binding.root.isVisible = false
        if (pendingText.isNotEmpty()) quickCapture.setDraft("")
        val keyboard = fragment.requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
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
        PopupMenu(fragment.requireContext(), anchor).apply {
            menu.add(0, 1, 0, R.string.workspace_choose_cover)
            if (documentCovers.file(file.id).exists()) menu.add(0, 2, 1, R.string.workspace_remove_cover)
            setOnMenuItemClickListener { item ->
                if (item.itemId == 1) {
                    coverFileId = file.id
                    chooseCover.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
                } else {
                    fragment.viewLifecycleOwner.lifecycleScope.launch {
                        val removed = withContext(Dispatchers.IO) { documentCovers.file(file.id).delete() }
                        if (removed) documentsAdapter?.refreshCover(file.id) else toast(R.string.workspace_cover_error)
                    }
                }
                true
            }
            show()
        }
    }

    fun restoreState(savedState: Bundle?) {
        coverFileId = savedState?.getString("coverFileId")
        captureEditorId = savedState?.getString("captureEditorId")
    }

    fun saveState(outState: Bundle) {
        coverFileId?.let { outState.putString("coverFileId", it) }
        captureEditorId?.let { outState.putString("captureEditorId", it) }
    }

    fun updateVisibility() {
        binding.root.isVisible = isVisible || isExpanding
        if (isVisible) {
            binding.captureText.isEnabled = true
        } else if (!isExpanding) {
            binding.captureText.clearFocus()
        }
    }

    fun unbind() {
        captureStart?.let { binding.captureText.removeCallbacks(it) }
        captureStart = null
        captureExpansion?.cancel()
        captureExpansion = null
        binding.documentGrid.adapter = null
        documentsAdapter = null
        workspaceRoot = null
        viewBinding = null
    }

    private fun toast(id: Int) {
        fragment.context?.let { Toast.makeText(it, id, Toast.LENGTH_SHORT).show() }
    }

    companion object {
        private const val QUICK_CAPTURE_FOLDER = "quick_capture_folder"
    }
}
