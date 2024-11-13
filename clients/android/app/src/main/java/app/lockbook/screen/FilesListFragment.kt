package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.method.LinkMovementMethod
import android.view.*
import android.widget.ImageView
import android.widget.LinearLayout
import androidx.core.content.ContextCompat
import androidx.core.content.res.ResourcesCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
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
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.textview.MaterialTextView
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.Usage
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

                when (item?.itemId) {
                    R.id.menu_list_files_rename -> {
                        if (selectedFiles.size == 1) {
                            activityModel.launchTransientScreen(TransientScreen.Rename(selectedFiles[0].fileMetadata))
                        }
                    }
                    R.id.menu_list_files_delete -> {
                        activityModel.launchTransientScreen(TransientScreen.Delete(selectedFiles.intoFileMetadata()))
                    }
                    R.id.menu_list_files_info -> {
                        if (model.files.getSelectionCount() == 1) {
                            activityModel.launchTransientScreen(TransientScreen.Info(selectedFiles[0].fileMetadata))
                        }
                    }
                    R.id.menu_list_files_move -> {
                        activityModel.launchTransientScreen(
                            TransientScreen.Move(selectedFiles.intoFileMetadata())
                        )
                    }
                    R.id.menu_list_files_export -> {
                        (activity as MainScreenActivity).apply {
                            model.shareSelectedFiles(selectedFiles.intoFileMetadata(), cacheDir)
                        }
                    }
                    R.id.menu_list_files_share -> {
                        if (model.files.getSelectionCount() == 1) {
                            activityModel.launchTransientScreen(TransientScreen.ShareFile(selectedFiles[0].fileMetadata))
                            unselectFiles()
                        }
                    }
                    else -> return false
                }

                return true
            }

            override fun onDestroyActionMode(mode: ActionMode?) {
                model.files.deselectAll()
                actionModeMenu = null
            }
        }
    }

    private val activityModel: StateViewModel by activityViewModels()
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    private val model: FilesListViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(FilesListViewModel::class.java))
                        return FilesListViewModel(
                            requireActivity().application,
                        ) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

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
            model.generateQuickNote(workspaceModel)
            true
        }

        binding.listFilesRefresh.setOnRefreshListener {
            workspaceModel.isSyncing = true
            workspaceModel._sync.postValue(Unit)
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

        workspaceModel.msg.observe(viewLifecycleOwner) { msg ->
            binding.workspaceMsg.text = msg
        }

        workspaceModel.refreshFiles.observe(viewLifecycleOwner) {
            model.reloadFiles()
        }

        workspaceModel.syncCompleted.observe(viewLifecycleOwner) {
            binding.listFilesRefresh.isRefreshing = false
        }

        workspaceModel.selectedFile.observe(viewLifecycleOwner) { id ->
            model.fileOpened(id)
        }

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        setUpToolbar()

        if (!model.isSuggestedDocsVisible) {
            binding.suggestedDocsLayout.root.visibility = View.GONE
        }

        (requireActivity().application as App).billingClientLifecycle.showInAppMessaging(requireActivity())
    }

    private fun setUpToolbar() {
        binding.filesToolbar.setNavigationOnClickListener {
            binding.drawerLayout.open()
        }

        binding.navigationView.getHeaderView(0).let { header ->
            header.findViewById<LinearLayout>(R.id.launch_pending_shares).setOnClickListener {
                activityModel.launchActivityScreen(ActivityScreen.Shares)
                binding.drawerLayout.close()
            }

            header.findViewById<LinearLayout>(R.id.set_theme).setOnClickListener {
                var selected = ThemeMode.getSavedThemeIndex(requireContext())

                MaterialAlertDialogBuilder(requireContext())
                    .setTitle("Choose your theme")
                    .setSingleChoiceItems(ThemeMode.getThemeModes(requireContext()), selected) { _, new ->
                        selected = new
                    }
                    .setPositiveButton("Apply") { _, _ ->
                        ThemeMode.saveAndSetThemeIndex(requireContext(), selected)
                    }
                    .setNegativeButton("Cancel") { dialog, _ ->
                        dialog.dismiss()
                    }
                    .show()
            }

            header.findViewById<LinearLayout>(R.id.launch_settings).setOnClickListener {
                activityModel.launchActivityScreen(ActivityScreen.Settings())
                binding.drawerLayout.close()
            }
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
                        description.text = Lb.getTimestampHumanString(item.fileMetadata.lastModified)
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

        binding.suggestedDocsLayout.suggestedDocsList.setup {
            withDataSource(model.suggestedDocs)
            this.withLayoutManager(LinearLayoutManager(requireContext(), LinearLayoutManager.HORIZONTAL, false))

            withItem<SuggestedDocsViewHolderInfo, SuggestedDocsItemViewHolder>(R.layout.suggested_doc_item) {
                onBind(::SuggestedDocsItemViewHolder) { _, item ->
                    name.text = item.fileMetadata.name
                    folderName.text = getString(R.string.suggested_docs_parent_folder, item.folderName)
                    lastEdited.text = Lb.getTimestampHumanString(item.fileMetadata.lastModified)

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
        when (item.type) {
            FileType.Document -> {
                // TODO: consider that not all updates to the screen may go through because of postVal
                activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFile(item.id))
            }
            FileType.Folder -> {
                model.enterFolder(item)
            }
            FileType.Link -> {} // shouldn't happen
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

                    activityModel.launchActivityScreen(ActivityScreen.Settings(R.string.export_account_raw_key))
                }

                beforeYouStartDialog.findViewById<MaterialTextView>(R.id.before_you_start_description)!!.movementMethod = LinkMovementMethod.getInstance()

                beforeYouStartDialog.show()
                getApp().isNewAccount = false
            }
            UpdateFilesUI.SyncImport -> {
                (activity as MainScreenActivity).syncImportAccount()
            }
            is UpdateFilesUI.UpdateSideBarInfo -> {
                val header = binding.navigationView.getHeaderView(0)

                uiUpdates.usageMetrics?.let { usageMetrics ->
                    val dataCap = usageMetrics.dataCap.exact.toFloat()
                    val usage = usageMetrics.serverUsage.exact.toFloat()

                    val donut = header.findViewById<DonutProgressView>(R.id.filesListUsageDonut)
                    donut.cap = dataCap

                    val usageSection = DonutSection(
                        name = "",
                        color = ResourcesCompat.getColor(resources, R.color.md_theme_primary, null),
                        amount = usage
                    )

                    donut.submitData(listOf(usageSection))

                    header.findViewById<MaterialTextView>(R.id.filesListUsage).text = getString(R.string.free_space, usageMetrics.serverUsage.readable, usageMetrics.dataCap.readable)
                }

                uiUpdates.lastSynced?.let { lastSynced ->
                    header.findViewById<MaterialTextView>(R.id.filesListLastSynced).text = getString(R.string.last_sync, lastSynced)
                }

                uiUpdates.localDirtyFilesCount?.let { localDirtyFilesCount ->
                    header.findViewById<MaterialTextView>(R.id.filesListLocalDirty).text = resources.getQuantityString(R.plurals.files_to_push, localDirtyFilesCount, localDirtyFilesCount)
                }

                uiUpdates.serverDirtyFilesCount?.let { serverDirtyFilesCount ->
                    header.findViewById<MaterialTextView>(R.id.filesListServerDirty).text = resources.getQuantityString(R.plurals.files_to_pull, serverDirtyFilesCount, serverDirtyFilesCount)
                }

                uiUpdates.hasPendingShares?.let { hasPendingShares ->
                    header.findViewById<ImageView>(R.id.pending_shares_icon).setImageResource(
                        if (hasPendingShares) {
                            R.drawable.ic_outline_folder_shared_notif_24
                        } else {
                            R.drawable.ic_outline_folder_shared_24
                        }
                    )
                }
            }
            is UpdateFilesUI.ToggleSuggestedDocsVisibility -> {
                binding.suggestedDocsLayout.root.visibility = if (uiUpdates.show) View.VISIBLE else View.GONE
            }
            is UpdateFilesUI.OutOfSpace -> {
                val usageRatio = uiUpdates.progress.toFloat() / uiUpdates.max

                val (usageBarColor, msgId) = if (usageRatio >= 1.0) {
                    listOf(R.color.md_theme_error, R.string.out_of_space)
                } else {
                    val usageBarColor = if (usageRatio > 0.9) {
                        R.color.md_theme_error
                    } else {
                        R.color.md_theme_progressWarning
                    }

                    listOf(usageBarColor, R.string.running_out_of_space)
                }

                binding.outOfSpace.apply {
                    outOfSpaceMsg.setText(msgId)
                    outOfSpaceProgressBar.setIndicatorColor(ContextCompat.getColor(requireContext(), usageBarColor))
                    outOfSpaceProgressBar.progress = uiUpdates.progress
                    outOfSpaceProgressBar.max = uiUpdates.max
                    Animate.animateVisibility(root, View.VISIBLE, 255, 200)

                    outOfSpaceExit.setOnClickListener {
                        Animate.animateVisibility(root, View.GONE, 0, 200)

                        val pref = PreferenceManager
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
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_share)?.isVisible = true
            }
            else -> {
                if (actionModeMenu == null) {
                    actionModeMenu = menu.startActionMode(actionModeMenuCallback)
                }

                actionModeMenu?.title = getString(R.string.files_list_items_selected, selectionCount)
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_info)?.isVisible = false
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_rename)?.isVisible = false
                actionModeMenu?.menu?.findItem(R.id.menu_list_files_share)?.isVisible = false
            }
        }
    }

    override fun onBackPressed(): Boolean = when {
        binding.fabSpeedDial.isOpen -> {
            binding.fabSpeedDial.close()
            false
        }
        model.files.hasSelection() -> {
            unselectFiles()
            false
        }
        !model.fileModel.isAtRoot() -> {
            model.intoParentFolder()
            false
        }
        else -> {
            true
        }
    }

    override fun sync(usePreferences: Boolean) {
        if (!usePreferences || PreferenceManager.getDefaultSharedPreferences(requireContext())
            .getBoolean(
                    getString(
                            resources,
                            R.string.sync_automatically_key
                        ),
                    false
                )
        ) {
            workspaceModel._sync.postValue(Unit)
        }
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
    data class UpdateSideBarInfo(var usageMetrics: Usage? = null, var lastSynced: String? = null, var localDirtyFilesCount: Int? = null, var serverDirtyFilesCount: Int? = null, var hasPendingShares: Boolean? = null) : UpdateFilesUI()
    data class ToggleSuggestedDocsVisibility(var show: Boolean) : UpdateFilesUI()
    object ToggleMenuBar : UpdateFilesUI()
    object ShowBeforeWeStart : UpdateFilesUI()
    object SyncImport : UpdateFilesUI()
    data class OutOfSpace(val progress: Int, val max: Int) : UpdateFilesUI()
    data class NotifyWithSnackbar(val msg: String) : UpdateFilesUI()
}
