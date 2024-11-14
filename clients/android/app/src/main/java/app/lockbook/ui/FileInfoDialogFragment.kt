package app.lockbook.ui

import android.app.Dialog
import android.os.Bundle
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.DialogFileInfoBinding
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import net.lockbook.Lb

class FileInfoDialogFragment : DialogFragment() {
    private lateinit var binding: DialogFileInfoBinding
    private val activityModel: StateViewModel by activityViewModels()

    companion object {
        const val TAG = "FileInfoDialogFragment"
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext(), theme)
        .setTitle(R.string.popup_info_title)
        .apply {
            binding = DialogFileInfoBinding.inflate(layoutInflater)
            setUpInfo()
            setView(binding.root)
        }
        .create()

    private fun setUpInfo() {
        val file = (activityModel.transientScreen as TransientScreen.Info).file

        binding.popupInfoLastModified.text = Lb.getTimestampHumanString(file.lastModified)
        binding.popupInfoName.text = file.name
        binding.popupInfoId.text = file.id
        binding.popupInfoFileType.text = file.type.name
    }
}
