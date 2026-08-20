@file:Suppress("ktlint:standard:backing-property-naming")

package app.lockbook.ui

import android.content.Context
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import androidx.activity.OnBackPressedCallback
import androidx.core.os.bundleOf
import androidx.core.view.children
import androidx.core.view.isVisible
import androidx.core.widget.addTextChangedListener
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.SheetCreateFileBinding
import app.lockbook.model.CreateFilePage
import app.lockbook.model.CreateFileUiState
import app.lockbook.model.CreateFileViewModel
import app.lockbook.model.CreateLocation
import app.lockbook.model.FileTreeViewModel
import app.lockbook.model.FolderChoice
import app.lockbook.model.MainNavigationAction
import app.lockbook.model.MainScreenViewModel
import app.lockbook.model.NewFileType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.requestKeyboardFocus
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.bottomsheet.BottomSheetDialogFragment

class CreateFileBottomSheetFragment : BottomSheetDialogFragment() {
    private var _binding: SheetCreateFileBinding? = null
    private val binding get() = _binding!!
    private var renderedPage: CreateFilePage? = null

    private val model: CreateFileViewModel by viewModels()
    private val mainScreenModel: MainScreenViewModel by activityViewModels()
    private val fileTreeModel: FileTreeViewModel by activityViewModels()
    private val workspaceModel: WorkspaceViewModel by activityViewModels()
    private val folderAdapter =
        CreateFolderAdapter(
            onClick = { model.selectFolder(it.file) },
            onToggle = { model.toggleFolder(it.file) },
        )
    private val dismissKeyboardOnScroll =
        object : RecyclerView.OnScrollListener() {
            override fun onScrollStateChanged(
                recyclerView: RecyclerView,
                newState: Int,
            ) {
                if (newState == RecyclerView.SCROLL_STATE_DRAGGING) hideFolderKeyboard()
            }
        }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = SheetCreateFileBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)

        binding.folderList.layoutManager = LinearLayoutManager(requireContext())
        binding.folderList.adapter = folderAdapter
        binding.folderList.itemAnimator = null
        binding.folderList.addOnScrollListener(dismissKeyboardOnScroll)

        bindActions()
        model.state.observe(viewLifecycleOwner, ::render)
        model.createdFile.observe(viewLifecycleOwner) { file ->
            fileTreeModel.reloadFiles()
            val openAutomatically =
                PreferenceManager
                    .getDefaultSharedPreferences(requireContext())
                    .getBoolean(getString(R.string.open_new_doc_automatically_key), true)
            if (file.type == net.lockbook.File.FileType.Document && openAutomatically) {
                mainScreenModel.navigate(MainNavigationAction.OpenDocument(file.id, newFile = false))
            }
            dismiss()
        }

        model.initialize(
            initialParentId = requireArguments().getString(INITIAL_PARENT_ID),
            focusedFolderId = requireArguments().getString(FOCUSED_FOLDER_ID),
            alongsideFileId = workspaceModel.currentTab.value?.id,
        )

        (requireDialog() as BottomSheetDialog).onBackPressedDispatcher.addCallback(
            viewLifecycleOwner,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (model.state.value?.isCreating == true) return
                    if (model.state.value?.page == CreateFilePage.FolderPicker) {
                        model.hideFolderPicker()
                    } else {
                        dismiss()
                    }
                }
            },
        )
    }

    override fun onStart() {
        super.onStart()
        (dialog as? BottomSheetDialog)?.behavior?.apply {
            state = BottomSheetBehavior.STATE_EXPANDED
            skipCollapsed = true
        }
        if (model.state.value?.page == CreateFilePage.FolderPicker) {
            focusFolderSearch()
        } else {
            binding.fileName.post {
                dialog?.window.requestKeyboardFocus(binding.fileName)
                binding.fileName.selectAll()
            }
        }
    }

    override fun onDestroyView() {
        binding.folderList.removeOnScrollListener(dismissKeyboardOnScroll)
        binding.folderList.adapter = null
        renderedPage = null
        _binding = null
        super.onDestroyView()
    }

    private fun bindActions() {
        binding.createFile.setOnClickListener { model.create() }
        binding.selectFolder.setOnClickListener { model.confirmFolder() }
        binding.locationFocused.setOnClickListener { model.selectFocusedFolder() }
        binding.locationAlongside.setOnClickListener { model.selectAlongside() }
        binding.locationChoose.setOnClickListener { model.showFolderPicker() }
        binding.fileName.addTextChangedListener { model.setName(it?.toString().orEmpty()) }
        binding.folderSearch.addTextChangedListener { model.setFolderQuery(it?.toString().orEmpty()) }
        binding.fileTypeGroup.addOnButtonCheckedListener { _, checkedId, isChecked ->
            if (!isChecked) return@addOnButtonCheckedListener
            model.setType(
                when (checkedId) {
                    R.id.type_drawing -> NewFileType.Drawing
                    R.id.type_folder -> NewFileType.Folder
                    R.id.type_other -> NewFileType.Other
                    else -> NewFileType.Note
                },
            )
        }
    }

    private fun render(state: CreateFileUiState) {
        val enteringFolderPicker =
            state.page == CreateFilePage.FolderPicker && renderedPage != CreateFilePage.FolderPicker
        val returningToDetails =
            state.page == CreateFilePage.Details && renderedPage == CreateFilePage.FolderPicker
        binding.createDetailsPage.isVisible = state.page == CreateFilePage.Details
        binding.folderPickerPage.isVisible = state.page == CreateFilePage.FolderPicker
        renderedPage = state.page
        when {
            enteringFolderPicker -> focusFolderSearch()
            returningToDetails -> focusFileNameAtEnd()
        }

        val typeId =
            when (state.type) {
                NewFileType.Note -> R.id.type_note
                NewFileType.Drawing -> R.id.type_drawing
                NewFileType.Folder -> R.id.type_folder
                NewFileType.Other -> R.id.type_other
            }
        if (binding.fileTypeGroup.checkedButtonId != typeId) binding.fileTypeGroup.check(typeId)
        if (binding.fileName.text?.toString() != state.name) {
            binding.fileName.setText(state.name)
            binding.fileName.selectAll()
        }
        binding.fileNameLayout.suffixText = state.type.extension
        binding.fileNameLayout.setStartIconDrawable(state.type.iconRes)

        binding.locationFocused.text = state.focusedFolderLabel ?: getString(R.string.home)
        binding.locationFocused.setChipIconResource(
            if (state.focusedFolderIsRoot) R.drawable.ic_baseline_home_24 else R.drawable.ic_baseline_folder_24,
        )
        binding.locationAlongside.isVisible =
            state.alongsideParentId != null && state.alongsideParentId != state.focusedFolderId
        binding.locationAlongside.text = state.alongsideLabel?.let { getString(R.string.alongside_file, it) }
        binding.locationChoose.text = state.customLabel ?: getString(R.string.choose_location)
        val locationId =
            when (state.location) {
                CreateLocation.FocusedFolder -> R.id.location_focused
                CreateLocation.Alongside -> R.id.location_alongside
                CreateLocation.Custom -> R.id.location_choose
            }
        if (binding.locationGroup.checkedChipId != locationId) binding.locationGroup.check(locationId)

        binding.createFileError.text = state.error.orEmpty()
        binding.createFileError.isVisible = !state.error.isNullOrBlank()
        binding.createFile.isEnabled = !state.isLoading && !state.isCreating && state.name.isNotBlank()
        binding.fileNameLayout.isEnabled = !state.isCreating
        binding.fileTypeGroup.children.forEach { it.isEnabled = !state.isCreating }
        binding.locationGroup.children.forEach { it.isEnabled = !state.isCreating }
        binding.createFileProgress.isVisible = state.isCreating
        isCancelable = !state.isCreating
        (dialog as? BottomSheetDialog)?.apply {
            setCanceledOnTouchOutside(!state.isCreating)
            behavior.isHideable = !state.isCreating
        }

        val query = state.folderQuery.trim()
        val visibleEntries =
            if (query.isEmpty()) {
                visibleTreeEntries(state.pickerEntries, state.expandedFolderIds)
            } else {
                state.pickerEntries
                    .filter {
                        val matchesQuery =
                            it.file.name.contains(query, ignoreCase = true) ||
                                it.path.contains(query, ignoreCase = true)
                        it.file.type == net.lockbook.File.FileType.Folder &&
                            matchesQuery
                    }.map { it.copy(isExpanded = false) }
            }
        folderAdapter.showHierarchy = query.isEmpty()
        folderAdapter.selectedId = state.selectedFolderId
        folderAdapter.submitList(visibleEntries)
        val selected = state.pickerEntries.firstOrNull { it.file.id == state.selectedFolderId }?.file
        binding.selectFolder.isEnabled = selected != null
        binding.selectFolder.text =
            getString(
                R.string.select_folder,
                selected?.let { if (it.isRoot) getString(R.string.home) else it.name }.orEmpty(),
            )
    }

    private fun focusFolderSearch() {
        binding.folderSearch.post {
            dialog?.window.requestKeyboardFocus(binding.folderSearch)
            binding.folderSearch.setSelection(binding.folderSearch.length())
        }
    }

    private fun focusFileNameAtEnd() {
        binding.fileName.post {
            dialog?.window.requestKeyboardFocus(binding.fileName)
            binding.fileName.setSelection(binding.fileName.length())
        }
    }

    private fun hideFolderKeyboard() {
        val inputMethodManager = requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        inputMethodManager.hideSoftInputFromWindow(binding.folderSearch.windowToken, 0)
        binding.folderSearch.clearFocus()
    }

    private fun visibleTreeEntries(
        entries: List<FolderChoice>,
        expandedFolderIds: Set<String>,
    ): List<FolderChoice> {
        val root = entries.firstOrNull { it.file.isRoot } ?: return emptyList()
        val childrenByParent =
            entries
                .asSequence()
                .filterNot { it.file.isRoot }
                .groupBy { it.file.parent }
                .mapValues { (_, children) ->
                    children.sortedWith(
                        compareBy<FolderChoice>(
                            { it.file.type != net.lockbook.File.FileType.Folder },
                            { it.file.name.lowercase() },
                        ),
                    )
                }
        val visible = mutableListOf(root.copy(isExpanded = root.file.id in expandedFolderIds))

        fun appendExpandedChildren(parentId: String) {
            childrenByParent[parentId].orEmpty().forEach { entry ->
                val displayedEntry = entry.copy(isExpanded = entry.file.id in expandedFolderIds)
                visible += displayedEntry
                if (displayedEntry.file.type == net.lockbook.File.FileType.Folder && displayedEntry.isExpanded) {
                    appendExpandedChildren(displayedEntry.file.id)
                }
            }
        }

        if (root.file.id in expandedFolderIds) appendExpandedChildren(root.file.id)
        return visible
    }

    companion object {
        const val TAG = "CreateFileBottomSheetFragment"
        private const val INITIAL_PARENT_ID = "initial_parent_id"
        private const val FOCUSED_FOLDER_ID = "focused_folder_id"

        fun newInstance(
            initialParentId: String?,
            focusedFolderId: String?,
        ): CreateFileBottomSheetFragment =
            CreateFileBottomSheetFragment().apply {
                arguments =
                    bundleOf(
                        INITIAL_PARENT_ID to initialParentId,
                        FOCUSED_FOLDER_ID to focusedFolderId,
                    )
            }
    }
}
