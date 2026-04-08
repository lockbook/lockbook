package app.lockbook.screen

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.*
import android.widget.LinearLayout
import androidx.appcompat.widget.PopupMenu
import androidx.core.content.ContextCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.LinearLayoutManager
import app.futured.donut.DonutProgressView
import app.futured.donut.DonutSection
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.FragmentFilesListBinding
import app.lockbook.model.*
import app.lockbook.model.MoveFileViewModel.Companion.PARENT_ID
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.viewholder.isSelected
import com.afollestad.recyclical.withItem
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.textview.MaterialTextView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference
import java.util.*

class FilesListFragment : Fragment(), FilesFragment {
    private var _binding: FragmentFilesListBinding? = null
    private val binding get() = _binding!!
    private val menu get() = binding.filesToolbar
    private var actionModeMenu: ActionMode? = null

    private var currentTab: WorkspaceTab = WorkspaceTab.Welcome
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

    private val model: FileTreeViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private val recyclerView get() = binding.filesList

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

        model.breadcrumbItems.observe(viewLifecycleOwner) {
            binding.filesBreadcrumbBar.setBreadCrumbItems(it)
        }

        binding.filesBreadcrumbBar.setListener(object : BreadCrumbItemClickListener {
            override fun onItemClick(breadCrumbItem: View, file: File) {
                model.enterFolder(file)
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
            when (it.id) {
                R.id.fab_create_drawing -> createDocAtParent(true)
                R.id.fab_create_document -> createDocAtParent(false)
                R.id.fab_create_folder -> activityModel.launchTransientScreen(
                    TransientScreen.Create(model.fileModel.parent.id)
                )
                else -> return@setOnActionSelectedListener false
            }

            binding.fabSpeedDial.close()
            true
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
            if (currentTab != it) {
                model.fileModel.idsAndFiles[it.id]?.let { child ->
                    model.fileModel.idsAndFiles[child.parent]?.let { parent ->
                        model.enterFolder(parent)
                    }
                }
            }
            currentTab = it
        }

        model.isSuggestedDocsVisible.observe(viewLifecycleOwner) {
            binding.suggestedDocsLayout.root.visibility = if (it) View.VISIBLE else View.GONE
        }

        val header = binding.navigationView.getHeaderView(0)
        val donut = header.findViewById<DonutProgressView>(R.id.filesListUsageDonut)

        val accentColor = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            ContextCompat.getColor(requireContext(), android.R.color.system_accent1_200)
        } else {
            ContextCompat.getColor(requireContext(), R.color.md_theme_primary)
        }

        model.usage.observe(viewLifecycleOwner) { usageMetrics ->
            usageMetrics?.let {
                val dataCap = it.dataCap?.exact?.toFloat() ?: 0f
                val usage = it.serverUsage?.exact?.toFloat() ?: 0f

                donut.cap = dataCap

                val usageSection = DonutSection(
                    name = "",
                    color = accentColor,
                    amount = usage
                )
                donut.submitData(listOf(usageSection))

                header.findViewById<MaterialTextView>(R.id.filesListUsage).text =
                    getString(R.string.free_space, usageMetrics.serverUsage?.readable, usageMetrics.dataCap?.readable)
            }
        }

        model.syncStatus.observe(viewLifecycleOwner) {
            header.findViewById<MaterialTextView>(R.id.filesListLastSynced).text =
                getString(R.string.last_sync, it)
        }

        model.dirtyLocally.observe(viewLifecycleOwner) {
            header.findViewById<MaterialTextView>(R.id.filesListLocalDirty).text =
                resources.getQuantityString(R.plurals.files_to_push, it.size, it.size)
        }

        model.pushingFiles.observe(viewLifecycleOwner) {
            header.findViewById<MaterialTextView>(R.id.filesListServerDirty).text =
                resources.getQuantityString(R.plurals.files_to_pull, it.size, it.size)
        }

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        setUpToolbar()

        (requireActivity().application as App).billingClientLifecycle.showInAppMessaging(requireActivity())
    }

    private fun createDocAtParent(isDrawing: Boolean) {
        workspaceModel._createDocAt.value = isDrawing to model.fileModel.parent.id
    }

    private fun setUpToolbar() {
        binding.filesToolbar.setNavigationOnClickListener {
            binding.drawerLayout.open()
        }

        binding.navigationView.getHeaderView(0).let { header ->

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
                R.id.menu_files_list_open_ws -> {
                    activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenWorkspacePane)
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
                            val background = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                                android.R.color.system_accent1_10
                            } else {
                                R.color.md_theme_inverseOnSurface
                            }
                            fileItemHolder.setBackgroundResource(background)
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
                    name.text = item.fileMetadata.getPrettyName()
                    if (item.fileMetadata.lastModified != 0L) {
                        description.visibility = View.VISIBLE
                        description.text = Lb.getTimestampHumanString(item.fileMetadata.lastModified)
                    } else {
                        description.visibility = View.GONE
                    }

                    icon.setImageResource(item.fileMetadata.getIconResource())

                    when {
                        isSelected() -> {
                            val background = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                                android.R.color.system_accent1_10
                            } else {
                                R.color.md_theme_inverseOnSurface
                            }
                            fileItemHolder.setBackgroundResource(background)
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

        binding.suggestedDocsLayout.clearAllBtn.setOnClickListener {
            model.suggestedDocs.clear()
            Lb.clearSuggested()
            lifecycleScope.launch {
                model.maybeToggleSuggestedDocs()
            }
        }

        binding.suggestedDocsLayout.suggestedDocsList.setup {
            withDataSource(model.suggestedDocs)
            this.withLayoutManager(LinearLayoutManager(requireContext(), LinearLayoutManager.HORIZONTAL, false))

            withItem<SuggestedDocsViewHolderInfo, SuggestedDocsItemViewHolder>(R.layout.suggested_doc_item) {
                onBind(::SuggestedDocsItemViewHolder) { i, item ->
                    name.text = item.fileMetadata.getPrettyName()
                    folderName.text = getString(R.string.suggested_docs_parent_folder, item.folderName)
                    lastEdited.text = Lb.getTimestampHumanString(item.fileMetadata.lastModified)

                    icon.setImageResource(item.fileMetadata.getIconResource())

                    itemView.setOnLongClickListener { view ->
                        val popup = PopupMenu(view.context, view)

                        popup.menu.add(0, 1, 0, "Remove")

                        popup.setOnMenuItemClickListener { menuItem ->
                            Lb.clearSuggestedId(item.fileMetadata.id)
                            model.suggestedDocs.removeAt(i)
                            model.reloadFiles()
                            true
                        }

                        popup.show()
                        true
                    }
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
                model._breadcrumbItems.value = getBreadcrumbItems()
            }
            UpdateFilesUI.ToggleMenuBar -> toggleMenuBar()
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

                val (usageBarColor, msgId) = if (usageRatio >= 1.0) {
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

    private fun getUsageColor(usageRatio: Float): Int {
        return when {
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
    }

    private fun getBreadcrumbItems(): MutableList<BreadCrumbItem> {
        return model.fileModel.getFileDir().map { BreadCrumbItem(it) }.toMutableList()
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
            model.enterFolder(null)
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
    object UpdateBreadcrumbBar : UpdateFilesUI()
    data class NotifyError(val error: LbError) : UpdateFilesUI()
    object ToggleMenuBar : UpdateFilesUI()
    object RequestSync : UpdateFilesUI()
    object SyncImport : UpdateFilesUI()
    data class OutOfSpace(val progress: Int, val max: Int) : UpdateFilesUI()
    data class NotifyWithSnackbar(val msg: String) : UpdateFilesUI()
}

fun File.getPrettyName(): String {
    return if (this.type == FileType.Document && this.id != PARENT_ID) {
        // todo: consider removing the extension
        this.name
    } else {
        this.name
    }
}
