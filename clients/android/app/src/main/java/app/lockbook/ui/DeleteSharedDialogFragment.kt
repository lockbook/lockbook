@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.FileMetadataStore
import app.lockbook.util.withFileMetadata
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class DeleteSharedDialogFragment private constructor() : AppCompatDialogFragment() {
    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    companion object {
        const val DELETE_SHARED_DIALOG_FRAGMENT = "DeleteSharedDialogFragment"

        const val FILES_ID_KEY = "files_key"

        fun newInstance(files: ArrayList<File>): DeleteSharedDialogFragment {
            val dialog = DeleteSharedDialogFragment()

            val bundle = Bundle()
            bundle.putStringArray(FILES_ID_KEY, files.map { it.id }.toTypedArray())
            dialog.arguments = bundle

            return dialog
        }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog {
        val files by lazy { requireArguments().getStringArray(FILES_ID_KEY)!!.map { FileMetadataStore.requireFile(it) } }

        return MaterialAlertDialogBuilder(requireContext())
            .setMessage(R.string.metadata_loading)
            .setNegativeButton(R.string.cancel, null)
            .setPositiveButton(R.string.delete_shared_reject, null)
            .create()
            .apply {
                setOnShowListener {
                    getButton(AlertDialog.BUTTON_POSITIVE).isEnabled = false
                    getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive(files) }
                }
                withFileMetadata {
                    setMessage(
                        if (files.size ==
                            1
                        ) {
                            getString(R.string.delete_shared_1_message, files[0].name)
                        } else {
                            getString(R.string.delete_shared_message, files.size)
                        },
                    )
                    getButton(AlertDialog.BUTTON_POSITIVE).isEnabled = true
                }
            }
    }

    private fun onButtonPositive(files: List<File>) {
        lifecycleScope.launch {
            for (file in files) {
                try {
                    withContext(Dispatchers.IO) {
                        Lb.deletePendingShare(file.id)
                    }
                } catch (err: LbError) {
                    alertModel.notifyError(err)
                }
            }

            dismiss()
        }
    }
}
