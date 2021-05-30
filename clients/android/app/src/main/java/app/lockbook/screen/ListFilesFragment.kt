package app.lockbook.screen

import android.content.Intent
import android.content.res.Configuration.*
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout.HORIZONTAL
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.lifecycle.ViewModelProvider
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.FragmentListFilesBinding
import app.lockbook.model.*
import app.lockbook.modelfactory.ListFilesViewModelFactory
import app.lockbook.screen.RequestResultCodes.DRAWING_REQUEST_CODE
import app.lockbook.screen.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import app.lockbook.ui.*
import app.lockbook.util.*
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import java.util.*

class ListFilesFragment : Fragment() {
    lateinit var listFilesViewModel: ListFilesViewModel
    private var _binding: FragmentListFilesBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private var updatedLastSyncedDescription = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))
    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            if (f is CreateFileDialogFragment) {
                listFilesViewModel.refreshFiles(f.newDocument)
            } else {
                listFilesViewModel.refreshFiles(null)
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

    private val syncInfoSnackBar by lazy {
        SnackProgressBar(
            SnackProgressBar.TYPE_NORMAL,
            ""
        )
            .setSwipeToDismiss(true)
            .setAllowUserInput(true)
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DataBindingUtil.inflate(
            inflater,
            R.layout.fragment_list_files,
            container,
            false
        )

        val application = requireNotNull(this.activity).application
        val filesDir = application.filesDir.absolutePath
        val listFilesViewModelFactory =
            ListFilesViewModelFactory(filesDir, application)
        listFilesViewModel =
            ViewModelProvider(this, listFilesViewModelFactory).get(ListFilesViewModel::class.java)
        LinearRecyclerViewAdapter(listFilesViewModel, filesDir)

        binding.listFilesViewModel = listFilesViewModel
        var adapter = setFileAdapter(binding, filesDir)
        binding.lifecycleOwner = this

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
                listFilesViewModel.refreshFiles(null)
                adapter = setFileAdapter(binding, filesDir)
            }
        )

        listFilesViewModel.switchMenu.observe(
            viewLifecycleOwner,
            {
                moreOptionsMenu()
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

        listFilesViewModel.showSnackBar.observe(
            viewLifecycleOwner,
            { msg ->
                if (container != null) {
                    snackProgressBarManager.dismiss()
                    AlertModel.notify(container, msg, OnFinishAlert.DoNothingOnFinishAlert)
                }
            }
        )

        listFilesViewModel.updateSyncSnackBar.observe(
            viewLifecycleOwner,
            { progressAndTotal ->
                updateProgressSnackBar(progressAndTotal.first, progressAndTotal.second)
            }
        )

        listFilesViewModel.errorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                if (container != null) {
                    AlertModel.errorHasOccurred(container, errorText, OnFinishAlert.DoNothingOnFinishAlert)
                }
            }
        )

        listFilesViewModel.unexpectedErrorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                AlertModel.unexpectedCoreErrorHasOccurred(requireContext(), errorText, OnFinishAlert.DoNothingOnFinishAlert)
            }
        )

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        binding.filesBreadcrumbBar.setListener(object : BreadCrumbItemClickListener {
            override fun onItemClick(breadCrumbItem: View, position: Int) {
                listFilesViewModel.handleRefreshAtParent(position)
            }
        })

        snackProgressBarManager.useRoundedCornerBackground(true)

        if (resources.configuration.orientation == ORIENTATION_LANDSCAPE && resources.configuration.screenLayout == SCREENLAYOUT_SIZE_SMALL) {
            binding.listFilesFabHolder.orientation = HORIZONTAL
        }
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

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        listFilesViewModel.onOpenedActivityEnd()
    }

    override fun onActivityCreated(savedInstanceState: Bundle?) {
        super.onActivityCreated(savedInstanceState)
        setUpAfterConfigChange()
    }

    private fun setFileAdapter(binding: FragmentListFilesBinding, filesDir: String): GeneralViewAdapter {
        val config = resources.configuration

        val fileLayoutPreference = PreferenceManager.getDefaultSharedPreferences(App.instance)
            .getString(
                SharedPreferences.FILE_LAYOUT_KEY,
                if (config.isLayoutSizeAtLeast(SCREENLAYOUT_SIZE_LARGE) || (config.screenWidthDp >= 480 && config.screenHeightDp >= 640)) {
                    SharedPreferences.GRID_LAYOUT
                } else {
                    SharedPreferences.LINEAR_LAYOUT
                }
            )

        if (fileLayoutPreference == SharedPreferences.LINEAR_LAYOUT) {
            val adapter = LinearRecyclerViewAdapter(listFilesViewModel, filesDir)
            binding.filesList.adapter = adapter
            binding.filesList.layoutManager = LinearLayoutManager(context)
            return adapter
        } else {
            val orientation = config.orientation
            val adapter = GridRecyclerViewAdapter(listFilesViewModel)
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
        adapter.selectedFiles = MutableList(listFilesViewModel.files.value?.size ?: 0) { false }
    }

    private fun setUpAfterConfigChange() {
        collapseExpandFAB(listFilesViewModel.isFABOpen)

        if (listFilesViewModel.syncModel.syncStatus is SyncStatus.IsSyncing) {
            val status = listFilesViewModel.syncModel.syncStatus as SyncStatus.IsSyncing
            showSyncSnackBar()
            updateProgressSnackBar(status.total, status.progress)
        }

        parentFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
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

    private fun showSyncInfoSnackBar(amountToSync: Int) {
        snackProgressBarManager.dismiss()
        if (amountToSync == 0) {
            syncInfoSnackBar.setMessage(resources.getString(R.string.list_files_sync_finished_snackbar))
            snackProgressBarManager.show(syncInfoSnackBar, SnackProgressBarManager.LENGTH_SHORT)
        } else {
            syncInfoSnackBar.setMessage(
                resources.getString(
                    R.string.list_files_sync_info_snackbar,
                    amountToSync.toString()
                )
            )
            snackProgressBarManager.show(syncInfoSnackBar, SnackProgressBarManager.LENGTH_SHORT)
        }
    }

    private fun collapseExpandFAB(isFABOpen: Boolean) {
        if (isFABOpen) {
            showFABMenu()
        } else {
            closeFABMenu()
        }
    }

    private fun closeFABMenu() {
        binding.listFilesFab.animate().setDuration(200L).rotation(90f)
        binding.listFilesFab.setImageResource(R.drawable.ic_baseline_add_24)
        binding.listFilesFabFolder.hide()
        binding.listFilesFabDocument.hide()
        binding.listFilesFabDrawing.hide()
        binding.listFilesRefresh.alpha = 1f
        binding.listFilesFrameLayout.isClickable = false
    }

    private fun showFABMenu() {
        binding.listFilesFab.animate().setDuration(200L).rotation(-90f)
        binding.listFilesFabFolder.show()
        binding.listFilesFabDocument.show()
        binding.listFilesFabDrawing.show()
        binding.listFilesRefresh.alpha = 0.7f
        binding.listFilesFrameLayout.isClickable = true
        binding.listFilesFrameLayout.setOnClickListener {
            listFilesViewModel.collapseExpandFAB()
        }
    }

    private fun updateFilesList(
        files: List<FileMetadata>,
        adapter: GeneralViewAdapter
    ) {
        adapter.files = files
        if (!listFilesViewModel.selectedFiles.contains(true)) {
            listFilesViewModel.selectedFiles = MutableList(files.size) { false }
        }

        adapter.selectedFiles = listFilesViewModel.selectedFiles.toMutableList()
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
        startActivityForResult(intent, TEXT_EDITOR_REQUEST_CODE)
    }

    private fun moreOptionsMenu() {
        if (activity is ListFilesActivity) {
            (activity as ListFilesActivity).switchMenu()
        } else {
            AlertModel.errorHasOccurred(binding.fragmentListFiles, BASIC_ERROR, OnFinishAlert.DoNothingOnFinishAlert)
        }
    }

    private fun navigateToDrawing(editableFile: EditableFile) {
        val intent = Intent(context, DrawingActivity::class.java)
        intent.putExtra("id", editableFile.id)
        startActivityForResult(intent, DRAWING_REQUEST_CODE)
    }

    private fun showMoreInfoDialog(fileMetadata: FileMetadata) {
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
