@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.FileMetadataStore
import app.lockbook.model.FinishedAction
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.withFileMetadata
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class DeleteFilesDialogFragment : AppCompatDialogFragment() {
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    val files by lazy {
        requireArguments().getStringArrayList(FILE_IDS_KEY).orEmpty().map(FileMetadataStore::requireFile)
    }

    companion object {
        const val DELETE_FILES_DIALOG_FRAGMENT = "DeleteFilesDialogFragment"
        private const val FILE_IDS_KEY = "file_ids"

        fun newInstance(fileIds: List<String>): DeleteFilesDialogFragment =
            DeleteFilesDialogFragment().apply {
                arguments = Bundle().apply { putStringArrayList(FILE_IDS_KEY, ArrayList(fileIds)) }
            }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext())
            .setMessage(R.string.metadata_loading)
            .setNegativeButton(R.string.cancel, null)
            .setPositiveButton(R.string.delete_file_delete, null)
            .create()
            .apply {
                setOnShowListener {
                    getButton(AlertDialog.BUTTON_POSITIVE).isEnabled = false
                    getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
                }
                withFileMetadata {
                    setMessage(
                        if (files.size ==
                            1
                        ) {
                            getString(R.string.delete_1_file_message, files[0].name)
                        } else {
                            getString(R.string.delete_file_message, files.size)
                        },
                    )
                    getButton(AlertDialog.BUTTON_POSITIVE).isEnabled = true
                }
            }

    private fun onButtonPositive() {
        lifecycleScope.launch(Dispatchers.IO) {
            for (file in files) {
                try {
                    Lb.deleteFile(file.id)
                    workspaceModel.isFileTreeSyncedToCurrentTab = false
                    workspaceModel._finishedAction.postValue(FinishedAction.Delete(file.id))
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
