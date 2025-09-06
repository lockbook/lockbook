package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.inputmethod.EditorInfo
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.DialogRenameFileBinding
import app.lockbook.model.FinishedAction
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.requestKeyboardFocus
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.Lb
import net.lockbook.LbError

class RenameFileDialogFragment : DialogFragment() {
    private lateinit var binding: DialogRenameFileBinding
    private val activityModel: StateViewModel by activityViewModels()
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    companion object {
        const val TAG = "RenameFileDialogFragment"
    }

    val file by lazy {
        (activityModel.transientScreen as TransientScreen.Rename).file
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext())
            .setTitle(R.string.dialog_rename_file_title)
            .apply {
                binding = DialogRenameFileBinding.inflate(layoutInflater)

                binding.renameFile.setText(file.name)
                binding.renameFile.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_DONE) {
                        onButtonPositive()
                    }

                    true
                }

                setView(binding.root)
            }
            .setNegativeButton(R.string.cancel, null)
            .setPositiveButton(R.string.rename_file_rename, null)
            .create()
            .apply {
                window.requestKeyboardFocus(binding.renameFile)

                binding.renameFile.text?.lastIndexOf(".")?.takeIf { it > 0 }
                    ?.let { binding.renameFile.setSelection(0, it) }

                setOnShowListener {
                    getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
                }
            }

    private fun onButtonPositive() {
        uiScope.launch(Dispatchers.IO) {
            try {
                Lb.renameFile(file.id, binding.renameFile.text.toString())
                withContext(Dispatchers.Main) {
                    workspaceModel._finishedAction.postValue(FinishedAction.Rename(file.id, binding.renameFile.text.toString()))
                    dismiss()
                }
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
                    binding.renameFileError.setText(err.msg)
                }
            }
        }
    }
}
