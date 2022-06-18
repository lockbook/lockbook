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
import java.sql.Timestamp
import java.util.*

class FileInfoDialogFragment : DialogFragment() {
    private lateinit var binding: DialogFileInfoBinding
    private val activityModel: StateViewModel by activityViewModels()

    companion object {
        const val FILE_INFO_DIALOG_TAG = "FileInfoDialogFragment"
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

        val dateMetadataVersion = Date(Timestamp(file.metadataVersion).time)
        val dateContentVersion = Date(Timestamp(file.contentVersion).time)

        binding.popupInfoMetadataVersion.text = if (dateMetadataVersion.time != 0L) dateMetadataVersion.time.toString() else resources.getString(R.string.pop_up_info_never_synced)
        binding.popupInfoContentVersion.text = if (dateContentVersion.time != 0L) dateContentVersion.time.toString() else resources.getString(R.string.pop_up_info_never_synced)
        binding.popupInfoName.text = file.decryptedName
        binding.popupInfoId.text = file.id
        binding.popupInfoFileType.text = file.fileType.name
    }
}
