package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.DialogRenameFileBinding
import app.lockbook.model.CoreModel
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*

data class RenameFileInfo(
    val id: String,
    val name: String
)

class RenameFileDialogFragment : DialogFragment() {

    private lateinit var binding: DialogRenameFileBinding
    private val activityModel: StateViewModel by activityViewModels()

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    companion object {
        const val RENAME_FILE_DIALOG_TAG = "RenameFileDialogFragment"
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext())
        .setTitle(R.string.dialog_rename_file_title)
        .apply {
            binding = DialogRenameFileBinding.inflate(layoutInflater)
            setView(binding.root)
        }
        .setNegativeButton(R.string.cancel, null)
        .setPositiveButton(R.string.rename_file_rename, null)
        .create()
        .apply {
            setOnShowListener {
                getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener{ onButtonPositive() }
            }
        }

    private fun onButtonPositive() {
        val file = (activityModel.transientScreen as TransientScreen.Rename).file

        uiScope.launch(Dispatchers.IO) {
            val createFileResult = CoreModel.renameFile(file.id, binding.renameFile.text.toString())

            withContext(Dispatchers.Main) {
                when(createFileResult) {
                    is Ok -> dismiss()
                    is Err -> binding.renameFileError.setText(createFileResult.error.toLbError(resources).msg)
                }.exhaustive

            }
        }
    }
}
