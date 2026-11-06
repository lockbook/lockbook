@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.inputmethod.EditorInfo
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.asLiveData
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.DialogRenameFileBinding
import app.lockbook.model.FileMetadataStore
import app.lockbook.model.FinishedAction
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.util.requestKeyboardFocus
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError

class RenameFileDialogFragment : DialogFragment() {
    private lateinit var binding: DialogRenameFileBinding
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    companion object {
        const val TAG = "RenameFileDialogFragment"
        private const val FILE_ID_KEY = "file_id"
        private const val FILE_NAME_KEY = "file_name"

        fun newInstance(file: File): RenameFileDialogFragment =
            RenameFileDialogFragment().apply {
                arguments =
                    Bundle().apply {
                        putString(FILE_ID_KEY, file.id)
                        putString(FILE_NAME_KEY, file.name)
                    }
            }
    }

    private val fileId by lazy { requireArguments().getString(FILE_ID_KEY)!! }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext())
            .setTitle(R.string.dialog_rename_file_title)
            .apply {
                binding = DialogRenameFileBinding.inflate(layoutInflater)

                var metadataName = requireArguments().getString(FILE_NAME_KEY)!!
                binding.renameFile.setText(metadataName)
                selectNameWithoutExtension()
                FileMetadataStore.snapshots.asLiveData().observe(this@RenameFileDialogFragment) { snapshot ->
                    val name = snapshot?.idsAndFiles?.get(fileId)?.name ?: return@observe
                    if (name != metadataName && binding.renameFile.text.toString() == metadataName) {
                        // Follow remote renames until the user edits the name themselves.
                        binding.renameFile.setText(name)
                        selectNameWithoutExtension()
                    }
                    metadataName = name
                }
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
                    getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
                    window.requestKeyboardFocus(binding.renameFile)
                }
            }

    private fun selectNameWithoutExtension() {
        binding.renameFile.text
            ?.lastIndexOf('.')
            ?.takeIf { it > 0 }
            ?.let { binding.renameFile.setSelection(0, it) }
    }

    private fun onButtonPositive() {
        val name = binding.renameFile.text.toString()
        lifecycleScope.launch(Dispatchers.IO) {
            try {
                Lb.renameFile(fileId, name)
                withContext(Dispatchers.Main) {
                    workspaceModel._finishedAction.postValue(FinishedAction.Rename(fileId, name))
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
