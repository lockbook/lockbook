package app.lockbook.ui

import android.app.Dialog
import android.os.Bundle
import androidx.fragment.app.DialogFragment
import app.lockbook.R
import app.lockbook.databinding.DialogFileInfoBinding
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import net.lockbook.Lb

class FileInfoDialogFragment : DialogFragment() {
    private lateinit var binding: DialogFileInfoBinding

    companion object {
        const val TAG = "FileInfoDialogFragment"
        private const val FILE_ID_KEY = "file_id"

        fun newInstance(fileId: String): FileInfoDialogFragment =
            FileInfoDialogFragment().apply {
                arguments = Bundle().apply { putString(FILE_ID_KEY, fileId) }
            }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext(), theme)
            .setTitle(R.string.popup_info_title)
            .apply {
                binding = DialogFileInfoBinding.inflate(layoutInflater)
                setUpInfo()
                setView(binding.root)
                setPositiveButton(R.string.done, null)
            }.create()
            .apply {
                setCanceledOnTouchOutside(false)
            }

    private fun setUpInfo() {
        val file = Lb.getFileById(requireArguments().getString(FILE_ID_KEY)!!)

        binding.popupInfoLastModified.text = Lb.getTimestampHumanString(file.lastModified)
        binding.popupInfoName.text = file.name
        binding.popupInfoId.text = file.id
        binding.popupInfoFileType.text = file.type.name
    }
}
