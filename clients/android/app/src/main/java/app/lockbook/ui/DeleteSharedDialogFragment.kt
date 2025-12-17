package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.core.os.bundleOf
import androidx.fragment.app.setFragmentResult
import app.lockbook.R
import app.lockbook.model.AlertModel
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class DeleteSharedDialogFragment private constructor() : AppCompatDialogFragment() {
    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    companion object {
        const val DELETE_SHARED_DIALOG_FRAGMENT = "DeleteSharedDialogFragment"

        const val FILES_ID_KEY = "files_key"

        const val DELETE_SHARE_REQUEST_KEY = "delete_share_request_key"
        const val DELETE_SHARE_BUNDLE_KEY = "delete_share_bundle_key"

        fun newInstance(files: ArrayList<File>): DeleteSharedDialogFragment {
            val dialog = DeleteSharedDialogFragment()

            val bundle = Bundle()
            bundle.putStringArray(FILES_ID_KEY, files.map { it.id }.toTypedArray())
            dialog.arguments = bundle

            return dialog
        }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog {
        val files = requireArguments().getStringArray(FILES_ID_KEY)!!.map { Lb.getFileById(it) }

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

    private fun onButtonPositive(files: List<File>) {
        uiScope.launch(Dispatchers.IO) {
            for (file in files) {
                try {
                    Lb.deletePendingShare(file.id)
                    setFragmentResult(DELETE_SHARE_REQUEST_KEY, bundleOf(DELETE_SHARE_BUNDLE_KEY to file.id))
                } catch (err: LbError) {
                    alertModel.notifyError(err)
                }
            }

            withContext(Dispatchers.Main) {
                dismiss()
            }
        }
    }
}
