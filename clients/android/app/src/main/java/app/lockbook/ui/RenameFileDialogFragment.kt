@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.inputmethod.EditorInfo
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.DialogRenameFileBinding
import app.lockbook.model.FileMetadataStore
import app.lockbook.model.FinishedAction
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.requestKeyboardFocus
import app.lockbook.util.withFileMetadata
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.Lb
import net.lockbook.LbError

class RenameFileDialogFragment : DialogFragment() {
    private var nameLoaded = false
    private lateinit var binding: DialogRenameFileBinding
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    companion object {
        const val TAG = "RenameFileDialogFragment"
        private const val FILE_ID_KEY = "file_id"

        fun newInstance(fileId: String): RenameFileDialogFragment =
            RenameFileDialogFragment().apply {
                arguments = Bundle().apply { putString(FILE_ID_KEY, fileId) }
            }
    }

    val file by lazy {
        FileMetadataStore.requireFile(requireArguments().getString(FILE_ID_KEY)!!)
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext())
            .setTitle(R.string.dialog_rename_file_title)
            .apply {
                binding = DialogRenameFileBinding.inflate(layoutInflater)

                binding.renameFile.isEnabled = false
                binding.renameFile.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_DONE) {
                        onButtonPositive()
                    }

                    true
                }

                setView(binding.root)
            }.setNegativeButton(R.string.cancel, null)
            .setPositiveButton(R.string.rename_file_rename, null)
            .create()
            .apply {
                setOnShowListener {
                    getButton(AlertDialog.BUTTON_POSITIVE).isEnabled = nameLoaded
                    getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
                }
                withFileMetadata {
                    if (savedInstanceState?.getBoolean("nameLoaded") != true) {
                        binding.renameFile.setText(file.name)
                        binding.renameFile.text
                            ?.lastIndexOf('.')
                            ?.takeIf { it > 0 }
                            ?.let { binding.renameFile.setSelection(0, it) }
                    }
                    nameLoaded = true
                    binding.renameFile.isEnabled = true
                    getButton(AlertDialog.BUTTON_POSITIVE).isEnabled = true
                    window.requestKeyboardFocus(binding.renameFile)
                }
            }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putBoolean("nameLoaded", nameLoaded)
    }

    private fun onButtonPositive() {
        lifecycleScope.launch(Dispatchers.IO) {
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
