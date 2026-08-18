@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.screen

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.animation.AnimatorSet
import android.animation.ObjectAnimator
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.*
import android.widget.EditText
import androidx.annotation.StringRes
import androidx.core.content.ContextCompat
import androidx.core.view.doOnLayout
import androidx.core.view.updatePadding
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.interpolator.view.animation.FastOutSlowInInterpolator
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.FragmentFilesListBinding
import app.lockbook.model.*
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.snackbar.Snackbar
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference
import java.util.*

private enum class PinnedFileAction(
    @param:StringRes val titleRes: Int,
) {
    ChangeEmoji(R.string.change_emoji),
    RemoveEmoji(R.string.remove_emoji),
    Unpin(R.string.unpin),
    ShowInFolder(R.string.show_in_folder),
}

class FilesListFragment :
    Fragment(),
    FilesFragment {
    private var _binding: FragmentFilesListBinding? = null
    val binding get() = _binding!!
    private var fileActionDispatcher: FileSelectionActionDispatcher? = null
    private var originalListPaddingBottom = 0
    private var folderTransition: Animator? = null
    private var folderTransitionId = 0

    private var currentTab: WorkspaceTab = WorkspaceTab.welcome
    private val fileTreeAdapter by lazy {
        FileTreeAdapter(
            onItemClick = ::onFileItemClicked,
            onItemLongClick = ::onFileItemLongClicked,
        )
    }
    private val pinnedFilesAdapter by lazy {
        PinnedFilesAdapter(
            onItemClick = { enterFile(it.file) },
            onItemLongClick = ::showPinnedFileActions,
        )
    }

    private val mainScreenModel: MainScreenViewModel by activityViewModels()
    private val workspaceModel: WorkspaceViewModel by activityViewModels()
    private val selectionModel: FileSelectionViewModel by activityViewModels()

    private val model: FileTreeViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private val recyclerView get() = binding.filesList

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = FragmentFilesListBinding.inflate(inflater, container, false)
        originalListPaddingBottom = binding.filesList.paddingBottom
        model.notifyUpdateFilesUI.observe(
            viewLifecycleOwner,
        ) { uiUpdates ->
            updateUI(uiUpdates)
        }

        setUpFilesList()
        setUpPinnedFiles()
        observeFilesList()

        model.breadcrumbItems.observe(viewLifecycleOwner) {
            binding.filesBreadcrumbBar.setBreadCrumbItems(it)
        }

        binding.filesBreadcrumbBar.setListener(
            object : BreadCrumbItemClickListener {
                override fun onItemClick(
                    breadCrumbItem: View,
                    file: File,
                ) {
                    enterFolder(file)
                    unselectFiles()
                }
            },
        )

        binding.outOfSpace.apply {
            outOfSpaceMoreInfo.setOnClickListener {
                val intent = Intent(requireContext(), SettingsActivity::class.java)
                intent.putExtra(SettingsFragment.SCROLL_TO_PREFERENCE_KEY, R.string.usage_bar_key)
                startActivity(intent)
            }

            outOfSpaceUpgradeNow.setOnClickListener {
                val intent = Intent(requireContext(), SettingsActivity::class.java)
                intent.putExtra(SettingsFragment.UPGRADE_NOW, true)
                startActivity(intent)
            }
        }

        binding.listFilesRefresh.setOnRefreshListener {
            model._notifyUpdateFilesUI.postValue(UpdateFilesUI.RequestSync)
        }

        model.isSyncing.observe(viewLifecycleOwner) {
            if (!it) {
                binding.listFilesRefresh.isRefreshing = it
            }
        }

        workspaceModel.currentTab.observe(viewLifecycleOwner) {
            if (workspaceModel.isFileTreeSyncedToCurrentTab && currentTab != it) {
                model.fileModel.ownedParentOf(it.id)?.let { parent ->
                    enterFolder(parent, animate = false)
                }
            }

            currentTab = it
        }

        return binding.root
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)

        setUpFileActions()
        viewLifecycleOwner.lifecycleScope.launch {
            viewLifecycleOwner.repeatOnLifecycle(Lifecycle.State.STARTED) {
                selectionModel.uiState.collect(::renderSelection)
            }
        }
        (requireActivity().application as App).billingClientLifecycle.showInAppMessaging(requireActivity())
    }

    private fun setUpFileActions() {
        fileActionDispatcher =
            FileSelectionActionDispatcher(
                fragment = this,
                mainScreenModel = mainScreenModel,
                fileTreeModel = model,
                snackbarAnchor = (requireActivity() as MainScreenActivity).fileActionSnackbarAnchorView,
                onClearSelection = ::unselectFiles,
                onAddPinnedEmoji = { file -> showEmojiDialog(file.id, file.name, null) },
            )
    }

    private fun observeFilesList() {
        model.files.observe(viewLifecycleOwner) { files ->
            val currentFiles = files.orEmpty()
            selectionModel.reconcile(
                FileSelectionSource.Files,
                currentFiles.map { item -> item.fileMetadata },
            )
            fileTreeAdapter.submitList(currentFiles.toList()) {
                fileTreeAdapter.setSelectedFileIds(
                    selectionModel.uiState.value.selectedIdsFor(FileSelectionSource.Files),
                )
                binding.filesEmptyFolder.visibility = if (currentFiles.isEmpty()) View.VISIBLE else View.GONE
            }
        }
    }

    private fun setUpFilesList() {
        recyclerView.layoutManager = LinearLayoutManager(requireContext())
        recyclerView.adapter = fileTreeAdapter
        recyclerView.itemAnimator = null
    }

    private fun setUpPinnedFiles() {
        binding.pinnedFilesList.layoutManager = LinearLayoutManager(requireContext(), LinearLayoutManager.HORIZONTAL, false)
        binding.pinnedFilesList.adapter = pinnedFilesAdapter
        binding.pinnedFilesList.itemAnimator = null

        model.pinnedFiles.observe(viewLifecycleOwner) { pins ->
            val items =
                pins.mapNotNull { pin ->
                    model.fileModel.idsAndFiles[pin.id]?.let { file -> PinnedFileItem(pin, file) }
                }
            pinnedFilesAdapter.submitList(items)
            binding.pinnedFilesSection.visibility = if (items.isEmpty()) View.GONE else View.VISIBLE
        }
    }

    private fun showPinnedFileActions(item: PinnedFileItem) {
        val actions =
            buildList {
                add(PinnedFileAction.ChangeEmoji)
                if (item.pin.emoji != null) {
                    add(PinnedFileAction.RemoveEmoji)
                }
                add(PinnedFileAction.Unpin)
                add(PinnedFileAction.ShowInFolder)
            }

        MaterialAlertDialogBuilder(requireContext())
            .setTitle(item.file.name)
            .setItems(actions.map { getString(it.titleRes) }.toTypedArray()) { _, which ->
                when (actions[which]) {
                    PinnedFileAction.ChangeEmoji -> {
                        showEmojiDialog(item.pin.id, item.file.name, item.pin.emoji)
                    }

                    PinnedFileAction.RemoveEmoji -> {
                        model.setPinnedEmoji(item.pin.id, null)
                    }

                    PinnedFileAction.Unpin -> {
                        model.unpinFile(item.pin.id)
                        Snackbar
                            .make(binding.root, R.string.unpinned, Snackbar.LENGTH_SHORT)
                            .setAction(R.string.undo) { model.restorePinnedFile(item.pin) }
                            .show()
                    }

                    PinnedFileAction.ShowInFolder -> {
                        showPinnedFileInFolder(item.file)
                    }
                }
            }.show()
    }

    private fun showEmojiDialog(
        fileId: String,
        fileName: String,
        currentEmoji: String?,
    ) {
        val content = layoutInflater.inflate(R.layout.dialog_pick_pin_emoji, null)
        val emojiInput = content.findViewById<EditText>(R.id.pin_emoji_input)
        emojiInput.setText(currentEmoji)
        emojiInput.setSelection(emojiInput.text.length)

        MaterialAlertDialogBuilder(requireContext())
            .setTitle(R.string.choose_pin_emoji)
            .setMessage(fileName)
            .setView(content)
            .setNegativeButton(android.R.string.cancel, null)
            .setPositiveButton(android.R.string.ok) { _, _ ->
                model.setPinnedEmoji(fileId, emojiInput.text?.toString())
            }.show()
            .window
            ?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE)
    }

    private fun showPinnedFileInFolder(file: File) {
        val parent = model.fileModel.idsAndFiles[file.parent] ?: model.fileModel.root
        enterFolder(parent)
    }

    private fun onFileItemClicked(item: FileViewHolderInfo) {
        if (selectionModel.uiState.value.isActive) {
            selectionModel.toggle(FileSelectionSource.Files, item.fileMetadata)
        } else {
            enterFile(item.fileMetadata)
        }
    }

    private fun onFileItemLongClicked(item: FileViewHolderInfo) {
        selectionModel.toggle(FileSelectionSource.Files, item.fileMetadata)
    }

    private fun renderSelection(state: FileSelectionUiState) {
        val selectedIds = state.selectedIdsFor(FileSelectionSource.Files)
        fileTreeAdapter.setSelectedFileIds(selectedIds)
        if (selectedIds.isEmpty()) {
            binding.filesList.updatePadding(bottom = originalListPaddingBottom)
        } else {
            (requireActivity() as MainScreenActivity).fileSelectionBottomBarView.doOnLayout { sheet ->
                binding.filesList.updatePadding(bottom = originalListPaddingBottom + sheet.height)
            }
        }
    }

    internal fun dispatchSelectionAction(
        action: FileSelectionAction,
        files: List<File>,
    ) {
        fileActionDispatcher?.dispatch(action, files)
    }

    private fun enterFile(item: File) {
        when (item.type) {
            FileType.Document -> {
                mainScreenModel.navigate(MainNavigationAction.OpenDocument(item.id, newFile = false))
            }

            FileType.Folder -> {
                enterFolder(item)
            }

            FileType.Link -> {} // shouldn't happen
        }
    }

    private fun enterParent() {
        val parent = model.fileModel.idsAndFiles[model.fileModel.parent.parent] ?: model.fileModel.root
        enterFolder(parent)
    }

    private fun enterFolder(
        newParent: File,
        animate: Boolean = true,
    ) {
        val transitionId = ++folderTransitionId

        folderTransition?.cancel()

        val transitionViews = listOf(recyclerView, binding.filesEmptyFolder)
        if (!animate) {
            transitionViews.forEach {
                it.alpha = 1f
                it.translationX = 0f
            }
            folderTransition = null
            model.enterFolder(newParent)
            return
        }

        val isMovingUpTree =
            model.fileModel
                .getFileDir()
                .dropLast(1)
                .any { it.id == newParent.id }
        val translationDistance = resources.displayMetrics.density * FOLDER_TRANSITION_TRANSLATION_DP
        val outgoingTranslation = if (isMovingUpTree) translationDistance else -translationDistance
        val incomingTranslation = -outgoingTranslation

        val fadeOut =
            AnimatorSet().apply {
                playTogether(
                    transitionViews.flatMap { view ->
                        listOf(
                            ObjectAnimator.ofFloat(view, View.ALPHA, view.alpha, 0f),
                            ObjectAnimator.ofFloat(view, View.TRANSLATION_X, view.translationX, outgoingTranslation),
                        )
                    },
                )
                duration = FOLDER_TRANSITION_FADE_OUT_DURATION
                interpolator = FastOutSlowInInterpolator()
            }

        val fadeIn =
            AnimatorSet().apply {
                playTogether(
                    transitionViews.flatMap { view ->
                        listOf(
                            ObjectAnimator.ofFloat(view, View.ALPHA, 0f, 1f),
                            ObjectAnimator.ofFloat(view, View.TRANSLATION_X, incomingTranslation, 0f),
                        )
                    },
                )
                duration = FOLDER_TRANSITION_FADE_IN_DURATION
                interpolator = FastOutSlowInInterpolator()
            }

        fadeOut.addListener(
            object : AnimatorListenerAdapter() {
                override fun onAnimationEnd(animation: Animator) {
                    if (transitionId == folderTransitionId) {
                        transitionViews.forEach {
                            it.alpha = 0f
                            it.translationX = incomingTranslation
                        }
                        model.enterFolder(newParent)
                    }
                }
            },
        )

        folderTransition =
            AnimatorSet().apply {
                playSequentially(fadeOut, fadeIn)
                addListener(
                    object : AnimatorListenerAdapter() {
                        override fun onAnimationEnd(animation: Animator) {
                            if (transitionId == folderTransitionId) {
                                transitionViews.forEach {
                                    it.alpha = 1f
                                    it.translationX = 0f
                                }
                                folderTransition = null
                            }
                        }

                        override fun onAnimationCancel(animation: Animator) {
                            if (transitionId == folderTransitionId) {
                                transitionViews.forEach {
                                    it.alpha = 1f
                                    it.translationX = 0f
                                }
                            }
                        }
                    },
                )
                start()
            }
    }

    private fun updateUI(uiUpdates: UpdateFilesUI) {
        when (uiUpdates) {
            is UpdateFilesUI.NotifyError -> {
                if (binding.listFilesRefresh.isRefreshing) {
                    binding.listFilesRefresh.isRefreshing = false
                }

                alertModel.notifyError(uiUpdates.error)
            }

            is UpdateFilesUI.NotifyWithSnackbar -> {
                if (binding.listFilesRefresh.isRefreshing) {
                    binding.listFilesRefresh.isRefreshing = false
                }

                alertModel.notify(uiUpdates.msg)
            }

            is UpdateFilesUI.UpdateBreadcrumbBar -> {
                model._breadcrumbItems.value = getBreadcrumbItems()
            }

            UpdateFilesUI.RequestSync -> {
                lifecycleScope.launch(Dispatchers.IO) {
                    try {
                        Lb.sync()
                    } catch (err: LbError) {
                        alertModel.notifyError(err)
                    }
                }
            }

            UpdateFilesUI.SyncImport -> {
                (activity as MainScreenActivity).syncImportAccount()
            }

            is UpdateFilesUI.OutOfSpace -> {
                val usageRatio = uiUpdates.progress.toFloat() / uiUpdates.max

                val (usageBarColor, msgId) =
                    if (usageRatio >= 1.0) {
                        listOf(getUsageColor(usageRatio), R.string.out_of_space)
                    } else {
                        listOf(getUsageColor(usageRatio), R.string.running_out_of_space)
                    }

                binding.outOfSpace.apply {
                    outOfSpaceMsg.setText(msgId)
                    outOfSpaceProgressBar.setIndicatorColor(ContextCompat.getColor(requireContext(), usageBarColor))
                    outOfSpaceProgressBar.progress = uiUpdates.progress
                    outOfSpaceProgressBar.max = uiUpdates.max
                    Animate.animateVisibility(root, View.VISIBLE, 255, 200)

                    outOfSpaceExit.setOnClickListener {
                        Animate.animateVisibility(root, View.GONE, 0, 200)

                        val pref =
                            PreferenceManager
                                .getDefaultSharedPreferences(requireContext())
                                .edit()

                        if (usageRatio > 0.9 && usageRatio < 1.0) {
                            pref.putBoolean(getString(R.string.show_running_out_of_space_0_9_key), false)
                            pref.apply()
                        } else if (usageRatio > 0.8 && usageRatio <= 0.9) {
                            pref.putBoolean(getString(R.string.show_running_out_of_space_0_8_key), false)
                            pref.apply()
                        }
                    }
                }
            }
        }
    }

    private fun getUsageColor(usageRatio: Float): Int =
        when {
            usageRatio >= 1.0 -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM) {
                    android.R.color.system_error_500
                } else {
                    R.color.md_theme_error
                }
            }

            usageRatio > 0.9 -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM) {
                    android.R.color.system_error_200
                } else {
                    R.color.md_theme_error
                }
            }

            else -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    android.R.color.system_accent1_100
                } else {
                    R.color.md_theme_primary
                }
            }
        }

    private fun getBreadcrumbItems(): MutableList<BreadCrumbItem> =
        model.fileModel
            .getFileDir()
            .map { BreadCrumbItem(it) }
            .toMutableList()

    override fun onBackPressed(): Boolean =
        when {
            selectionModel.uiState.value
                .selectedIdsFor(FileSelectionSource.Files)
                .isNotEmpty() -> {
                unselectFiles()
                false
            }

            !model.fileModel.isAtRoot() -> {
                enterParent()
                false
            }

            else -> {
                true
            }
        }

    override fun reloadFiles() {
        model.reloadFiles()
    }

    override fun unselectFiles() {
        selectionModel.clear()
    }

    override fun onNewFileCreated(newDocument: File?) {
        when {
            newDocument != null &&
                PreferenceManager
                    .getDefaultSharedPreferences(requireContext())
                    .getBoolean(getString(R.string.open_new_doc_automatically_key), true)
            -> {
                model.reloadFiles()
                enterFile(newDocument)
            }

            newDocument != null -> {
                model.reloadFiles()
            }
        }
    }

    override fun onDestroyView() {
        fileActionDispatcher = null
        binding.filesList.adapter = null
        binding.pinnedFilesList.adapter = null
        _binding = null
        super.onDestroyView()
    }
}

private const val FOLDER_TRANSITION_FADE_OUT_DURATION = 70L
private const val FOLDER_TRANSITION_FADE_IN_DURATION = 160L
private const val FOLDER_TRANSITION_TRANSLATION_DP = 24f

sealed class UpdateFilesUI {
    object UpdateBreadcrumbBar : UpdateFilesUI()

    data class NotifyError(
        val error: LbError,
    ) : UpdateFilesUI()

    object RequestSync : UpdateFilesUI()

    object SyncImport : UpdateFilesUI()

    data class OutOfSpace(
        val progress: Int,
        val max: Int,
    ) : UpdateFilesUI()

    data class NotifyWithSnackbar(
        val msg: String,
    ) : UpdateFilesUI()
}
