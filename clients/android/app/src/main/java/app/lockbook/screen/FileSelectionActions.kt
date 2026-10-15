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
import androidx.fragment.app.activityViewModels
import androidx.interpolator.view.animation.FastOutSlowInInterpolator
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.FileSelectionActionButtonBinding
import app.lockbook.databinding.FileSelectionActionRowBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.FileOperationsViewModel
import app.lockbook.model.FileTreeViewModel
import app.lockbook.model.MainNavigationAction
import app.lockbook.model.MainScreenViewModel
import app.lockbook.model.TransientScreen
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.button.MaterialButton
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
    Share(R.string.menu_list_files_share, R.drawable.ic_baseline_group_24, singleSelectionOnly = true),
    Export(R.string.export, R.drawable.ic_baseline_share_24),
    Info(R.string.menu_list_files_info, R.drawable.ic_baseline_info_24, singleSelectionOnly = true),
    Delete(R.string.menu_list_files_delete, R.drawable.ic_outline_delete_24, destructive = true),
    ;

    fun isAvailableFor(files: List<File>): Boolean =
        files.isNotEmpty() &&
            (!singleSelectionOnly || files.size == 1) &&
            (!documentOnly || files.any { it.type == File.FileType.Document })
}

/** The shared compact/expanded action surface used by home selection and workspace documents. */
internal class FileActionsSheetContent(
    private val root: View,
    private val onAction: (FileSelectionAction, List<File>) -> Unit,
    onDismiss: () -> Unit,
) {
    private val title = root.findViewById<android.widget.TextView>(R.id.file_selection_count)
    private val header = root.findViewById<View>(R.id.file_selection_header)
    private val content = root.findViewById<View>(R.id.file_selection_content)
    private val compact = root.findViewById<View>(R.id.file_selection_compact_actions)
    private val expanded = root.findViewById<View>(R.id.file_selection_expanded_scroll)
    private val compactActions = root.findViewById<ViewGroup>(R.id.file_selection_actions)
    private val expandedActions = root.findViewById<ViewGroup>(R.id.file_selection_expanded_actions)
    private var files = emptyList<File>()
    private val buttons =
        FileSelectionAction.entries.associateWith { action ->
            listOf(addAction(action, compactActions, false), addAction(action, expandedActions, true))
        }

    init {
        root.findViewById<View>(R.id.clear_file_selection).setOnClickListener { onDismiss() }
    }

    fun render(
        files: List<File>,
        title: String,
    ) {
        this.files = files
        this.title.text = title
        buttons.forEach { (action, views) ->
            views.forEach { it.isVisible = action.isAvailableFor(files) }
        }
    }

    fun attachBehavior(behavior: BottomSheetBehavior<out View>) {
        behavior.isFitToContents = true
        behavior.skipCollapsed = false
        // Keep long action lists scrollable on small screens and in landscape.
        root.rootView.doOnLayout { window ->
            behavior.maxHeight = (window.height * 0.9f).toInt()
            root.requestLayout()
        }
        root.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            val peekHeight =
                header.height + compact.height + content.paddingTop +
                    content.paddingBottom + root.paddingTop + root.paddingBottom
            if (peekHeight > 0 && behavior.peekHeight != peekHeight) behavior.peekHeight = peekHeight
        }
        behavior.addBottomSheetCallback(
            object : BottomSheetBehavior.BottomSheetCallback() {
                override fun onStateChanged(
                    bottomSheet: View,
                    newState: Int,
                ) {
                    if (newState == BottomSheetBehavior.STATE_COLLAPSED) setExpansion(0f)
                    if (newState == BottomSheetBehavior.STATE_EXPANDED) setExpansion(1f)
                }

                override fun onSlide(
                    bottomSheet: View,
                    slideOffset: Float,
                ) {
                    setExpansion(slideOffset.coerceIn(0f, 1f))
                }
            },
        )
        setExpansion(if (behavior.state == BottomSheetBehavior.STATE_EXPANDED) 1f else 0f)
    }

    private fun setExpansion(fraction: Float) {
        compact.alpha = 1f - fraction
        expanded.alpha = fraction
        // INVISIBLE preserves the expanded sheet's measured height while it is collapsed.
        compact.visibility = if (fraction < 1f) View.VISIBLE else View.INVISIBLE
        expanded.visibility = if (fraction > 0f) View.VISIBLE else View.INVISIBLE
        compact.importantForAccessibility =
            if (fraction < 0.5f) {
                View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
            } else {
                View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            }
        expanded.importantForAccessibility =
            if (fraction >= 0.5f) {
                View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
            } else {
                View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            }
    }

    private fun addAction(
        action: FileSelectionAction,
        parent: ViewGroup,
        vertical: Boolean,
    ): MaterialButton {
        val inflater = LayoutInflater.from(root.context)
        val button =
            if (vertical) {
                FileSelectionActionRowBinding.inflate(inflater, parent, false).root
            } else {
                FileSelectionActionButtonBinding.inflate(inflater, parent, false).root
            }
        button.apply {
            setText(action.titleRes)
            setIconResource(action.iconRes)
            if (action.destructive) {
                val error = MaterialColors.getColor(this, androidx.appcompat.R.attr.colorError)
                setTextColor(error)
                iconTint = ColorStateList.valueOf(error)
            }
            setOnClickListener { onAction(action, files) }
        }
        parent.addView(button)
        return button
    }
}

internal class FileSelectionBottomBarController(
    private val root: View,
    onAction: (FileSelectionAction, List<File>) -> Unit,
    onClearSelection: () -> Unit,
    private val onVisibilityChanged: (Boolean) -> Unit = {},
) {
    private val behavior = BottomSheetBehavior.from(root)
    private val content = FileActionsSheetContent(root, onAction, onClearSelection)
    private var renderedState = FileSelectionUiState()

    init {
        root.isVisible = false
        behavior.isHideable = false
        content.attachBehavior(behavior)
    }

    fun render(state: FileSelectionUiState) {
        if (state.isActive) {
            content.render(state.selectedFiles, root.context.getString(R.string.files_list_items_selected, state.selectedCount))
        }
        val visibilityChanged = state.isActive != renderedState.isActive
        renderedState = state
        if (visibilityChanged) setSheetVisible(state.isActive)
    }

    fun collapseIfExpanded(): Boolean {
        if (!renderedState.isActive || behavior.state == BottomSheetBehavior.STATE_COLLAPSED) return false
        behavior.state = BottomSheetBehavior.STATE_COLLAPSED
        return true
    }

    private fun setSheetVisible(isVisible: Boolean) {
        root.animate().cancel()
        if (isVisible) showSheet() else hideSheet()
        onVisibilityChanged(isVisible)
    }

    private fun showSheet() {
        val startOffscreen = !root.isVisible
        root.isVisible = true
        behavior.state = BottomSheetBehavior.STATE_COLLAPSED
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
                    behavior.state = BottomSheetBehavior.STATE_COLLAPSED
                }
            }.start()
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
    private val fileOperations: FileOperationsViewModel by fragment.activityViewModels()

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

            FileSelectionAction.Share -> {
                files.singleOrNull()?.let { mainScreenModel.launchTransientScreen(TransientScreen.Share(it)) }
                onClearSelection()
            }

            FileSelectionAction.Export -> {
                fileOperations.requestExport(files)
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
