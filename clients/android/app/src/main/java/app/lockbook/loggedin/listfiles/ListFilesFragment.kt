package app.lockbook.loggedin.listfiles

import android.app.Dialog
import android.content.Intent
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.EditText
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentListFilesBinding
import app.lockbook.loggedin.editor.HandwritingEditorActivity
import app.lockbook.loggedin.editor.TextEditorActivity
import app.lockbook.loggedin.popupinfo.PopUpInfoActivity
import app.lockbook.utils.EditableFile
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.RequestResultCodes.POP_UP_INFO_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import kotlinx.android.synthetic.main.fragment_list_files.*

class ListFilesFragment : Fragment() {
    private lateinit var listFilesViewModel: ListFilesViewModel
    private val snackProgressBarManager by lazy {
        SnackProgressBarManager(
            requireView(),
            lifecycleOwner = this
        ).setViewToMove(list_files_layout)
    }
    private val offlineSnackBar by lazy {
        SnackProgressBar(
            SnackProgressBar.TYPE_NORMAL,
            resources.getString(R.string.list_files_offline_snackbar)
        )
            .setSwipeToDismiss(false)
            .setAllowUserInput(true)
    }
    private val preSyncSnackBar by lazy {
        SnackProgressBar(
            SnackProgressBar.TYPE_NORMAL,
            resources.getString(R.string.list_files_presync_snackbar, "n")
        )
            .setSwipeToDismiss(true)
            .setAllowUserInput(true)
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
    private val syncUpToDateSnackBar by lazy {
        SnackProgressBar(
            SnackProgressBar.TYPE_NORMAL,
            resources.getString(R.string.list_files_sync_finished_snackbar)
        )
            .setSwipeToDismiss(false)
            .setAllowUserInput(true)
    }
    private lateinit var alertDialog: AlertDialog

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

        listFilesViewModel.navigateToPopUpInfo.observe(
            viewLifecycleOwner,
            { fileMetadata ->
                navigateToPopUpInfo(fileMetadata)
            }
        )

        listFilesViewModel.collapseExpandFAB.observe(
            viewLifecycleOwner,
            { isFABOpen ->
                collapseExpandFAB(isFABOpen)
            }
        )

        listFilesViewModel.createFileNameDialog.observe(
            viewLifecycleOwner,
            {
                createFileNameDialog("")
            }
        )

        listFilesViewModel.fileModelErrorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                errorHasOccurred(errorText)
            }
        )

        listFilesViewModel.errorHasOccurred.observe(
            viewLifecycleOwner,
            { errorText ->
                errorHasOccurred(errorText)
            }
        )

        return binding.root
    }

    override fun onResume() {
        super.onResume()
        setUpAfterConfigChange()
    }

    private fun setUpAfterConfigChange() {
        collapseExpandFAB(listFilesViewModel.isFABOpen)
        if (listFilesViewModel.dialogStatus.isDialogOpen) {
            createFileNameDialog(listFilesViewModel.dialogStatus.alertDialogFileName)
        }
        if (listFilesViewModel.syncingStatus.isSyncing) {
            showSyncSnackBar(listFilesViewModel.syncingStatus.maxProgress)
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        setUpBeforeConfigChange()
    }

    private fun setUpBeforeConfigChange() {
        if (listFilesViewModel.dialogStatus.isDialogOpen) {
            listFilesViewModel.dialogStatus.alertDialogFileName = alertDialog.findViewById<EditText>(R.id.new_file_username)?.text.toString()
            alertDialog.dismiss()
        }
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
            snackProgressBarManager.show(syncUpToDateSnackBar, SnackProgressBarManager.LENGTH_LONG)
        } else {
            snackProgressBarManager.show(
                preSyncSnackBar.setMessage(
                    resources.getString(
                        R.string.list_files_presync_snackbar,
                        amountToSync.toString()
                    )
                ),
                SnackProgressBarManager.LENGTH_SHORT
            )
        }
    }

    private fun showOfflineSnackBar() {
        snackProgressBarManager.dismiss()
        snackProgressBarManager.show(offlineSnackBar, SnackProgressBarManager.LENGTH_SHORT)
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
        list_files_layout.isClickable = false
    }

    private fun showFABMenu() {
        list_files_fab.animate().setDuration(200L).rotation(-90f)
//        list_files_fab.setImageResource(R.drawable.round_gesture_white_18dp)
        list_files_fab_folder.show()
        list_files_fab_document.show()
        list_files_refresh.alpha = 0.7f
        list_files_layout.isClickable = true
        list_files_layout.setOnClickListener {
            listFilesViewModel.collapseExpandFAB()
        }
    }

    private fun createFileNameDialog(originalFileName: String) {
        val dialogBuilder = AlertDialog.Builder(requireContext(), R.style.DarkBlue_Dialog)

        alertDialog = dialogBuilder.setView(
            layoutInflater.inflate(
                R.layout.dialog_create_file_name,
                view as ViewGroup,
                false
            )
        )
            .setPositiveButton(R.string.new_file_create) { dialog, _ ->
                listFilesViewModel.handleNewFileRequest((dialog as Dialog).findViewById<EditText>(R.id.new_file_username).text.toString())
                listFilesViewModel.dialogStatus.isDialogOpen = false
                dialog.dismiss()
            }
            .setNegativeButton(R.string.new_file_cancel) { dialog, _ ->
                dialog.cancel()
                listFilesViewModel.dialogStatus.isDialogOpen = false
            }
            .create()

        alertDialog.show()
        alertDialog.findViewById<EditText>(R.id.new_file_username)?.setText(originalFileName)
    }

    private fun updateRecyclerView(
        files: List<FileMetadata>,
        adapter: FilesAdapter
    ) {
        adapter.files = files
    }

    private fun navigateToFileEditor(editableFile: EditableFile) {
        val intent = Intent(context, TextEditorActivity::class.java)
        intent.putExtra("name", editableFile.name)
        intent.putExtra("id", editableFile.id)
        startActivityForResult(intent, TEXT_EDITOR_REQUEST_CODE)
    }

    private fun navigateToPopUpInfo(fileMetadata: FileMetadata) {
        val intent = Intent(context, PopUpInfoActivity::class.java)
        intent.putExtra("name", fileMetadata.name)
        intent.putExtra("id", fileMetadata.id)
        intent.putExtra("fileType", fileMetadata.file_type.toString())
        intent.putExtra("metadataVersion", fileMetadata.metadata_version.toString())
        intent.putExtra("contentVersion", fileMetadata.content_version.toString())
        startActivityForResult(intent, POP_UP_INFO_REQUEST_CODE)
    }

    private fun navigateToHandwritingEditor(editableFile: EditableFile) {
        val intent = Intent(context, HandwritingEditorActivity::class.java)
        intent.putExtra("name", editableFile.name)
        intent.putExtra("id", editableFile.id)
        startActivity(intent)
    }

    private fun errorHasOccurred(errorText: String) {
        Toast.makeText(context, errorText, Toast.LENGTH_LONG).show()
    }

    fun onBackPressed(): Boolean {
        return listFilesViewModel.quitOrNot()
    }

    fun onSortPressed(id: Int) {
        listFilesViewModel.onSortPressed(id)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        listFilesViewModel.handleActivityResult(requestCode, resultCode, data)
    }
}
