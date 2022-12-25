package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import androidx.appcompat.app.AppCompatDialogFragment
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.util.File
import com.github.michaelbull.result.Err
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class DeleteSharedDialogFragment private constructor() : AppCompatDialogFragment() {
    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    companion object {
        const val DELETE_SHARED_DIALOG_FRAGMENT = "DeleteSharedDialogFragment"

        const val FILES_KEY = "files_key"

        fun newInstance(files: ArrayList<File>): DeleteSharedDialogFragment {
            val dialog = DeleteSharedDialogFragment()

            val bundle = Bundle()
            bundle.putParcelableArrayList(FILES_KEY, files)
            dialog.arguments = bundle

            return dialog
        }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog {
        val files = requireArguments().getParcelableArrayList<File>(FILES_KEY)!!

        return MaterialAlertDialogBuilder(requireContext())
            .apply {
                val msg = if (files.size == 1) {
                    getString(R.string.delete_shared_1_message, files[0].name)
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
                    getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive(files) }
                }
            }
    }

    private fun onButtonPositive(files: ArrayList<File>) {
        uiScope.launch(Dispatchers.IO) {
            for (file in files) {
                val deleteFileResult = CoreModel.deletePendingShare(file.id)

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
