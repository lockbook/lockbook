package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.FinishedAction
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.model.WorkspaceViewModel
import com.github.michaelbull.result.Err
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class DeleteFilesDialogFragment : AppCompatDialogFragment() {
    private val activityModel: StateViewModel by activityViewModels()
    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    val files by lazy {
        (activityModel.transientScreen as TransientScreen.Delete).files
    }

    companion object {
        const val DELETE_FILES_DIALOG_FRAGMENT = "DeleteFilesDialogFragment"
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext())
        .apply {
            val msg = if (files.size == 1) {
                getString(R.string.delete_1_file_message, files[0].name)
            } else {
                getString(R.string.delete_file_message, files.size)
            }

            setMessage(msg)
        }
        .setNegativeButton(R.string.cancel, null)
        .setPositiveButton(R.string.delete_file_delete, null)
        .create()
        .apply {
            setOnShowListener {
                getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
            }
        }

    private fun onButtonPositive() {
        uiScope.launch(Dispatchers.IO) {

            for (file in files) {
                val deleteFileResult = CoreModel.deleteFile(file.id)

                if (deleteFileResult is Err) {
                    alertModel.notifyError(deleteFileResult.error.toLbError(resources))
                    break
                } else {
                    workspaceModel._finishedAction.postValue(FinishedAction.Delete(file.id))
                }
            }

            withContext(Dispatchers.Main) {
                dismiss()
            }
        }
    }
}
