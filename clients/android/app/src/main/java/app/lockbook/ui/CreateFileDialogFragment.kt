package app.lockbook.ui

import android.annotation.SuppressLint
import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.DialogCreateFileBinding
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.util.requestKeyboardFocus
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError

class CreateFileDialogFragment : AppCompatDialogFragment() {
    private lateinit var binding: DialogCreateFileBinding

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())
    private val activityModel: StateViewModel by activityViewModels()
    private val info by lazy {
        activityModel.transientScreen as TransientScreen.Create
    }

    var newFile: File? = null

    companion object {
        const val TAG = "CreateFileDialogFragment"
    }

    @SuppressLint("SetTextI18n")
    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext())
        .apply {
            binding = DialogCreateFileBinding.inflate(layoutInflater)

            setUpFolderTextInput()

            binding.createDocumentHolder.visibility = View.GONE
            binding.createDocumentExtensionHolder.visibility = View.GONE
            binding.createFolderHolder.visibility = View.VISIBLE
            
            val title = getString(R.string.create_file_title_folder)

            setTitle(title)
            setView(binding.root)
        }
        .setPositiveButton(R.string.create, null)
        .setNegativeButton(R.string.cancel, null)
        .create()
        .apply {
            window.requestKeyboardFocus(binding.createFolder)
            setOnShowListener {
                getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
            }
        }

    private fun setUpFolderTextInput() {
        binding.createFolder.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                onButtonPositive()
            }

            true
        }
    }

    private fun setUpDocumentTextInput() {
        binding.createDocument.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_NEXT) {
                binding.createDocumentExtension.apply {
                    requestFocus()
                    selectAll()
                }
            }

            true
        }

        binding.createDocumentExtension.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                onButtonPositive()
            }

            true
        }
    }

    private fun onButtonPositive() {
        val fileName = binding.createFolder.text.toString()

        uiScope.launch(Dispatchers.IO) {
            try {
                newFile = Lb.createFile(fileName, info.parentId, false)
                withContext(Dispatchers.Main) {
                    dismiss()
                }
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
                    binding.createFileError.setText(err.msg)
                }
            }
        }
    }
}
