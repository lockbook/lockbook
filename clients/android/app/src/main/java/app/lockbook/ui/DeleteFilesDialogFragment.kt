@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.asLiveData
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.FileMetadataStore
import app.lockbook.model.FinishedAction
import app.lockbook.model.WorkspaceViewModel
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class DeleteFilesDialogFragment : AppCompatDialogFragment() {
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private val fileIds by lazy { requireArguments().getStringArrayList(FILE_IDS_KEY).orEmpty() }

    companion object {
        const val DELETE_FILES_DIALOG_FRAGMENT = "DeleteFilesDialogFragment"
        private const val FILE_IDS_KEY = "file_ids"
        private const val FILE_NAME_KEY = "file_name"

        fun newInstance(files: List<File>): DeleteFilesDialogFragment =
            DeleteFilesDialogFragment().apply {
                arguments =
                    Bundle().apply {
                        putStringArrayList(FILE_IDS_KEY, ArrayList(files.map { it.id }))
                        putString(FILE_NAME_KEY, files.singleOrNull()?.name)
                    }
            }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext())
            .setMessage(
                if (fileIds.size == 1) {
                    getString(R.string.delete_1_file_message, requireArguments().getString(FILE_NAME_KEY))
                } else {
                    getString(R.string.delete_file_message, fileIds.size)
                },
            ).setNegativeButton(R.string.cancel, null)
            .setPositiveButton(R.string.delete_file_delete, null)
            .create()
            .apply {
                setOnShowListener {
                    getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
                }
                FileMetadataStore.snapshots.asLiveData().observe(this@DeleteFilesDialogFragment) { snapshot ->
                    fileIds.singleOrNull()?.let { id ->
                        snapshot?.idsAndFiles?.get(id)?.let { file ->
                            setMessage(getString(R.string.delete_1_file_message, file.name))
                        }
                    }
                }
            }

    private fun onButtonPositive() {
        lifecycleScope.launch(Dispatchers.IO) {
            for (fileId in fileIds) {
                try {
                    Lb.deleteFile(fileId)
                    workspaceModel.isFileTreeSyncedToCurrentTab = false
                    workspaceModel._finishedAction.postValue(FinishedAction.Delete(fileId))
                } catch (err: LbError) {
                    alertModel.notifyError(err)
                    break
                }
            }

            withContext(Dispatchers.Main) {
                dismiss()
            }
        }
    }
}
