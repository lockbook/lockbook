package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import app.lockbook.R
import app.lockbook.databinding.DialogMoveFileBinding
import app.lockbook.model.*
import app.lockbook.util.DecryptedFileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.HorizontalViewHolder
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import java.lang.ref.WeakReference

class MoveFileDialogFragment : DialogFragment() {

    private var _binding: DialogMoveFileBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    private val activityModel: StateViewModel by activityViewModels()
    private val model: MoveFileViewModel by viewModels()

    companion object {
        const val MOVE_FILE_DIALOG_TAG = "MoveFileDialogFragment"
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DialogMoveFileBinding.inflate(
            inflater,
            container,
            false
        )

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        binding.moveFileList.setup {
            withDataSource(model.files)
            withItem<DecryptedFileMetadata, HorizontalViewHolder>(R.layout.linear_layout_file_item) {
                onBind(::HorizontalViewHolder) { _, item ->
                    name.text = item.decryptedName
                    description.text = resources.getString(
                        R.string.last_synced,
                        CoreModel.convertToHumanDuration(item.metadataVersion)
                    )
                    val extensionHelper = ExtensionHelper(item.decryptedName)

                    val imageResource = when {
                        item.fileType == FileType.Document && extensionHelper.isDrawing -> {
                            R.drawable.ic_baseline_draw_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isImage -> {
                            R.drawable.ic_baseline_image_24
                        }
                        item.fileType == FileType.Document -> {
                            R.drawable.ic_baseline_insert_drive_file_24
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

        binding.moveFileCancel.setOnClickListener {
            dismiss()
        }
        binding.moveFileConfirm.setOnClickListener {
            binding.moveFileProgressBar.visibility = View.VISIBLE
            model.moveFilesToFolder((activityModel.transientScreen as TransientScreen.Move).ids)
        }

        dialog?.setCanceledOnTouchOutside(false) ?: alertModel.notifyBasicError()

        model.ids = (activityModel.transientScreen as TransientScreen.Move).ids

        model.closeDialog.observe(
            viewLifecycleOwner
        ) {
            binding.moveFileProgressBar.visibility = View.GONE
            dismiss()
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
        }
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.MATCH_PARENT
        )
    }
}
