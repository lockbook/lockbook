package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.DialogFileInfoBinding
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import java.sql.Timestamp
import java.util.*

class FileInfoDialogFragment : DialogFragment() {
    private var _binding: DialogFileInfoBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()

    companion object {
        const val FILE_INFO_DIALOG_TAG = "FileInfoDialogFragment"
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DialogFileInfoBinding.inflate(
            inflater,
            container,
            false
        )

        setUpInfo()

        return binding.root
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            (resources.displayMetrics.widthPixels * 0.9).toInt(),
            WindowManager.LayoutParams.WRAP_CONTENT
        )
    }

    private fun setUpInfo() {
        val file = (activityModel.transientScreen as TransientScreen.Info).file
        val dateMetadataVersion = Date(Timestamp(file.metadataVersion).time)

        binding.popupInfoMetadataVersion.text = getString(
            R.string.popup_info_metadata_version,
            if (dateMetadataVersion.time != 0L) dateMetadataVersion else resources.getString(R.string.pop_up_info_never_synced)
        )

        val dateContentVersion = Date(Timestamp(file.contentVersion).time)
        binding.popupInfoContentVersion.text = getString(
            R.string.popup_info_content_version,
            if (dateContentVersion.time != 0L) dateContentVersion else resources.getString(R.string.pop_up_info_never_synced)
        )
        binding.popupInfoName.text = getString(R.string.popup_info_name, file.decryptedName)
        binding.popupInfoId.text = getString(R.string.popup_info_id, file.id)
        binding.popupInfoFileType.text =
            getString(R.string.popup_info_file_type, file.fileType.name)
    }
}
