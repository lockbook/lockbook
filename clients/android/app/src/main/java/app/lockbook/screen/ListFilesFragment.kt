package app.lockbook.screen

import android.content.ClipData
import android.content.Intent
import android.content.res.Configuration.*
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.lifecycle.ViewModelProvider
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentListFilesBinding
import app.lockbook.model.*
import app.lockbook.modelfactory.ListFilesViewModelFactory
import app.lockbook.ui.*
import app.lockbook.util.*
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import timber.log.Timber
import java.io.File
import java.lang.ref.WeakReference
import java.util.*

class ListFilesFragment : Fragment() {
    lateinit var listFilesViewModel: ListFilesViewModel
    private var _binding: FragmentListFilesBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private var onActivityResult =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            listFilesViewModel.onOpenedActivityEnd()
        }

    private var onShareResult =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            getListFilesActivity().showHideProgressOverlay(false)
            listFilesViewModel.shareModel.isLoadingOverlayVisible = false
        }

    private var updatedLastSyncedDescription = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))
    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            if (f is CreateFileDialogFragment) {
                listFilesViewModel.onCreateFileDialogEnded(f.newDocument)
            } else {
                listFilesViewModel.onCreateFileDialogEnded(null)
            }
        }
    }
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
        _binding = FragmentListFilesBinding.inflate(
            inflater,
            container,
            false
        )

        val application = requireNotNull(this.activity).application
        val listFilesViewModelFactory =
            ListFilesViewModelFactory(application, getListFilesActivity().isThisAnImport())
        listFilesViewModel =
            ViewModelProvider(this, listFilesViewModelFactory).get(ListFilesViewModel::class.java)
        LinearRecyclerViewAdapter(listFilesViewModel)

        var adapter = setFileAdapter()

        binding.listFilesRefresh.setOnRefreshListener {
            listFilesViewModel.onSwipeToRefresh()
        }

        updatedLastSyncedDescription.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
                        adapter.notifyDataSetChanged()
                    }
                }
            },
            30000,
            30000
        )

        binding.fabsNewFile.listFilesFab.setOnClickListener {
            listFilesViewModel.collapseExpandFAB()
        }

        binding.fabsNewFile.listFilesFabFolder.setOnClickListener {
            listFilesViewModel.onNewFolderFABClicked()
        }

        binding.fabsNewFile.listFilesFabDocument.setOnClickListener {
            listFilesViewModel.onNewDocumentFABClicked(false)
        }

        binding.fabsNewFile.listFilesFabDrawing.setOnClickListener {
            listFilesViewModel.onNewDocumentFABClicked(true)
        }

        listFilesViewModel.files.observe(
            viewLifecycleOwner,
            { files ->
                updateFilesList(files, adapter)
            }
        )

        listFilesViewModel.stopProgressSpinner.observe(
            viewLifecycleOwner,
            {
                binding.listFilesRefresh.isRefreshing = false
            }
        )

        listFilesViewModel.showSyncSnackBar.observe(
            viewLifecycleOwner,
            {
                showSyncSnackBar()
            }
        )

        listFilesViewModel.navigateToDrawing.observe(
            viewLifecycleOwner,
            { editableFile ->
                navigateToDrawing(editableFile)
            }
        )

        listFilesViewModel.navigateToFileEditor.observe(
            viewLifecycleOwner,
            { editableFile ->
                navigateToFileEditor(editableFile)
            }
        )

        listFilesViewModel.switchFileLayout.observe(
            viewLifecycleOwner,
            {
                adapter = setFileAdapter(adapter)
            }
        )

        listFilesViewModel.expandCloseMenu.observe(
            viewLifecycleOwner,
            { expandOrNot ->
                moreOptionsMenu(expandOrNot)
            }
        )

        listFilesViewModel.collapseExpandFAB.observe(
            viewLifecycleOwner,
            { isFABOpen ->
                collapseExpandFAB(isFABOpen)
            }
        )

        listFilesViewModel.showCreateFileDialog.observe(
            viewLifecycleOwner,
            { createFileInfo ->
                showCreateFileDialog(createFileInfo)
            }
        )

        listFilesViewModel.showRenameFileDialog.observe(
            viewLifecycleOwner,
            { renameFileInfo ->
                showRenameFileDialog(renameFileInfo)
            }
        )

        listFilesViewModel.showFileInfoDialog.observe(
            viewLifecycleOwner,
            { fileMetadata ->
                showMoreInfoDialog(fileMetadata)
            }
        )

        listFilesViewModel.showMoveFileDialog.observe(
            viewLifecycleOwner,
            { moveFileInfo ->
                showMoveFileDialog(moveFileInfo)
            }
        )

        listFilesViewModel.uncheckAllFiles.observe(
            viewLifecycleOwner,
            {
                unSelectAllFiles(adapter)
            }
        )

        listFilesViewModel.updateBreadcrumbBar.observe(
            viewLifecycleOwner,
            { path ->
                binding.filesBreadcrumbBar.setBreadCrumbItems(path.toMutableList())
            }
        )

        listFilesViewModel.notifyWithSnackbar.observe(
            viewLifecycleOwner,
            { msg ->
                if (container != null) {
                    snackProgressBarManager.dismiss()
                    alertModel.notify(msg)
                }
            }
        )

        listFilesViewModel.shareDocument.observe(
            viewLifecycleOwner,
            { files ->
                shareDocuments(files)
            }
        )

        listFilesViewModel.updateSyncSnackBar.observe(
            viewLifecycleOwner,
            { progressAndTotal ->
                updateProgressSnackBar(progressAndTotal.first, progressAndTotal.second)
            }
        )

        listFilesViewModel.showHideProgressOverlay.observe(
            viewLifecycleOwner,
            { show ->
                showHideProgressOverlay(show)
            }
        )

        listFilesViewModel.notifyError.observe(
            viewLifecycleOwner,
            { error ->
                if (container != null) {
                    alertModel.notifyError(error)
                }
            }
        )

        return binding.root
    }

    private fun showHideProgressOverlay(show: Boolean) {
        if (show) {
            listFilesViewModel.collapseMoreOptionsMenu()
        }
        getListFilesActivity().showHideProgressOverlay(show)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        binding.filesBreadcrumbBar.setListener(object : BreadCrumbItemClickListener {
            override fun onItemClick(breadCrumbItem: View, position: Int) {
                listFilesViewModel.refreshAtPastParent(position)
            }
        })

        snackProgressBarManager.useRoundedCornerBackground(true)

        setUpAfterConfigChange()
    }

    override fun onDestroy() {
        super.onDestroy()
        parentFragmentManager.unregisterFragmentLifecycleCallbacks(fragmentFinishedCallback)
    }

    fun onBackPressed(): Boolean {
        return listFilesViewModel.onBackPress()
    }

    fun onMenuItemPressed(id: Int) {
        listFilesViewModel.onMenuItemPressed(id)
    }

    private fun setFileAdapter(oldAdapter: GeneralViewAdapter? = null): GeneralViewAdapter {
        if (binding.filesList.adapter is GeneralViewAdapter) {
            Timber.e("SET FILE ADAPTER: ${oldAdapter?.files?.map { it.name }}")

            Timber.e("CURRENT: ${(binding.filesList.adapter as GeneralViewAdapter).files.map { it.name }}")
        }
        val deviceConfig = resources.configuration

        val linearLayoutValue = getString(R.string.file_layout_linear_value)
        val gridLayoutValue = getString(R.string.file_layout_grid_value)

        val fileLayoutPreference = PreferenceManager
            .getDefaultSharedPreferences(context)
            .getString(
                getString(R.string.file_layout_key),
                if (deviceConfig.isLayoutSizeAtLeast(SCREENLAYOUT_SIZE_LARGE) || (deviceConfig.screenWidthDp >= 480 && deviceConfig.screenHeightDp >= 640)) {
                    gridLayoutValue
                } else {
                    linearLayoutValue
                }
            )

        if (fileLayoutPreference == linearLayoutValue) {
            val adapter = LinearRecyclerViewAdapter(listFilesViewModel)
            if (oldAdapter != null) {
                adapter.files = oldAdapter.files
            }

            binding.filesList.adapter = adapter
            binding.filesList.layoutManager = LinearLayoutManager(context)
            return adapter
        } else {
            val orientation = deviceConfig.orientation
            val adapter = GridRecyclerViewAdapter(listFilesViewModel)
            if (oldAdapter != null) {
                adapter.files = oldAdapter.files
            }
            binding.filesList.adapter = adapter

            val displayMetrics = resources.displayMetrics
            val noOfColumns = (((displayMetrics.widthPixels / displayMetrics.density) / 90)).toInt()

            if (orientation == ORIENTATION_PORTRAIT) {
                binding.filesList.layoutManager = GridLayoutManager(context, noOfColumns)
            } else {
                binding.filesList.layoutManager = GridLayoutManager(context, noOfColumns)
            }

            return adapter
        }
    }

    private fun unSelectAllFiles(adapter: GeneralViewAdapter) {
        adapter.clearSelectionMode()
    }

    private fun setUpAfterConfigChange() {
        collapseExpandFAB(listFilesViewModel.isFABOpen)

        val syncStatus = listFilesViewModel.syncModel.syncStatus
        if (syncStatus is SyncStatus.IsSyncing) {
            showSyncSnackBar()
            updateProgressSnackBar(syncStatus.total, syncStatus.progress)
        }

        val isLoadingOverlayVisible = listFilesViewModel.shareModel.isLoadingOverlayVisible
        if (isLoadingOverlayVisible) {
            showHideProgressOverlay(isLoadingOverlayVisible)
        }

        parentFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )
    }

    private fun shareDocuments(files: ArrayList<File>) {
        val uris = ArrayList<Uri>()

        for (file in files) {
            uris.add(
                FileProvider.getUriForFile(
                    requireContext(),
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

        onShareResult.launch(
            Intent.createChooser(
                intent,
                "Send multiple files."
            )
        )
    }

    private fun updateProgressSnackBar(total: Int, progress: Int) {
        syncSnackProgressBar.setProgressMax(total)
        snackProgressBarManager.setProgress(progress)
        syncSnackProgressBar.setMessage(
            resources.getString(
                R.string.list_files_sync_snackbar,
                total.toString()
            )
        )
        snackProgressBarManager.updateTo(syncSnackProgressBar)
    }

    private fun showSyncSnackBar() {
        snackProgressBarManager.dismiss()
        syncSnackProgressBar.setMessage(resources.getString(R.string.list_files_sync_snackbar_default))
        snackProgressBarManager.show(
            syncSnackProgressBar,
            SnackProgressBarManager.LENGTH_INDEFINITE
        )
    }

    private fun collapseExpandFAB(isFABOpen: Boolean) {
        if (isFABOpen) {
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
            listFilesViewModel.collapseExpandFAB()
        }
    }

    private fun updateFilesList(
        files: List<ClientFileMetadata>,
        adapter: GeneralViewAdapter
    ) {
        adapter.files = files
        adapter.selectedFiles = listFilesViewModel.selectedFiles.toMutableList()
        if (adapter.selectedFiles.isNotEmpty()) {
            adapter.selectionMode = true
        }

        if (files.isEmpty()) {
            binding.listFilesEmptyFolder.visibility = View.VISIBLE
        } else if (files.isNotEmpty() && binding.listFilesEmptyFolder.visibility == View.VISIBLE) {
            binding.listFilesEmptyFolder.visibility = View.GONE
        }
    }

    private fun navigateToFileEditor(editableFile: EditableFile) {
        val intent = Intent(context, TextEditorActivity::class.java)
        intent.putExtra("name", editableFile.name)
        intent.putExtra("id", editableFile.id)
        onActivityResult.launch(intent)
    }

    private fun moreOptionsMenu(expandOrNot: Boolean) {
        getListFilesActivity().switchMenu(expandOrNot)
    }

    private fun getListFilesActivity(): ListFilesActivity {
        return activity as ListFilesActivity
    }

    private fun navigateToDrawing(editableFile: EditableFile) {
        val intent = Intent(context, DrawingActivity::class.java)
        intent.putExtra("id", editableFile.id)
        onActivityResult.launch(intent)
    }

    private fun showMoreInfoDialog(fileMetadata: ClientFileMetadata) {
        val dialogFragment = FileInfoDialogFragment.newInstance(
            fileMetadata.name,
            fileMetadata.id,
            fileMetadata.metadataVersion.toString(),
            fileMetadata.contentVersion.toString(),
            fileMetadata.fileType.name
        )

        dialogFragment.show(childFragmentManager, FileInfoDialogFragment.FILE_INFO_DIALOG_TAG)
    }

    private fun showMoveFileDialog(moveFileInfo: MoveFileInfo) {
        val dialogFragment = MoveFileDialogFragment.newInstance(
            moveFileInfo.ids,
            moveFileInfo.names
        )

        dialogFragment.show(parentFragmentManager, RenameFileDialogFragment.RENAME_FILE_DIALOG_TAG)
    }

    private fun showRenameFileDialog(renameFileInfo: RenameFileInfo) {
        val dialogFragment = RenameFileDialogFragment.newInstance(
            renameFileInfo.id,
            renameFileInfo.name
        )

        dialogFragment.show(parentFragmentManager, MoveFileDialogFragment.MOVE_FILE_DIALOG_TAG)
    }

    private fun showCreateFileDialog(createFileInfo: CreateFileInfo) {
        val dialogFragment = CreateFileDialogFragment.newInstance(
            createFileInfo.parentId,
            createFileInfo.fileType,
            createFileInfo.isDrawing
        )

        dialogFragment.show(parentFragmentManager, CreateFileDialogFragment.CREATE_FILE_DIALOG_TAG)
    }
}
