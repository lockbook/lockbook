package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.DialogMoveFileBinding
import app.lockbook.model.*
import app.lockbook.util.DecryptedFileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.MoveFileItemViewHolder
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import java.lang.ref.WeakReference

class MoveFileDialogFragment : DialogFragment() {

    private lateinit var binding: DialogMoveFileBinding

    private val activityModel: StateViewModel by activityViewModels()
    private val model: MoveFileViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(MoveFileViewModel::class.java))
                        return MoveFileViewModel(
                            requireActivity().application,
                            (activityModel.transientScreen as TransientScreen.Move).files[0].parent
                        ) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    companion object {
        const val MOVE_FILE_DIALOG_TAG = "MoveFileDialogFragment"
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext())
        .setTitle(R.string.move_file_title)
        .apply {
            binding = DialogMoveFileBinding.inflate(layoutInflater)
            setUpView()
            setView(binding.root)
        }
        .setNegativeButton(R.string.cancel, null)
        .setPositiveButton(R.string.move_file_move, null)
        .create()
        .apply {
            setOnShowListener {
                getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
            }
        }

    private fun setUpView() {
        binding.moveFileList.setup {
            withDataSource(model.files)
            withItem<DecryptedFileMetadata, MoveFileItemViewHolder>(R.layout.move_file_item) {
                onBind(::MoveFileItemViewHolder) { _, item ->
                    name.text = item.decryptedName
                    val extensionHelper = ExtensionHelper(item.decryptedName)

                    val imageResource = when {
                        item.fileType == FileType.Document && extensionHelper.isDrawing -> {
                            R.drawable.ic_outline_draw_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isImage -> {
                            R.drawable.ic_outline_image_24
                        }
                        item.fileType == FileType.Document -> {
                            R.drawable.ic_outline_insert_drive_file_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isPdf -> {
                            R.drawable.ic_outline_picture_as_pdf_24
                        }
                        else -> {
                            R.drawable.ic_baseline_folder_24
                        }
                    }

                    icon.setImageResource(imageResource)
                }
                onClick {
                    model.onItemClick(item)
                }
            }
        }

        model.ids = (activityModel.transientScreen as TransientScreen.Move).files.map { it.id }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        model.closeDialog.observe(
            viewLifecycleOwner
        ) {
            dismiss()
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
            dismiss()
        }
    }

    private fun onButtonPositive() {
        model.moveFilesToCurrentFolder()
    }
}
