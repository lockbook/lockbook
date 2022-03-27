package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.method.LinkMovementMethod
import android.view.*
import android.widget.TextView
import androidx.core.content.res.ResourcesCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.databinding.FragmentFilesListBinding
import app.lockbook.model.*
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import app.lockbook.util.FilesFragment
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.viewholder.isSelected
import com.afollestad.recyclical.withItem
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import timber.log.Timber
import java.lang.ref.WeakReference
import java.util.*

class FilesListFragment : Fragment(), FilesFragment {
    private var _binding: FragmentFilesListBinding? = null
    private val binding get() = _binding!!
    private val menu get() = binding.filesToolbar

    private val model: FilesListViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(FilesListViewModel::class.java))
                        return FilesListViewModel(requireActivity().application, (activity as MainScreenActivity).isThisANewAccount()) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )
    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private val recyclerView get() = binding.filesList

    private var updatedLastSyncedDescription = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))

    private val snackProgressBarManager by lazy {
        SnackProgressBarManager(
            requireView(),
            lifecycleOwner = this
        ).setViewToMove(binding.listFilesFrameLayout)
    }

    private val syncSnackProgressBar by lazy {
        SnackProgressBar(
            SnackProgressBar.TYPE_HORIZONTAL,
            resources.getString(R.string.list_files_sync_snackbar_default)
        )
            .setIsIndeterminate(false)
            .setSwipeToDismiss(false)
            .setAllowUserInput(true)
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentFilesListBinding.inflate(inflater, container, false)

        setUpToolbar()

        if (model.breadcrumbItems.isNotEmpty()) {
            updateUI(UpdateFilesUI.UpdateBreadcrumbBar(model.breadcrumbItems))
        }

        model.notifyUpdateFilesUI.observe(
            viewLifecycleOwner
        ) { uiUpdates ->
            updateUI(uiUpdates)
        }

        model.syncModel.notifySyncStepInfo.observe(
            viewLifecycleOwner
        ) { syncProgress ->
            updateSyncProgress(syncProgress)
        }

        setUpFilesList()

        binding.filesBreadcrumbBar.setListener(object : BreadCrumbItemClickListener {
            override fun onItemClick(breadCrumbItem: View, position: Int) {
                model.intoAncestralFolder(position)
                model.selectableFiles.deselectAll()
                toggleMenuBar()
            }
        })

        when (
            val optionValue = PreferenceManager.getDefaultSharedPreferences(requireContext()).getString(
                getString(R.string.sort_files_key),
                getString(R.string.sort_files_a_z_value)
            )
        ) {
            getString(R.string.sort_files_a_z_value) -> menu.menu!!.findItem(R.id.menu_list_files_sort_a_z)?.isChecked = true
            getString(R.string.sort_files_z_a_value) -> menu.menu!!.findItem(R.id.menu_list_files_sort_z_a)?.isChecked = true
            getString(R.string.sort_files_last_changed_value) ->
                menu.menu!!.findItem(R.id.menu_list_files_sort_last_changed)?.isChecked =
                    true
            getString(R.string.sort_files_first_changed_value) ->
                menu.menu!!.findItem(R.id.menu_list_files_sort_first_changed)?.isChecked =
                    true
            getString(R.string.sort_files_type_value) -> menu.menu!!.findItem(R.id.menu_list_files_sort_type)?.isChecked = true
            else -> {
                Timber.e("File sorting shared preference does not match every supposed option: $optionValue")
                alertModel.notifyBasicError()
            }
        }.exhaustive

        binding.listFilesRefresh.setOnRefreshListener {
            model.onSwipeToRefresh()
        }

        updatedLastSyncedDescription.schedule(
            object : TimerTask() {
                @SuppressLint("NotifyDataSetChanged")
                override fun run() {
                    handler.post {
                        binding.filesList.adapter?.notifyDataSetChanged()
                    }
                }
            },
            30000,
            30000
        )

        binding.fabsNewFile.listFilesFab.setOnClickListener {
            collapseExpandFAB()
        }

        binding.fabsNewFile.listFilesFabFolder.setOnClickListener {
            onDocumentFolderFabClicked(ExtendedFileType.Folder)
        }

        binding.fabsNewFile.listFilesFabDocument.setOnClickListener {
            onDocumentFolderFabClicked(ExtendedFileType.Text)
        }

        binding.fabsNewFile.listFilesFabDrawing.setOnClickListener {
            onDocumentFolderFabClicked(ExtendedFileType.Drawing)
        }

        return binding.root
    }

    private fun updateSyncProgress(syncStepInfo: SyncStepInfo) {
        if (syncStepInfo.progress == 0) {
            snackProgressBarManager.dismiss()

            updateUI(UpdateFilesUI.ShowSyncSnackBar(syncStepInfo.total))
        } else {

            syncSnackProgressBar.setProgressMax(syncStepInfo.total)
            snackProgressBarManager.setProgress(syncStepInfo.progress)
            syncSnackProgressBar.setMessage(syncStepInfo.action.toMessage())
            snackProgressBarManager.updateTo(syncSnackProgressBar)
        }
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        val syncStatus = model.syncModel.syncStatus
        if (syncStatus is SyncStatus.Syncing) {
            updateUI(UpdateFilesUI.ShowSyncSnackBar(syncStatus.syncStepInfo.total))
            updateSyncProgress(syncStatus.syncStepInfo)
        }
    }

    private fun setUpToolbar() {
        binding.filesToolbar.title = "Lockbook"
        binding.filesToolbar.inflateMenu(R.menu.menu_list_files)
        binding.filesToolbar.setOnMenuItemClickListener { item ->
            val selectedFiles = model.selectableFiles.getSelectedItems()

            when (item.itemId) {
                R.id.menu_list_files_settings -> startActivity(
                    Intent(
                        context,
                        SettingsActivity::class.java
                    )
                )
                R.id.menu_list_files_sort_last_changed -> {
                    model.changeFileSort(SortStyle.LastChanged)
                    menu.menu!!.findItem(R.id.menu_list_files_sort_last_changed)?.isChecked = true
                }
                R.id.menu_list_files_sort_a_z -> {
                    model.changeFileSort(SortStyle.AToZ)
                    menu.menu!!.findItem(R.id.menu_list_files_sort_a_z)?.isChecked = true
                }
                R.id.menu_list_files_sort_z_a -> {
                    model.changeFileSort(SortStyle.ZToA)
                    menu.menu!!.findItem(R.id.menu_list_files_sort_z_a)?.isChecked = true
                }
                R.id.menu_list_files_sort_first_changed -> {
                    model.changeFileSort(SortStyle.FirstChanged)
                    menu.menu!!.findItem(R.id.menu_list_files_sort_first_changed)?.isChecked = true
                }
                R.id.menu_list_files_sort_type -> {
                    model.changeFileSort(SortStyle.FileType)
                    menu.menu!!.findItem(R.id.menu_list_files_sort_type)?.isChecked = true
                }
                R.id.menu_list_files_rename -> {
                    if (selectedFiles.size == 1) {
                        activityModel.launchTransientScreen(TransientScreen.Rename(selectedFiles[0]))
                    }
                }
                R.id.menu_list_files_delete -> {
                    activityModel.detailsScreen?.fileMetadata.let {
                        if (model.selectableFiles.getSelectedItems().contains(it)) {
                            activityModel.launchDetailsScreen(null)
                        }
                    }

                    model.deleteSelectedFiles()
                    alertModel.notify(getString(R.string.success_delete))
                }
                R.id.menu_list_files_info -> {
                    if (model.selectableFiles.getSelectionCount() == 1) {
                        activityModel.launchTransientScreen(TransientScreen.Info(selectedFiles[0]))
                    }
                }
                R.id.menu_list_files_move -> {
                    activityModel.launchTransientScreen(
                        TransientScreen.Move(
                            selectedFiles.map { it.id }.toTypedArray()
                        )
                    )
                }
                R.id.menu_list_files_share -> {
                    (activity as MainScreenActivity).model.shareSelectedFiles(model.selectableFiles.getSelectedItems(), requireActivity().cacheDir)
                }
            }

            toggleMenuBar()

            true
        }
    }

    private fun setUpFilesList() {
        recyclerView.setup {
            withDataSource(model.selectableFiles)
            withEmptyView(binding.listFilesFrameLayout.findViewById(R.id.files_empty_folder)!!)

            withItem<DecryptedFileMetadata, HorizontalViewHolder>(R.layout.linear_layout_file_item) {
                onBind(::HorizontalViewHolder) { _, item ->
                    name.text = item.decryptedName
                    description.text = resources.getString(
                        R.string.last_synced,
                        CoreModel.convertToHumanDuration(item.metadataVersion)
                    )

                    when {
                        isSelected() -> {
                            icon.setImageResource(R.drawable.ic_baseline_check_24)
                        }
                        item.fileType == FileType.Document && item.decryptedName.endsWith(".draw") -> {
                            icon.setImageResource(R.drawable.ic_baseline_border_color_24)
                        }
                        item.fileType == FileType.Document -> {
                            icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                        }
                        else -> {
                            icon.setImageResource(R.drawable.round_folder_white_18dp)
                        }
                    }

                    itemView.background.setTint(
                        ResourcesCompat.getColor(
                            resources,
                            if (isSelected()) R.color.selectedFileBackground else R.color.colorPrimaryDark,
                            itemView.context.theme
                        )
                    )
                }
                onClick {
                    if (isSelected() || model.selectableFiles.hasSelection()) {
                        toggleSelection()
                        toggleMenuBar()
                    } else {
                        enterFile(item)
                    }
                }
                onLongClick {
                    this.toggleSelection()
                    toggleMenuBar()
                }
            }
        }
    }

    private fun enterFile(item: DecryptedFileMetadata) {
        when (item.fileType) {
            FileType.Document -> {
                activityModel.launchDetailsScreen(DetailsScreen.Loading(item))
            }
            FileType.Folder -> {
                model.enterFolder(item)
            }
        }
    }

    private fun updateUI(uiUpdates: UpdateFilesUI) {
        when (uiUpdates) {
            is UpdateFilesUI.NotifyError -> alertModel.notifyError(uiUpdates.error)
            is UpdateFilesUI.NotifyWithSnackbar -> {
                alertModel.notify(uiUpdates.msg)
            }
            is UpdateFilesUI.ShowSyncSnackBar -> {
                snackProgressBarManager.dismiss()
                syncSnackProgressBar.setMessage(resources.getString(R.string.list_files_sync_snackbar, uiUpdates.totalSyncItems.toString()))
                snackProgressBarManager.show(
                    syncSnackProgressBar,
                    SnackProgressBarManager.LENGTH_INDEFINITE
                )
            }
            UpdateFilesUI.StopProgressSpinner ->
                binding.listFilesRefresh.isRefreshing =
                    false
            is UpdateFilesUI.UpdateBreadcrumbBar -> {
                binding.filesBreadcrumbBar.setBreadCrumbItems(
                    uiUpdates.breadcrumbItems.toMutableList()
                )
            }
            UpdateFilesUI.ToggleMenuBar -> toggleMenuBar()
            UpdateFilesUI.ShowBeforeWeStart -> {
                val bottomSheetDialog = BottomSheetDialog(requireContext())
                bottomSheetDialog.setContentView(R.layout.sheet_before_you_start)

                bottomSheetDialog.findViewById<TextView>(R.id.before_you_start_description)!!.movementMethod = LinkMovementMethod.getInstance()

                bottomSheetDialog.show()
            }
            UpdateFilesUI.SyncImport -> {
                (activity as MainScreenActivity).syncImportAccount()
            }
        }.exhaustive
    }

    private fun toggleMenuBar() {
        when (model.selectableFiles.getSelectionCount()) {
            0 -> {
                for (menuItem in menuItemsOneOrMoreSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = false
                }

                for (menuItem in menuItemsOneSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = false
                }

                for (menuItem in menuItemsNoneSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = true
                }
            }
            1 -> {
                for (menuItem in menuItemsNoneSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = false
                }

                for (menuItem in menuItemsOneOrMoreSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = true
                }

                for (menuItem in menuItemsOneSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = true
                }
            }
            else -> {
                for (menuItem in menuItemsOneSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = false
                }

                for (menuItem in menuItemsNoneSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = false
                }

                for (menuItem in menuItemsOneOrMoreSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = true
                }
            }
        }
    }

    private fun onDocumentFolderFabClicked(extendedFileType: ExtendedFileType) {
        activityModel.launchTransientScreen(
            TransientScreen.Create(
                CreateFileInfo(
                    model.fileModel.parent.id,
                    extendedFileType
                )
            )
        )
    }

    private fun collapseExpandFAB() {
        if (binding.fabsNewFile.listFilesFabDocument.isOrWillBeHidden) {
            showFABMenu()
        } else {
            closeFABMenu()
        }
    }

    private fun closeFABMenu() {
        val fabsNewFile = binding.fabsNewFile
        fabsNewFile.listFilesFab.animate().setDuration(200L).rotation(90f)
        fabsNewFile.listFilesFab.setImageResource(R.drawable.ic_baseline_add_24)
        fabsNewFile.listFilesFabFolder.hide()
        fabsNewFile.listFilesFabDocument.hide()
        fabsNewFile.listFilesFabDrawing.hide()
        binding.listFilesRefresh.alpha = 1f
        binding.listFilesFrameLayout.isClickable = false
    }

    private fun showFABMenu() {
        val fabsNewFile = binding.fabsNewFile
        fabsNewFile.listFilesFab.animate().setDuration(200L).rotation(-90f)
        fabsNewFile.listFilesFabFolder.show()
        fabsNewFile.listFilesFabDocument.show()
        fabsNewFile.listFilesFabDrawing.show()
        binding.listFilesRefresh.alpha = 0.7f
        binding.listFilesFrameLayout.isClickable = true
        binding.listFilesFrameLayout.setOnClickListener {
            closeFABMenu()
        }
    }

    override fun onBackPressed(): Boolean = when {
        model.selectableFiles.hasSelection() -> {
            model.selectableFiles.deselectAll()
            toggleMenuBar()
            false
        }
        !model.fileModel.isAtRoot() -> {
            model.intoParentFolder()
            false
        }
        else -> true
    }

    override fun syncBasedOnPreferences() {
        model.syncBasedOnPreferences()
    }

    override fun refreshFiles() {
        model.reloadFiles()
    }

    override fun unselectFiles() {
        model.selectableFiles.deselectAll()
        toggleMenuBar()
    }

    override fun onNewFileCreated(newDocument: DecryptedFileMetadata?) {
        when {
            newDocument != null && PreferenceManager.getDefaultSharedPreferences(requireContext())
                .getBoolean(getString(R.string.open_new_doc_automatically_key), true) -> {
                enterFile(newDocument)
                if (newDocument.fileType == FileType.Document) {
                    model.reloadFiles()
                }
            }
            newDocument != null -> model.reloadFiles()
        }

        closeFABMenu()
    }
}

sealed class UpdateFilesUI {
    data class UpdateBreadcrumbBar(val breadcrumbItems: List<BreadCrumbItem>) : UpdateFilesUI()
    data class NotifyError(val error: LbError) : UpdateFilesUI()
    data class ShowSyncSnackBar(val totalSyncItems: Int) : UpdateFilesUI()
    object StopProgressSpinner : UpdateFilesUI()
    object ToggleMenuBar : UpdateFilesUI()
    object ShowBeforeWeStart : UpdateFilesUI()
    object SyncImport : UpdateFilesUI()
    data class NotifyWithSnackbar(val msg: String) : UpdateFilesUI()
}

private val menuItemsNoneSelected = listOf(
    R.id.menu_list_files_sort,
    R.id.menu_list_files_settings
)

private val menuItemsOneOrMoreSelected = listOf(
    R.id.menu_list_files_delete,
    R.id.menu_list_files_move,
    R.id.menu_list_files_share
)

private val menuItemsOneSelected = listOf(
    R.id.menu_list_files_rename,
    R.id.menu_list_files_info,
)
