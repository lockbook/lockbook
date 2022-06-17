package app.lockbook.ui

import android.app.Dialog
import android.os.Bundle
import android.view.LayoutInflater
import androidx.appcompat.app.AppCompatDialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.DialogCreateFileBinding
import app.lockbook.model.StateViewModel
import com.google.android.material.dialog.MaterialAlertDialogBuilder

class CreateFileDialogFragment : AppCompatDialogFragment() {
    private lateinit var binding: DialogCreateFileBinding
    private val activityModel: StateViewModel by activityViewModels()

    companion object {
        const val CREATE_FILE_DIALOG_TAG = "CreateFileDialogFragment"
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext(), theme)
//        .setView(view)

        .setTitle(R.string.new_file_title)
        .apply {
            binding = DialogCreateFileBinding.inflate(LayoutInflater.from(requireContext()))
            setView(binding.root)
        }
        .setPositiveButton(R.string.create_file_create) { _, _ -> }
        .setNegativeButton(R.string.cancel) { _, _ -> }.show()
}
