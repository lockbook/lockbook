package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.*
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import androidx.core.content.res.ResourcesCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.databinding.FragmentFilesBinding
import app.lockbook.model.*
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.SelectableDataSource
import com.afollestad.recyclical.datasource.selectableDataSourceTypedOf
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.viewholder.isSelected
import com.afollestad.recyclical.withItem
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import timber.log.Timber
import java.io.File
import java.lang.ref.WeakReference
import java.util.*

class FilesFragment : Fragment() {
    private var _binding: FragmentFilesBinding? = null
    private val binding get() = _binding!!
    private val menu get() = binding.filesToolbar

    private val model: FilesViewModel by viewModels()
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
        _binding = FragmentFilesBinding.inflate(inflater, container, false)

        binding.filesToolbar.title = "Lockbook"
        binding.filesToolbar.inflateMenu(R.menu.menu_list_files)
        binding.filesToolbar.setOnMenuItemClickListener { item ->
            val selectedFiles = model.selectableFiles

            when (item.itemId) {
                R.id.menu_list_files_settings -> startActivity(
                    Intent(
                        context,
                        SettingsActivity::class.java
                    )
                )
                R.id.menu_list_files_sort_last_changed -> model.changeFileSort(SortStyle.LastChanged)
                R.id.menu_list_files_sort_a_z -> model.changeFileSort(SortStyle.AToZ)
                R.id.menu_list_files_sort_z_a -> model.changeFileSort(SortStyle.ZToA)
                R.id.menu_list_files_sort_first_changed -> model.changeFileSort(SortStyle.FirstChanged)
                R.id.menu_list_files_sort_type -> model.changeFileSort(SortStyle.FileType)
                R.id.menu_list_files_rename -> {
                    if (selectedFiles.getSelectionCount() == 1) {
                        activityModel.launchTransientScreen(TransientScreen.Rename(selectedFiles[0]))
                    }
                }
                R.id.menu_list_files_delete -> {
                    model.deleteSelectedFiles()
                }
                R.id.menu_list_files_info -> {
                    if (model.selectableFiles.getSelectionCount() == 1) {
                        activityModel.launchTransientScreen(TransientScreen.Info(selectedFiles[0]))
                    }
                }
                R.id.menu_list_files_move -> {
                    activityModel.launchTransientScreen(
                        TransientScreen.Move(
                            selectedFiles.getSelectedItems().map { it.id }.toTypedArray()
                        )
                    )
                }
                R.id.menu_list_files_share -> {
                    model.shareSelectedFiles(requireActivity().cacheDir)
                }
            }

            selectedFiles.deselectAll()

            true
        }

        model.notifyUpdateFilesUI.observe(
            viewLifecycleOwner,
            { uiUpdates ->
                when (uiUpdates) {
                    is UpdateFilesUI.NotifyError -> alertModel.notifyError(uiUpdates.error)
                    is UpdateFilesUI.NotifyWithSnackbar -> alertModel.notify(uiUpdates.msg)
                    UpdateFilesUI.ShowSyncSnackBar -> {
                        snackProgressBarManager.dismiss()
                        syncSnackProgressBar.setMessage(resources.getString(R.string.list_files_sync_snackbar_default))
                        snackProgressBarManager.show(
                            syncSnackProgressBar,
                            SnackProgressBarManager.LENGTH_INDEFINITE
                        )
                    }
                    UpdateFilesUI.StopProgressSpinner -> binding.listFilesRefresh.isRefreshing =
                        false
                    is UpdateFilesUI.UpdateBreadcrumbBar -> binding.filesBreadcrumbBar.setBreadCrumbItems(
                        uiUpdates.breadcrumbItems.toMutableList()
                    )
                    is UpdateFilesUI.UpdateSyncSnackBar -> {
                        syncSnackProgressBar.setProgressMax(uiUpdates.total)
                        snackProgressBarManager.setProgress(uiUpdates.progress)
                        syncSnackProgressBar.setMessage(
                            resources.getString(
                                R.string.list_files_sync_snackbar,
                                uiUpdates.total.toString()
                            )
                        )
                        snackProgressBarManager.updateTo(syncSnackProgressBar)
                    }
                    is UpdateFilesUI.ShareDocuments -> finalizeShare(uiUpdates.files)
                    is UpdateFilesUI.ShowHideProgressOverlay -> {
                        val progressOverlay =
                            (activity as MainScreenActivity).binding.progressOverlay.root

                        if (progressOverlay.visibility == View.GONE) {
                            Animate.animateVisibility(progressOverlay, View.VISIBLE, 102, 500)
                        } else {
                            Animate.animateVisibility(progressOverlay, View.GONE, 0, 500)
                        }
                    }
                }.exhaustive
            }
        )

        recyclerView.setup {
            withDataSource(model.selectableFiles)
            withEmptyView(binding.listFilesFrameLayout.findViewById(R.id.files_empty_folder)!!)

            withItem<ClientFileMetadata, HorizontalViewHolder>(R.layout.linear_layout_file_item) {
                onBind(::HorizontalViewHolder) { _, item ->
                    name.text = item.name
                    description.text = resources.getString(
                        R.string.last_synced,
                        CoreModel.convertToHumanDuration(item.metadataVersion)
                    )

                    when {
                        isSelected() -> {
                            icon.setImageResource(R.drawable.ic_baseline_check_24)
                        }
                        item.fileType == FileType.Document && item.name.endsWith(".draw") -> {
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
                        val detailsScreen = when (item.fileType) {
                            FileType.Document -> {
                                if (item.name.endsWith(".draw")) {
                                    DetailsScreen.Drawing(item)
                                } else {
                                    DetailsScreen.TextEditor(item)
                                }
                            }
                            FileType.Folder -> {
                                model.enterFolder(item)
                                return@onClick
                            }
                        }

                        activityModel.launchDetailsScreen(detailsScreen)
                    }

                }
                onLongClick {
                    this.toggleSelection()
                    toggleMenuBar()
                }
            }
        }

        when (
            val optionValue = PreferenceManager.getDefaultSharedPreferences(context).getString(
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

    private fun toggleMenuBar() {
        when (model.selectableFiles.getSelectionCount()) {
            0 -> {
                for (menuItem in menuItemsOneOrMoreSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = true
                }

                for (menuItem in menuItemsOneSelected) {
                    menu.menu.findItem(menuItem)!!.isVisible = true
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

    private fun finalizeShare(files: List<File>) {
        val onShare =
            requireActivity().registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {

            }

        val uris = ArrayList<Uri>()

        for (file in files) {
            uris.add(
                FileProvider.getUriForFile(
                    requireActivity(),
                    "app.lockbook.fileprovider",
                    file
                )
            )
        }

        val intent = Intent(Intent.ACTION_SEND_MULTIPLE)
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)

        val clipData = ClipData.newRawUri(null, Uri.EMPTY)
        uris.forEach { uri ->
            clipData.addItem(ClipData.Item(uri))
        }

        intent.clipData = clipData
        intent.type = "*/*"
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        intent.putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris)

        onShare.launch(
            Intent.createChooser(
                intent,
                "Send multiple files."
            )
        )
    }

    fun onBackPressed(): Boolean = when {
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
}

sealed class UpdateFilesUI {
    data class ShowHideProgressOverlay(val hide: Boolean) : UpdateFilesUI()
    data class ShareDocuments(val files: ArrayList<File>) : UpdateFilesUI()
    data class UpdateBreadcrumbBar(val breadcrumbItems: List<BreadCrumbItem>) : UpdateFilesUI()
    data class NotifyError(val error: LbError) : UpdateFilesUI()
    object ShowSyncSnackBar : UpdateFilesUI()
    object StopProgressSpinner : UpdateFilesUI()
    data class UpdateSyncSnackBar(val total: Int, val progress: Int) : UpdateFilesUI()
    data class NotifyWithSnackbar(val msg: String) : UpdateFilesUI()
}

private val menuItemsNoneSelected = listOf<Int>(
//    R.id.menu_list_files_sort,
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