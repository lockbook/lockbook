@file:Suppress("ktlint:standard:backing-property-naming")

package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.doOnLayout
import androidx.core.view.isVisible
import androidx.core.view.updatePadding
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.databinding.FragmentRecentFilesBinding
import app.lockbook.model.FileTreeViewModel
import app.lockbook.model.MainNavigationAction
import app.lockbook.model.MainScreenViewModel
import app.lockbook.util.FilesFragment
import kotlinx.coroutines.launch
import net.lockbook.File
import net.lockbook.Lb

class RecentFilesFragment :
    Fragment(),
    FilesFragment {
    private var _binding: FragmentRecentFilesBinding? = null
    private val binding get() = _binding!!

    private val fileTreeModel: FileTreeViewModel by activityViewModels()
    private val mainScreenModel: MainScreenViewModel by activityViewModels()
    private val selectionModel: FileSelectionViewModel by activityViewModels()
    private var fileActionDispatcher: FileSelectionActionDispatcher? = null
    private var originalListPaddingBottom = 0
    private val adapter by lazy {
        RecentFilesAdapter(
            onFileClick = ::onFileClicked,
            onFileLongClick = ::onFileLongClicked,
            currentUsername = runCatching { Lb.getAccount().username }.getOrNull(),
        )
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = FragmentRecentFilesBinding.inflate(inflater, container, false)

        binding.recentFilesList.layoutManager = LinearLayoutManager(requireContext())
        binding.recentFilesList.adapter = adapter
        binding.recentFilesList.itemAnimator = null
        originalListPaddingBottom = binding.recentFilesList.paddingBottom

        fileTreeModel.recentFiles.observe(viewLifecycleOwner) { files ->
            val recentFiles = files.orEmpty()
            selectionModel.reconcile(FileSelectionSource.Recents, recentFiles)
            adapter.submitRecentFiles(
                files = recentFiles,
                filesById = fileTreeModel.fileModel.idsAndFiles,
            )
            adapter.setSelectedFileIds(selectionModel.uiState.value.selectedIdsFor(FileSelectionSource.Recents))
            binding.recentFilesEmpty.isVisible = recentFiles.isEmpty()
        }

        return binding.root
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)
        fileActionDispatcher =
            FileSelectionActionDispatcher(
                fragment = this,
                mainScreenModel = mainScreenModel,
                fileTreeModel = fileTreeModel,
                snackbarAnchor = (requireActivity() as MainScreenActivity).fileActionSnackbarAnchorView,
                onClearSelection = ::unselectFiles,
            )
        viewLifecycleOwner.lifecycleScope.launch {
            viewLifecycleOwner.repeatOnLifecycle(Lifecycle.State.STARTED) {
                selectionModel.uiState.collect(::renderSelection)
            }
        }
    }

    override fun onDestroyView() {
        fileActionDispatcher = null
        binding.recentFilesList.adapter = null
        _binding = null
        super.onDestroyView()
    }

    private fun onFileClicked(file: File) {
        if (selectionModel.uiState.value.isActive) {
            selectionModel.toggle(FileSelectionSource.Recents, file)
        } else {
            mainScreenModel.navigate(MainNavigationAction.OpenDocument(file.id, newFile = true))
        }
    }

    private fun onFileLongClicked(file: File) {
        selectionModel.toggle(FileSelectionSource.Recents, file)
    }

    private fun renderSelection(state: FileSelectionUiState) {
        val selectedIds = state.selectedIdsFor(FileSelectionSource.Recents)
        adapter.setSelectedFileIds(selectedIds)
        if (selectedIds.isEmpty()) {
            binding.recentFilesList.updatePadding(bottom = originalListPaddingBottom)
        } else {
            (requireActivity() as MainScreenActivity).fileSelectionBottomBarView.doOnLayout { sheet ->
                binding.recentFilesList.updatePadding(bottom = originalListPaddingBottom + sheet.height)
            }
        }
    }

    internal fun dispatchSelectionAction(
        action: FileSelectionAction,
        files: List<File>,
    ) = fileActionDispatcher?.dispatch(action, files)

    override fun reloadFiles() {
        fileTreeModel.reloadFiles()
    }

    override fun unselectFiles() {
        selectionModel.clear()
    }

    override fun onNewFileCreated(newDocument: File?) {
        if (newDocument != null) reloadFiles()
    }

    override fun onBackPressed(): Boolean {
        if (
            selectionModel.uiState.value
                .selectedIdsFor(FileSelectionSource.Recents)
                .isEmpty()
        ) {
            return true
        }
        unselectFiles()
        return false
    }
}
