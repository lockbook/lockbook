package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.method.LinkMovementMethod
import android.view.*
import androidx.core.content.res.ResourcesCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.LinearLayoutManager
import app.futured.donut.DonutProgressView
import app.futured.donut.DonutSection
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
                val selectedFiles = model.files.getSelectedItems()

                return when (item?.itemId) {
                    R.id.menu_list_files_rename -> {
                        if (selectedFiles.size == 1) {
                            activityModel.launchTransientScreen(TransientScreen.Rename(selectedFiles[0].fileMetadata))
                        }

                        true
                    }
                    R.id.menu_list_files_delete -> {
                        activityModel.launchTransientScreen(TransientScreen.Delete(selectedFiles.intoFileMetadata()))

                        true
                    }
                    R.id.menu_list_files_info -> {
                        if (model.files.getSelectionCount() == 1) {
                            activityModel.launchTransientScreen(TransientScreen.Info(selectedFiles[0].fileMetadata))
                        }

                        true
                    }
                    R.id.menu_list_files_move -> {
                        activityModel.launchTransientScreen(
                            TransientScreen.Move(selectedFiles.intoFileMetadata())
                        )

                        true
                    }
                    R.id.menu_list_files_share -> {
                        (activity as MainScreenActivity).model.shareSelectedFiles(selectedFiles.intoFileMetadata(), requireActivity().cacheDir)

                        true
                    }
                    else -> false
                }
            }

            override fun onDestroyActionMode(mode: ActionMode?) {
                model.files.deselectAll()
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
        binding.fabSpeedDial.mainFab.setOnLongClickListener { view ->
            view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
            model.generateQuickNote(activityModel)
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
                        model.reloadWorkInfo()
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

        model.maybeLastSidebarInfo?.let { uiUpdate ->
            updateUI(uiUpdate)
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

        if (!model.isRecentFilesVisible) {
            binding.recentFilesLayout.root.visibility = View.GONE
        }

        (requireActivity().application as App).billingClientLifecycle.showInAppMessaging(requireActivity())
    }

    private fun setUpToolbar() {
        binding.filesToolbar.setNavigationOnClickListener {
            binding.drawerLayout.open()
        }
        binding.navigationView.setNavigationItemSelectedListener { item ->
            when (item.itemId) {
                R.id.menu_files_list_settings -> startActivity(
                    Intent(
                        context,
                        SettingsActivity::class.java
                    )
                )
            }

            binding.drawerLayout.close()
            true
        }
        binding.filesToolbar.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                R.id.menu_files_list_search -> {
                    activityModel.updateMainScreenUI(UpdateMainScreenUI.ShowSearch)
                }
            }

            toggleMenuBar()

            true
        }

        toggleMenuBar()
    }

    private fun setUpFilesList() {
        recyclerView.setup {
            withDataSource(model.files)
            withEmptyView(binding.filesEmptyFolder)

            withItem<FileViewHolderInfo.FolderViewHolderInfo, FolderViewHolder>(R.layout.folder_file_item) {
                onBind(::FolderViewHolder) { _, item ->
                    name.text = item.fileMetadata.name

                    when {
                        isSelected() -> {
                            fileItemHolder.setBackgroundResource(R.color.md_theme_primaryContainer)
                            actionIcon.setImageResource(R.drawable.ic_baseline_check_circle_24)
                            actionIcon.visibility = View.VISIBLE
                        }
                        item.needsToBePulled -> {
                            fileItemHolder.setBackgroundResource(0)
                            actionIcon.setImageResource(R.drawable.ic_baseline_cloud_download_24)
                            actionIcon.visibility = View.VISIBLE
                        }
                        item.needToBePushed -> {
                            fileItemHolder.setBackgroundResource(0)
                            actionIcon.setImageResource(R.drawable.ic_baseline_cloud_upload_24)
                            actionIcon.visibility = View.VISIBLE
                        }
                        else -> {
                            fileItemHolder.setBackgroundResource(0)
                            actionIcon.visibility = View.GONE
                        }
                    }
                }

                onClick {
                    if (isSelected() || model.files.hasSelection()) {
                        toggleSelection()
                        toggleMenuBar()
                    } else {
                        enterFile(item.fileMetadata)
                    }
                }

                onLongClick {
                    this.toggleSelection()
                    toggleMenuBar()
                }
            }

            withItem<FileViewHolderInfo.DocumentViewHolderInfo, DocumentViewHolder>(R.layout.document_file_item) {
                onBind(::DocumentViewHolder) { _, item ->
                    name.text = item.fileMetadata.name
                    if (item.fileMetadata.lastModified != 0L) {
                        description.visibility = View.VISIBLE
                        description.text = CoreModel.convertToHumanDuration(item.fileMetadata.lastModified)
                    } else {
                        description.visibility = View.GONE
                    }

                    val extensionHelper = ExtensionHelper(item.fileMetadata.name)

                    val iconResource = when {
                        extensionHelper.isDrawing -> R.drawable.ic_outline_draw_24
                        extensionHelper.isImage -> R.drawable.ic_outline_image_24
                        extensionHelper.isPdf -> R.drawable.ic_outline_picture_as_pdf_24
                        else -> R.drawable.ic_outline_insert_drive_file_24
                    }

                    icon.setImageResource(iconResource)

                    when {
                        isSelected() -> {
                            fileItemHolder.setBackgroundResource(R.color.md_theme_primaryContainer)
                            actionIcon.setImageResource(R.drawable.ic_baseline_check_circle_24)
                            actionIcon.visibility = View.VISIBLE
                        }
                        item.needsToBePulled -> {
                            fileItemHolder.setBackgroundResource(0)
                            actionIcon.setImageResource(R.drawable.ic_baseline_cloud_download_24)
                            actionIcon.visibility = View.VISIBLE
                        }
                        item.needToBePushed -> {
                            fileItemHolder.setBackgroundResource(0)
                            actionIcon.setImageResource(R.drawable.ic_baseline_cloud_upload_24)
                            actionIcon.visibility = View.VISIBLE
                        }
                        else -> {
                            fileItemHolder.setBackgroundResource(0)
                            actionIcon.visibility = View.GONE
                        }
                    }
                }

                onClick {
                    if (isSelected() || model.files.hasSelection()) {
                        toggleSelection()
                        toggleMenuBar()
                    } else {
                        enterFile(item.fileMetadata)
                    }
                }

                onLongClick {
                    this.toggleSelection()
                    toggleMenuBar()
                }
            }
        }

        binding.recentFilesLayout.recentFilesList.setup {
            withDataSource(model.recentFiles)
            this.withLayoutManager(LinearLayoutManager(requireContext(), LinearLayoutManager.HORIZONTAL, false))

            withItem<RecentFileViewHolderInfo, RecentFileItemViewHolder>(R.layout.recent_file_item) {
                onBind(::RecentFileItemViewHolder) { _, item ->
                    name.text = item.fileMetadata.name
                    folderName.text = getString(R.string.recent_files_folder, item.folderName)
                    lastEdited.text = CoreModel.convertToHumanDuration(item.fileMetadata.lastModified)

                    val extensionHelper = ExtensionHelper(item.fileMetadata.name)

                    val iconResource = when {
                        extensionHelper.isDrawing -> R.drawable.ic_outline_draw_24
                        extensionHelper.isImage -> R.drawable.ic_outline_image_24
                        extensionHelper.isPdf -> R.drawable.ic_outline_picture_as_pdf_24
                        else -> R.drawable.ic_outline_insert_drive_file_24
                    }

                    icon.setImageResource(iconResource)
                }

                onClick {
                    enterFile(item.fileMetadata)
                }
            }
        }
    }

    private fun enterFile(item: File) {
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
                if (binding.listFilesRefresh.isRefreshing) {
                    binding.listFilesRefresh.isRefreshing = false
                }

                if (binding.syncHolder.isVisible) {
                    binding.syncHolder.visibility = View.GONE
                }

                alertModel.notifyError(uiUpdates.error)
            }
            is UpdateFilesUI.NotifyWithSnackbar -> {
                if (binding.listFilesRefresh.isRefreshing) {
                    binding.listFilesRefresh.isRefreshing = false
                }

                if (binding.syncHolder.isVisible) {
                    binding.syncHolder.visibility = View.GONE
                }

                alertModel.notify(uiUpdates.msg)
            }
            is UpdateFilesUI.ShowSyncSnackBar -> {
                binding.syncProgressIndicator.max = uiUpdates.totalSyncItems
                binding.syncProgressIndicator.visibility = View.VISIBLE
                binding.syncText.text = resources.getString(R.string.list_files_sync_snackbar, uiUpdates.totalSyncItems.toString())
                binding.syncHolder.visibility = View.VISIBLE
            }
            UpdateFilesUI.UpToDateSyncSnackBar -> {
                binding.listFilesRefresh.isRefreshing = false

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
            is UpdateFilesUI.UpdateSideBarInfo -> {
                uiUpdates.usageMetrics?.let { usageMetrics ->
                    val dataCap = usageMetrics.dataCap.exact.toFloat()
                    val usage = usageMetrics.serverUsage.exact.toFloat()

                    val donut = binding.navigationView.getHeaderView(0).findViewById<DonutProgressView>(R.id.filesListUsageDonut)
                    donut.cap = dataCap

                    val usageSection = DonutSection(
                        name = "",
                        color = ResourcesCompat.getColor(resources, R.color.md_theme_primary, null),
                        amount = usage
                    )

                    donut.submitData(listOf(usageSection))

                    binding.navigationView.getHeaderView(0).findViewById<MaterialTextView>(R.id.filesListUsage).text = getString(R.string.free_space, usageMetrics.serverUsage.readable, usageMetrics.dataCap.readable)
                }

                uiUpdates.lastSynced?.let { lastSynced ->
                    binding.navigationView.getHeaderView(0).findViewById<MaterialTextView>(R.id.filesListLastSynced).text = getString(R.string.last_sync, lastSynced)
                }

                uiUpdates.localDirtyFilesCount?.let { localDirtyFilesCount ->
                    binding.navigationView.getHeaderView(0).findViewById<MaterialTextView>(R.id.filesListLocalDirty).text = resources.getQuantityString(R.plurals.files_to_push, localDirtyFilesCount, localDirtyFilesCount)
                }

                uiUpdates.serverDirtyFilesCount?.let { serverDirtyFilesCount ->
                    binding.navigationView.getHeaderView(0).findViewById<MaterialTextView>(R.id.filesListServerDirty).text = resources.getQuantityString(R.plurals.files_to_pull, serverDirtyFilesCount, serverDirtyFilesCount)
                }
            }
            is UpdateFilesUI.ToggleRecentFilesVisibility -> {
                binding.recentFilesLayout.root.visibility = if (uiUpdates.show) View.VISIBLE else View.GONE
            }
        }
    }

    private fun toggleMenuBar() {
        when (val selectionCount = model.files.getSelectionCount()) {
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
        model.files.hasSelection() -> {
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
        model.files.deselectAll()
        toggleMenuBar()
    }

    override fun onNewFileCreated(newDocument: File?) {
        when {
            newDocument != null && PreferenceManager.getDefaultSharedPreferences(requireContext())
                .getBoolean(getString(R.string.open_new_doc_automatically_key), true) -> {
                model.reloadFiles()
                enterFile(newDocument)
            }
            newDocument != null -> model.reloadFiles()
        }
    }
}

sealed class UpdateFilesUI {
    data class UpdateBreadcrumbBar(val breadcrumbItems: List<BreadCrumbItem>) : UpdateFilesUI()
    data class NotifyError(val error: LbError) : UpdateFilesUI()
    data class ShowSyncSnackBar(val totalSyncItems: Int) : UpdateFilesUI()
    data class UpdateSideBarInfo(var usageMetrics: UsageMetrics? = null, var lastSynced: String? = null, var localDirtyFilesCount: Int? = null, var serverDirtyFilesCount: Int? = null) : UpdateFilesUI()
    data class ToggleRecentFilesVisibility(var show: Boolean) : UpdateFilesUI()
    object UpToDateSyncSnackBar : UpdateFilesUI()
    object ToggleMenuBar : UpdateFilesUI()
    object ShowBeforeWeStart : UpdateFilesUI()
    object SyncImport : UpdateFilesUI()
    data class NotifyWithSnackbar(val msg: String) : UpdateFilesUI()
}
