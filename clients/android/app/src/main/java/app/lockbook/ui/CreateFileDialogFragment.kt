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
import app.lockbook.model.CoreModel
import app.lockbook.model.ExtendedFileType
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.util.File
import app.lockbook.util.FileType
import app.lockbook.util.exhaustive
import app.lockbook.util.requestKeyboardFocus
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.*

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

            val title = when (info.extendedFileType) {
                ExtendedFileType.Drawing -> {
                    setUpDocumentTextInput()

                    binding.createDocument.setText("")
                    binding.createDocumentExtension.setText(".svg")

                    binding.createDocumentHolder.setStartIconDrawable(R.drawable.ic_outline_draw_24)

                    getString(R.string.create_file_title_drawing)
                }
                ExtendedFileType.Folder -> {
                    setUpFolderTextInput()

                    binding.createDocumentHolder.visibility = View.GONE
                    binding.createDocumentExtensionHolder.visibility = View.GONE
                    binding.createFolderHolder.visibility = View.VISIBLE

                    binding.createFolder.setText("")

                    getString(R.string.create_file_title_folder)
                }
                ExtendedFileType.Document -> {
                    setUpDocumentTextInput()

                    binding.createDocument.setText("")
                    binding.createDocumentExtension.setText(".md")

                    getString(R.string.create_file_title_document)
                }
            }.exhaustive

            setTitle(title)
            setView(binding.root)
        }
        .setPositiveButton(R.string.create, null)
        .setNegativeButton(R.string.cancel, null)
        .create()
        .apply {
            when (info.extendedFileType.toFileType()) {
                FileType.Document -> window.requestKeyboardFocus(binding.createDocument)
                FileType.Folder -> window.requestKeyboardFocus(binding.createFolder)
            }.exhaustive
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
        val fileType = info.extendedFileType.toFileType()
        val fileName = when (fileType) {
            FileType.Document -> "${binding.createDocument.text}${binding.createDocumentExtension.text}"
            FileType.Folder -> binding.createFolder.text.toString()
        }

        uiScope.launch(Dispatchers.IO) {
            val createFileResult = CoreModel.createFile(info.parentId, fileName, fileType)
            withContext(Dispatchers.Main) {
                when (createFileResult) {
                    is Ok -> {
                        newFile = createFileResult.value
                        dismiss()
                    }
                    is Err -> binding.createFileError.setText(createFileResult.error.toLbError(resources).msg)
                }.exhaustive
            }
        }
    }
}
