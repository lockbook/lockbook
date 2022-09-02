package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.util.File
import com.github.michaelbull.result.Err
import com.google.android.material.bottomsheet.BottomSheetDialogFragment
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class DeleteSharedDialogFragment : AppCompatDialogFragment() {
    private val activityModel: StateViewModel by activityViewModels()
    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    val files by lazy {
        (activityModel.transientScreen as TransientScreen.DeleteShared).files
    }

    companion object {
        const val DELETE_SHARED_DIALOG_FRAGMENT = "DeleteSharedDialogFragment"
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext())
        .apply {
            val msg = if (files.size == 1) {
                getString(R.string.delete_shared_1_message, files[0].name, files[0].owner)
            } else {
                getString(R.string.delete_shared_message, files.size)
            }

            setMessage(msg)
        }
        .setNegativeButton(R.string.cancel, null)
        .setPositiveButton(R.string.delete_shared_reject, null)
        .create()
        .apply {
            setOnShowListener {
                getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
            }
        }

    private fun onButtonPositive() {
        uiScope.launch(Dispatchers.IO) {

            for (file in files) {
                val deleteFileResult = CoreModel.deletePendingShares(file.id)

                if (deleteFileResult is Err) {
                    alertModel.notifyError(deleteFileResult.error.toLbError(resources))
                    break
                }
            }

            withContext(Dispatchers.Main) {
                dismiss()
            }
        }
    }
}