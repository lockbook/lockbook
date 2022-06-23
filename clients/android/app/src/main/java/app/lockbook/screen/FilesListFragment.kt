package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.method.LinkMovementMethod
import android.view.*
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.preference.PreferenceManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.FragmentFilesListBinding
import app.lockbook.model.*
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.viewholder.isSelected
import com.afollestad.recyclical.withItem
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.button.MaterialButton
import com.google.android.material.textview.MaterialTextView
import java.lang.ref.WeakReference
import java.util.*

class FilesListFragment : Fragment(), FilesFragment {
    private var _binding: FragmentFilesListBinding? = null
    private val binding get() = _binding!!
    private val menu get() = binding.filesToolbar
    private var actionModeMenu: ActionMode? = null
    private val actionModeMenuCallback: ActionMode.Callback by lazy {
        object : ActionMode.Callback {
            override fun onCreateActionMode(mode: ActionMode?, menu: Menu?): Boolean {
                mode?.menuInflater?.inflate(R.menu.menu_files_list_selected, menu)
                return true
            }

            override fun onPrepareActionMode(mode: ActionMode?, menu: Menu?): Boolean = false

            override fun onActionItemClicked(mode: ActionMode?, item: MenuItem?): Boolean {
                val selectedFiles = model.selectableFiles.getSelectedItems()

                return when (item?.itemId) {
                    R.id.menu_list_files_rename -> {
                        if (selectedFiles.size == 1) {
                            activityModel.launchTransientScreen(TransientScreen.Rename(selectedFiles[0]))
                        }

                        true
                    }
                    R.id.menu_list_files_delete -> {
                        activityModel.launchTransientScreen(TransientScreen.Delete(model.selectableFiles.getSelectedItems()))

                        true
                    }
                    R.id.menu_list_files_info -> {
                        if (model.selectableFiles.getSelectionCount() == 1) {
                            activityModel.launchTransientScreen(TransientScreen.Info(selectedFiles[0]))
                        }

                        true
                    }
                    R.id.menu_list_files_move -> {
                        activityModel.launchTransientScreen(
                            TransientScreen.Move(selectedFiles)
                        )

                        true
                    }
                    R.id.menu_list_files_share -> {
                        (activity as MainScreenActivity).model.shareSelectedFiles(model.selectableFiles.getSelectedItems(), requireActivity().cacheDir)

                        true
                    }
                    else -> false
                }
            }

            override fun onDestroyActionMode(mode: ActionMode?) {
                model.selectableFiles.deselectAll()
                actionModeMenu = null
            }
        }
    }

    private val model: FilesListViewModel by viewModels()
    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private val recyclerView get() = binding.filesList

    private var updatedLastSyncedDescription = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentFilesListBinding.inflate(inflater, container, false)

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

        if (model.breadcrumbItems.isNotEmpty()) {
            updateUI(UpdateFilesUI.UpdateBreadcrumbBar(model.breadcrumbItems))
        }

        binding.filesBreadcrumbBar.setListener(object : BreadCrumbItemClickListener {
            override fun onItemClick(breadCrumbItem: View, position: Int) {
                model.intoAncestralFolder(position)
                unselectFiles()
            }
        })

        when (
            PreferenceManager.getDefaultSharedPreferences(requireContext()).getString(
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
                alertModel.notifyBasicError()
            }
        }.exhaustive

        binding.fabSpeedDial.inflate(R.menu.menu_files_list_speed_dial)
        binding.fabSpeedDial.setOnActionSelectedListener {
            val extendedFileType = when (it.id) {
                R.id.fab_create_drawing -> ExtendedFileType.Drawing
                R.id.fab_create_document -> ExtendedFileType.Document
                R.id.fab_create_folder -> ExtendedFileType.Folder
                else -> return@setOnActionSelectedListener false
            }

            activityModel.launchTransientScreen(
                TransientScreen.Create(model.fileModel.parent.id, extendedFileType)
            )

            binding.fabSpeedDial.close()
            true
        }

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

        if (getApp().isNewAccount) {
            updateUI(UpdateFilesUI.ShowBeforeWeStart)
        }

        return binding.root
    }

    private fun updateSyncProgress(syncStepInfo: SyncStepInfo) {
        if (syncStepInfo.progress == 0) {
            binding.syncHolder.visibility = View.GONE
            updateUI(UpdateFilesUI.ShowSyncSnackBar(syncStepInfo.total))
        } else {
            binding.syncProgressIndicator.apply {
                max = syncStepInfo.total
                progress = syncStepInfo.progress
            }
            binding.syncText.text = syncStepInfo.action.toMessage()
        }
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        setUpToolbar()

        val syncStatus = model.syncModel.syncStatus
        if (syncStatus is SyncStatus.Syncing) {
            updateUI(UpdateFilesUI.ShowSyncSnackBar(syncStatus.syncStepInfo.total))
            updateSyncProgress(syncStatus.syncStepInfo)
        }

        (requireActivity().application as App).billingClientLifecycle.showInAppMessaging(requireActivity())
    }

    private fun setUpToolbar() {
        binding.filesToolbar.setOnMenuItemClickListener { item ->
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
            }

            toggleMenuBar()

            true
        }

        toggleMenuBar()
    }

    private fun setUpFilesList() {
        recyclerView.setup {
            withDataSource(model.selectableFiles)
            withEmptyView(binding.filesEmptyFolder)

            withItem<DecryptedFileMetadata, LinearFileItemViewHolder>(R.layout.linear_layout_file_item) {
                onBind(::LinearFileItemViewHolder) { _, item ->
                    name.text = item.decryptedName
                    description.text = resources.getString(
                        R.string.last_synced,
                        CoreModel.convertToHumanDuration(item.metadataVersion)
                    )
                    val extensionHelper = ExtensionHelper(item.decryptedName)

                    val imageResource = when {
                        isSelected() -> {
                            R.drawable.ic_baseline_check_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isDrawing -> {
                            R.drawable.ic_outline_draw_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isImage -> {
                            R.drawable.ic_outline_image_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isPdf -> {
                            R.drawable.ic_outline_picture_as_pdf_24
                        }
                        item.fileType == FileType.Document -> {
                            R.drawable.ic_outline_insert_drive_file_24
                        }
                        else -> {
                            R.drawable.ic_baseline_folder_24
                        }
                    }

                    icon.setImageResource(imageResource)

                    if (isSelected()) {
                        fileItemHolder.setBackgroundResource(R.color.md_theme_inversePrimary)
                    } else {
                        fileItemHolder.setBackgroundResource(0)
                    }
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
            is UpdateFilesUI.NotifyError -> {
                if (binding.syncHolder.isVisible) {
                    binding.syncHolder.visibility = View.GONE
                }

                alertModel.notifyError(uiUpdates.error)
            }
            is UpdateFilesUI.NotifyWithSnackbar -> {
                if (binding.syncHolder.isVisible) {
                    binding.syncHolder.visibility = View.GONE
                }

                alertModel.notify(uiUpdates.msg)
            }
            is UpdateFilesUI.ShowSyncSnackBar -> {
                binding.syncProgressIndicator.max = uiUpdates.totalSyncItems
                binding.syncText.text = resources.getString(R.string.list_files_sync_snackbar, uiUpdates.totalSyncItems.toString())
                binding.syncHolder.visibility = View.VISIBLE
            }
            UpdateFilesUI.UpToDateSyncSnackBar -> {
                binding.syncText.text = getString(R.string.list_files_sync_finished_snackbar)
                binding.syncCheck.visibility = View.VISIBLE

                if (binding.syncProgressIndicator.isVisible) {
                    binding.syncProgressIndicator.visibility = View.GONE
                }

                if (!binding.syncHolder.isVisible) {
                    binding.syncHolder.visibility = View.VISIBLE
                }

                Handler(Looper.getMainLooper()).postDelayed(
                    {
                        binding.syncHolder.visibility = View.GONE
                        binding.syncCheck.visibility = View.GONE
                        binding.syncProgressIndicator.visibility = View.VISIBLE
                    },
                    3000L
                )
            }
            UpdateFilesUI.StopProgressSpinner ->
                binding.listFilesRefresh.isRefreshing = false
            is UpdateFilesUI.UpdateBreadcrumbBar -> {
                binding.filesBreadcrumbBar.setBreadCrumbItems(
                    uiUpdates.breadcrumbItems.toMutableList()
                )
            }
            UpdateFilesUI.ToggleMenuBar -> toggleMenuBar()
            UpdateFilesUI.ShowBeforeWeStart -> {
                val beforeYouStartDialog = BottomSheetDialog(requireContext())
                beforeYouStartDialog.setContentView(R.layout.sheet_before_you_start)
                beforeYouStartDialog.findViewById<MaterialButton>(R.id.backup_my_secret)!!.setOnClickListener {
                    beforeYouStartDialog.dismiss()

                    val intent = Intent(requireContext(), SettingsActivity::class.java)
                    intent.putExtra(SettingsFragment.SCROLL_TO_PREFERENCE_KEY, R.string.export_account_raw_key)
                    startActivity(intent)
                }

                beforeYouStartDialog.findViewById<MaterialTextView>(R.id.before_you_start_description)!!.movementMethod = LinkMovementMethod.getInstance()

                beforeYouStartDialog.show()
                getApp().isNewAccount = false
            }
            UpdateFilesUI.SyncImport -> {
                (activity as MainScreenActivity).syncImportAccount()
            }
        }
    }

    private fun toggleMenuBar() {
        when (val selectionCount = model.selectableFiles.getSelectionCount()) {
            0 -> {
                actionModeMenu?.finish()
            }
            1 -> {
                if (actionModeMenu == null) {
                    actionModeMenu = menu.startActionMode(actionModeMenuCallback)
                }

                actionModeMenu?.title = getString(R.string.files_list_items_selected, selectionCount)
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_info)?.isVisible = true
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_rename)?.isVisible = true
            }
            else -> {
                if (actionModeMenu == null) {
                    actionModeMenu = menu.startActionMode(actionModeMenuCallback)
                }

                actionModeMenu?.title = getString(R.string.files_list_items_selected, selectionCount)
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_info)?.isVisible = false
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_rename)?.isVisible = false
            }
        }
    }

    override fun onBackPressed(): Boolean = when {
        model.selectableFiles.hasSelection() -> {
            unselectFiles()
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
    }
}

sealed class UpdateFilesUI {
    data class UpdateBreadcrumbBar(val breadcrumbItems: List<BreadCrumbItem>) : UpdateFilesUI()
    data class NotifyError(val error: LbError) : UpdateFilesUI()
    data class ShowSyncSnackBar(val totalSyncItems: Int) : UpdateFilesUI()
    object UpToDateSyncSnackBar : UpdateFilesUI()
    object StopProgressSpinner : UpdateFilesUI()
    object ToggleMenuBar : UpdateFilesUI()
    object ShowBeforeWeStart : UpdateFilesUI()
    object SyncImport : UpdateFilesUI()
    data class NotifyWithSnackbar(val msg: String) : UpdateFilesUI()
}
