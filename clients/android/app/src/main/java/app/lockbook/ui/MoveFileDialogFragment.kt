@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.DialogMoveFileBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.MoveFileViewModel
import app.lockbook.util.FileMetadataRowInfo
import app.lockbook.util.FileMetadataViewHolder
import app.lockbook.util.getIconResource
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.listitem.ListItemLayout
import net.lockbook.File
import net.lockbook.Lb
import java.lang.ref.WeakReference

class MoveFileDialogFragment : DialogFragment() {
    private lateinit var binding: DialogMoveFileBinding

    private val files: List<File> by lazy {
        requireArguments().getStringArrayList(FILE_IDS_KEY).orEmpty().map(Lb::getFileById)
    }
    private val model: MoveFileViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(MoveFileViewModel::class.java)) {
                        return MoveFileViewModel(
                            requireActivity().application,
                            files.first().parent,
                        ) as T
                    }
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        },
    )

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    companion object {
        const val TAG = "MoveFileDialogFragment"
        private const val FILE_IDS_KEY = "file_ids"

        fun newInstance(fileIds: List<String>): MoveFileDialogFragment =
            MoveFileDialogFragment().apply {
                arguments = Bundle().apply { putStringArrayList(FILE_IDS_KEY, ArrayList(fileIds)) }
            }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext())
            .setTitle(R.string.move_file_title)
            .apply {
                binding = DialogMoveFileBinding.inflate(layoutInflater)
                setUpView()
                setView(binding.root)
            }.setNegativeButton(R.string.cancel, null)
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
            withItem<File, FileMetadataViewHolder>(R.layout.file_metadata_item) {
                onBind(::FileMetadataViewHolder) { index, item ->
                    (itemView as? ListItemLayout)?.updateAppearance(index, model.files.toList().size)
                    bind(
                        FileMetadataRowInfo(
                            file = item,
                            title = item.name,
                            iconRes = item.getIconResource(),
                        ),
                    )
                    fileItemHolder.setOnClickListener {
                        model.onItemClick(item)
                    }
                }
            }
        }

        model.ids = files.map { it.id }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View = binding.root

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        model.closeDialog.observe(
            viewLifecycleOwner,
        ) {
            dismiss()
        }

        model.notifyError.observe(
            viewLifecycleOwner,
        ) { error ->
            alertModel.notifyError(error)
            dismiss()
        }
    }

    private fun onButtonPositive() {
        model.moveFilesToCurrentFolder()
    }
}
