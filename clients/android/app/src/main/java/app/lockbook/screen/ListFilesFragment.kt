package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.app.AlertDialog
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentListFilesBinding
import app.lockbook.model.FilesAdapter
import app.lockbook.model.ListFilesViewModel
import app.lockbook.modelfactory.ListFilesViewModelFactory
import app.lockbook.ui.CreateFileDialogFragment
import app.lockbook.ui.FileInfoDialogFragment
import app.lockbook.ui.MoveFileDialogFragment
import app.lockbook.ui.RenameFileDialogFragment
import app.lockbook.util.*
import app.lockbook.util.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.util.Messages.UNEXPECTED_ERROR
import app.lockbook.util.RequestResultCodes.HANDWRITING_EDITOR_REQUEST_CODE
import app.lockbook.util.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import com.google.android.material.snackbar.Snackbar
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import kotlinx.android.synthetic.main.fragment_list_files.*

class ListFilesFragment : Fragment() {
    lateinit var listFilesViewModel: ListFilesViewModel
    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            listFilesViewModel.refreshAndAssessChanges()
        }
    }
    private val snackProgressBarManager by lazy {
        SnackProgressBarManager(
            requireView(),
            lifecycleOwner = this
        ).setViewToMove(list_files_frame_layout)
    }

    private val syncSnackProgressBar by lazy {
        SnackProgressBar(
            SnackProgressBar.TYPE_HORIZONTAL,
            resources.getString(R.string.list_files_sync_snackbar, "n")
        )
            .setIsIndeterminate(false)
            .setSwipeToDismiss(false)
            .setAllowUserInput(true)
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val binding: FragmentListFilesBinding = DataBindingUtil.inflate(
            inflater,
            R.layout.fragment_list_files,
            container,
            false
        )
        val application = requireNotNull(this.activity).application
        val listFilesViewModelFactory =
            ListFilesViewModelFactory(application.filesDir.absolutePath, application)
        listFilesViewModel =
            ViewModelProvider(this, listFilesViewModelFactory).get(ListFilesViewModel::class.java)
        val adapter =
            FilesAdapter(listFilesViewModel)

        binding.listFilesViewModel = listFilesViewModel
        binding.filesList.adapter = adapter
        binding.filesList.layoutManager = LinearLayoutManager(context)
        binding.lifecycleOwner = this

        binding.listFilesRefresh.setOnRefreshListener {
            listFilesViewModel.onSwipeToRefresh()
        }

        listFilesViewModel.files.observe(
            viewLifecycleOwner,
            { files ->
                updateRecyclerView(files, adapter)
            }
        )

        listFilesViewModel.stopSyncSnackBar.observe(
            viewLifecycleOwner,
            {
                earlyStopSyncSnackBar()
            }
        )

        listFilesViewModel.stopProgressSpinner.observe(
            viewLifecycleOwner,
            {
                list_files_refresh.isRefreshing = false
            }
        )

        listFilesViewModel.showSyncSnackBar.observe(
            viewLifecycleOwner,
            { maxProgress ->
                showSyncSnackBar(maxProgress)
            }
        )

        listFilesViewModel.showPreSyncSnackBar.observe(
            viewLifecycleOwner,
            { amountToSync ->
                showPreSyncSnackBar(amountToSync)
            }
        )

        listFilesViewModel.showOfflineSnackBar.observe(
            viewLifecycleOwner,
            {
                showOfflineSnackBar()
            }
        )

        listFilesViewModel.updateProgressSnackBar.observe(
            viewLifecycleOwner,
            { progress ->
                updateProgressSnackBar(progress)
            }
        )

        listFilesViewModel.navigateToHandwritingEditor.observe(
            viewLifecycleOwner,
            { editableFile ->
                navigateToHandwritingEditor(editableFile)
            }
        )

        listFilesViewModel.navigateToFileEditor.observe(
            viewLifecycleOwner,
            { editableFile ->
                navigateToFileEditor(editableFile)
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

        listFilesViewModel.showSuccessfulDeletion.observe(
            viewLifecycleOwner,
            {
                if (container != null) {
                    showSuccessfulDeletionSnackbar(container)
                }
            }
        )

        listFilesViewModel.fileModelErrorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                if (container != null) {
                    errorHasOccurred(container, errorText)
                }
            }
        )

        listFilesViewModel.errorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                if (container != null) {
                    errorHasOccurred(container, errorText)
                }
            }
        )

        listFilesViewModel.unexpectedErrorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                unexpectedErrorHasOccurred(errorText)
            }
        )

        listFilesViewModel.fileModeUnexpectedErrorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                unexpectedErrorHasOccurred(errorText)
            }
        )

        return binding.root
    }

    private fun unSelectAllFiles(adapter: FilesAdapter) {
        adapter.selectedFiles = MutableList(listFilesViewModel.files.value?.size ?: 0) { false }
    }

    override fun onResume() {
        super.onResume()
        setUpAfterConfigChange()
    }

    private fun setUpAfterConfigChange() {
        collapseExpandFAB(listFilesViewModel.isFABOpen)

        if (listFilesViewModel.syncingStatus.isSyncing) {
            showSyncSnackBar(listFilesViewModel.syncingStatus.maxProgress)
        }

        parentFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )
    }

    override fun onDestroy() {
        super.onDestroy()
        parentFragmentManager.unregisterFragmentLifecycleCallbacks(fragmentFinishedCallback)
    }

    private fun earlyStopSyncSnackBar() {
        snackProgressBarManager.dismiss()
    }

    private fun updateProgressSnackBar(progress: Int) {
        snackProgressBarManager.setProgress(progress)
    }

    private fun showSyncSnackBar(maxProgress: Int) {
        snackProgressBarManager.dismiss()
        syncSnackProgressBar.setProgressMax(maxProgress)
        syncSnackProgressBar.setMessage(
            resources.getString(
                R.string.list_files_sync_snackbar,
                maxProgress.toString()
            )
        )
        snackProgressBarManager.show(
            syncSnackProgressBar,
            SnackProgressBarManager.LENGTH_INDEFINITE
        )
    }

    private fun showPreSyncSnackBar(amountToSync: Int) {
        snackProgressBarManager.dismiss()
        if (amountToSync == 0) {
            Snackbar.make(
                fragment_list_files,
                resources.getString(R.string.list_files_sync_finished_snackbar),
                Snackbar.LENGTH_SHORT
            ).show()
        } else {
            Snackbar.make(
                fragment_list_files,
                resources.getString(
                    R.string.list_files_presync_snackbar,
                    amountToSync.toString()
                ),
                Snackbar.LENGTH_SHORT
            ).show()
        }
    }

    private fun showOfflineSnackBar() {
        snackProgressBarManager.dismiss()
        Snackbar.make(
            fragment_list_files,
            resources.getString(R.string.list_files_offline_snackbar),
            Snackbar.LENGTH_SHORT
        ).show()
    }

    private fun collapseExpandFAB(isFABOpen: Boolean) {
        if (isFABOpen) {
            showFABMenu()
        } else {
            closeFABMenu()
        }
    }

    private fun closeFABMenu() {
        list_files_fab.animate().setDuration(200L).rotation(90f)
        list_files_fab.setImageResource(R.drawable.ic_baseline_add_24)
        list_files_fab_folder.hide()
        list_files_fab_document.hide()
        list_files_refresh.alpha = 1f
        list_files_frame_layout.isClickable = false
    }

    private fun showFABMenu() {
        list_files_fab.animate().setDuration(200L).rotation(-90f)
//        list_files_fab.setImageResource(R.drawable.round_gesture_white_18dp)
        list_files_fab_folder.show()
        list_files_fab_document.show()
        list_files_refresh.alpha = 0.7f
        list_files_frame_layout.isClickable = true
        list_files_frame_layout.setOnClickListener {
            listFilesViewModel.collapseExpandFAB()
        }
    }

    private fun showSuccessfulDeletionSnackbar(view: ViewGroup) {
        Snackbar.make(view, "Successfully deleted the file(s)", Snackbar.LENGTH_SHORT).show()
    }

    private fun updateRecyclerView(
        files: List<FileMetadata>,
        adapter: FilesAdapter
    ) {
        adapter.files = files
        if (!listFilesViewModel.selectedFiles.contains(true)) {
            listFilesViewModel.selectedFiles = MutableList(files.size) { false }
        }

        adapter.selectedFiles = listFilesViewModel.selectedFiles.toMutableList()
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
            errorHasOccurred(fragment_list_files, UNEXPECTED_CLIENT_ERROR)
        }
    }

    private fun navigateToHandwritingEditor(editableFile: EditableFile) {
        val intent = Intent(context, HandwritingEditorActivity::class.java)
        intent.putExtra("name", editableFile.name)
        intent.putExtra("id", editableFile.id)
        startActivityForResult(intent, HANDWRITING_EDITOR_REQUEST_CODE)
    }

    private fun errorHasOccurred(view: ViewGroup, error: String) {
        Snackbar.make(view, error, Snackbar.LENGTH_SHORT).show()
    }

    private fun unexpectedErrorHasOccurred(error: String) {
        AlertDialog.Builder(requireContext(), R.style.DarkBlue_Dialog)
            .setTitle(UNEXPECTED_ERROR)
            .setMessage(error)
            .show()
    }

    private fun showMoreInfoDialog(fileMetadata: FileMetadata) {
        listFilesViewModel.collapseMoreOptionsMenu()

        FileInfoDialogFragment.newInstance(
            fileMetadata.name,
            fileMetadata.id,
            fileMetadata.metadataVersion.toString(),
            fileMetadata.contentVersion.toString(),
            fileMetadata.fileType.name
        ).show(childFragmentManager, FileInfoDialogFragment.FILE_INFO_DIALOG_TAG)
    }

    private fun showMoveFileDialog(moveFileInfo: MoveFileInfo) {
        val dialogFragment = MoveFileDialogFragment.newInstance(moveFileInfo.ids, moveFileInfo.names)

        dialogFragment.show(parentFragmentManager, RenameFileDialogFragment.RENAME_FILE_DIALOG_TAG)
    }

    private fun showRenameFileDialog(renameFileInfo: RenameFileInfo) {
        val dialogFragment = RenameFileDialogFragment.newInstance(renameFileInfo.id, renameFileInfo.name)

        dialogFragment.show(parentFragmentManager, MoveFileDialogFragment.MOVE_FILE_DIALOG_TAG)
    }

    private fun showCreateFileDialog(createFileInfo: CreateFileInfo) {
        val dialogFragment = CreateFileDialogFragment.newInstance(createFileInfo.parentId, createFileInfo.fileType)

        dialogFragment.show(parentFragmentManager, CreateFileDialogFragment.CREATE_FILE_DIALOG_TAG)
    }

    fun onBackPressed(): Boolean {
        return listFilesViewModel.quitOrNot()
    }

    fun onMenuItemPressed(id: Int) {
        listFilesViewModel.onMenuItemPressed(id)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        listFilesViewModel.handleActivityResult(requestCode, resultCode, data)
    }
}
