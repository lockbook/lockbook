package app.lockbook.ui

import android.annotation.SuppressLint
import android.app.Dialog
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.DialogCreateFileBinding
import app.lockbook.model.ExtendedFileType
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.util.exhaustive
import com.google.android.material.dialog.MaterialAlertDialogBuilder

class CreateFileDialogFragment : AppCompatDialogFragment() {
    private lateinit var binding: DialogCreateFileBinding

    private val activityModel: StateViewModel by activityViewModels()
    private val info by lazy {
        (activityModel.transientScreen as TransientScreen.Create).info
    }

    companion object {
        const val CREATE_FILE_DIALOG_TAG = "CreateFileDialogFragment"
        const val FILE_NAME_KEY = "file_name_key"
        const val FILE_EXTENSION_KEY = "file_extension_key"
    }

    @SuppressLint("SetTextI18n")
    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext(), theme)
        .apply {
            binding = DialogCreateFileBinding.inflate(LayoutInflater.from(requireContext()))
            val title = when(info.extendedFileType) {
                ExtendedFileType.Drawing -> {
                    binding.createDocument.setText(savedInstanceState?.getString(FILE_NAME_KEY) ?: "")
                    binding.createDocumentExtension.setText(savedInstanceState?.getString(FILE_EXTENSION_KEY) ?: ".draw")

                    getString(R.string.create_file_title_drawing)
                }
                ExtendedFileType.Folder -> {
                    binding.createDocumentHolder.visibility = View.GONE
                    binding.createDocumentExtensionHolder.visibility = View.GONE
                    binding.createFolderHolder.visibility = View.VISIBLE

                    binding.createFolder.setText(savedInstanceState?.getString(FILE_NAME_KEY) ?: "")

                    getString(R.string.create_file_title_folder)
                }
                ExtendedFileType.Text -> {
                    binding.createDocument.setText(savedInstanceState?.getString(FILE_NAME_KEY) ?: "")
                    binding.createDocumentExtension.setText(savedInstanceState?.getString(FILE_EXTENSION_KEY) ?: ".md")

                    getString(R.string.create_file_title_document)
                }
            }.exhaustive

            setTitle(title)
            setView(binding.root)
        }
        .setPositiveButton(R.string.create_file_create) { _, _ ->

        }
        .setNegativeButton(R.string.cancel) { _, _ ->

        }.show()

    override fun onSaveInstanceState(outState: Bundle) {
        outState
        super.onSaveInstanceState(outState)
    }
}
