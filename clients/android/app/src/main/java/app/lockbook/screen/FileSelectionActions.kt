package app.lockbook.screen

import android.content.res.ColorStateList
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import androidx.core.view.doOnLayout
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.interpolator.view.animation.FastOutSlowInInterpolator
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.FileSelectionActionButtonBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.FileTreeViewModel
import app.lockbook.model.MainNavigationAction
import app.lockbook.model.MainScreenViewModel
import app.lockbook.model.TransientScreen
import com.google.android.material.color.MaterialColors
import com.google.android.material.snackbar.Snackbar
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

internal enum class FileSelectionAction(
    @param:StringRes val titleRes: Int,
    @param:DrawableRes val iconRes: Int,
    val singleSelectionOnly: Boolean = false,
    val documentOnly: Boolean = false,
    val destructive: Boolean = false,
) {
    Rename(R.string.menu_list_files_rename, R.drawable.ic_baseline_text_format_24, singleSelectionOnly = true),
    OpenInNewTab(
        R.string.menu_list_files_open_in_new_tab,
        R.drawable.ic_outline_tab_24,
        documentOnly = true,
    ),
    Duplicate(
        R.string.duplicate,
        R.drawable.baseline_content_copy_24,
        singleSelectionOnly = true,
        documentOnly = true,
    ),
    Move(R.string.menu_list_files_move, R.drawable.ic_baseline_content_cut_24),
    Pin(R.string.pin, R.drawable.ic_outline_push_pin_24),
    Share(R.string.menu_list_files_share, R.drawable.ic_outline_folder_shared_24, singleSelectionOnly = true),
    Export(R.string.export, R.drawable.ic_baseline_share_24),
    Info(R.string.menu_list_files_info, R.drawable.ic_baseline_info_24, singleSelectionOnly = true),
    Delete(R.string.menu_list_files_delete, R.drawable.ic_outline_delete_24, destructive = true),
    ;

    fun isAvailableFor(files: List<File>): Boolean =
        files.isNotEmpty() &&
            (!singleSelectionOnly || files.size == 1) &&
            (!documentOnly || files.any { it.type == File.FileType.Document })
}

internal class FileSelectionBottomBarController(
    private val root: View,
    private val onAction: (FileSelectionAction, List<File>) -> Unit,
    private val onClearSelection: () -> Unit,
    private val onVisibilityChanged: (Boolean) -> Unit = {},
) {
    private val count = root.findViewById<android.widget.TextView>(R.id.file_selection_count)
    private val actions = root.findViewById<ViewGroup>(R.id.file_selection_actions)
    private val actionButtons = FileSelectionAction.entries.associateWith(::addAction)
    private var renderedState = FileSelectionUiState()

    init {
        root.isVisible = false
        root.findViewById<View>(R.id.clear_file_selection).setOnClickListener { onClearSelection() }
    }

    fun render(state: FileSelectionUiState) {
        if (state.isActive) {
            count.text = root.context.getString(R.string.files_list_items_selected, state.selectedCount)
            actionButtons.forEach { (action, button) ->
                button.isVisible = action in state.visibleActions
            }
        }

        val visibilityChanged = state.isActive != renderedState.isActive
        renderedState = state
        if (visibilityChanged) setSheetVisible(state.isActive)
    }

    private fun setSheetVisible(isVisible: Boolean) {
        root.animate().cancel()
        if (isVisible) showSheet() else hideSheet()
        onVisibilityChanged(isVisible)
    }

    private fun showSheet() {
        val startOffscreen = !root.isVisible
        root.isVisible = true
        root.doOnLayout {
            if (!renderedState.isActive) return@doOnLayout
            if (startOffscreen) root.translationY = root.height.toFloat()
            root
                .animate()
                .translationY(0f)
                .setDuration(SHEET_ENTER_DURATION_MS)
                .setInterpolator(FastOutSlowInInterpolator())
                .start()
        }
    }

    private fun hideSheet() {
        root
            .animate()
            .translationY(root.height.toFloat())
            .setDuration(SHEET_EXIT_DURATION_MS)
            .setInterpolator(FastOutSlowInInterpolator())
            .withEndAction {
                if (!renderedState.isActive) {
                    root.isVisible = false
                    root.translationY = 0f
                }
            }.start()
    }

    private fun addAction(action: FileSelectionAction): View {
        val binding =
            FileSelectionActionButtonBinding.inflate(
                LayoutInflater.from(root.context),
                actions,
                false,
            )
        binding.root.apply {
            setText(action.titleRes)
            setIconResource(action.iconRes)
            if (action.destructive) {
                val error = MaterialColors.getColor(this, androidx.appcompat.R.attr.colorError)
                setTextColor(error)
                iconTint = ColorStateList.valueOf(error)
            }
            setOnClickListener { onAction(action, renderedState.selectedFiles) }
        }
        actions.addView(binding.root)
        return binding.root
    }
}

private const val SHEET_ENTER_DURATION_MS = 220L
private const val SHEET_EXIT_DURATION_MS = 180L

internal class FileSelectionActionDispatcher(
    private val fragment: Fragment,
    private val mainScreenModel: MainScreenViewModel,
    private val fileTreeModel: FileTreeViewModel,
    private val snackbarAnchor: View,
    private val onClearSelection: () -> Unit,
    private val onAddPinnedEmoji: ((File) -> Unit)? = null,
) {
    private val alertModel = AlertModel(WeakReference(fragment.requireActivity()), snackbarAnchor)

    fun dispatch(
        action: FileSelectionAction,
        files: List<File>,
    ) {
        if (files.isEmpty()) return
        when (action) {
            FileSelectionAction.Pin -> {
                pin(files)
            }

            FileSelectionAction.Info -> {
                files.singleOrNull()?.let { mainScreenModel.launchTransientScreen(TransientScreen.Info(it)) }
            }

            FileSelectionAction.Rename -> {
                files.singleOrNull()?.let { mainScreenModel.launchTransientScreen(TransientScreen.Rename(it)) }
            }

            FileSelectionAction.OpenInNewTab -> {
                files
                    .filter { it.type == File.FileType.Document }
                    .forEach { file ->
                        mainScreenModel.navigate(MainNavigationAction.OpenDocument(file.id, newFile = true))
                    }
                onClearSelection()
            }

            FileSelectionAction.Duplicate -> {
                files.singleOrNull()?.let(::duplicate)
            }

            FileSelectionAction.Move -> {
                mainScreenModel.launchTransientScreen(TransientScreen.Move(files))
            }

            FileSelectionAction.Delete -> {
                mainScreenModel.launchTransientScreen(TransientScreen.Delete(files))
            }

            FileSelectionAction.Export -> {
                mainScreenModel.shareSelectedFiles(files, fragment.requireContext().cacheDir)
                onClearSelection()
            }

            FileSelectionAction.Share -> {
                files.singleOrNull()?.let { mainScreenModel.launchTransientScreen(TransientScreen.Share(it)) }
                onClearSelection()
            }
        }
    }

    private fun pin(files: List<File>) {
        val newlyPinned = fileTreeModel.pinFiles(files)
        val message = if (newlyPinned.isEmpty()) R.string.already_pinned else R.string.pinned
        val snackbar = makeSnackbar(message)
        if (newlyPinned.size == 1 && onAddPinnedEmoji != null) {
            files.firstOrNull { it.id == newlyPinned.single().id }?.let { file ->
                snackbar.setAction(R.string.add_emoji) { onAddPinnedEmoji.invoke(file) }
            }
        }
        snackbar.show()
        onClearSelection()
    }

    private fun duplicate(file: File) {
        fragment.viewLifecycleOwner.lifecycleScope.launch(Dispatchers.IO) {
            try {
                Lb.duplicateFile(file.id)
                withContext(Dispatchers.Main) {
                    fileTreeModel.reloadFiles()
                    makeSnackbar(R.string.duplicated).show()
                    onClearSelection()
                }
            } catch (error: LbError) {
                alertModel.notifyError(error)
            }
        }
    }

    private fun makeSnackbar(
        @StringRes messageRes: Int,
    ): Snackbar =
        Snackbar
            .make(snackbarAnchor, messageRes, Snackbar.LENGTH_SHORT)
            .setAnchorView(snackbarAnchor)
}
